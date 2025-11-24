//! Entity flow demo: ensure an entity exists, set/rotate info, ensure nym, then print overview.
use codec::{Decode, Encode};
use hex;
use origin_primitives::{element::ElementView, Ss58Identifier};
use origin_sdk::{
	client::{signer::MultiKeySigner, submit::TxOutcome, Signer},
	query::Query,
	types::{
		identifiers::{account_to_ss58, ss58_to_string},
		EntityStateView,
	},
	OriginClient, OriginSdkError,
};
use rand::{
	distributions::{Alphanumeric, Uniform},
	thread_rng, Rng,
};
use scale_value::{Composite, Primitive, Value, ValueDef};
use serde::Deserialize;
use std::{env, fmt::Write, fs};
use subxt::utils::AccountId32;

#[tokio::main]
async fn main() -> Result<(), OriginSdkError> {
	// 1) Simple client init with sr25519 signer (Alice is funded on dev chains).
	let signer = MultiKeySigner::from_seed("//Alice")
		.map_err(|e| OriginSdkError::InvalidInput(e.to_string()))?;
	let endpoint = env::args()
		.nth(1)
		.or_else(|| env::var("ORIGIN_RPC").ok())
		.unwrap_or_else(|| "ws://localhost:9910".to_string());
	let client = OriginClient::connect(endpoint).await?.with_signer(signer.clone());
	let domain = Query::new(&client);
	let account = signer.account_id();
	let account_ss58 = account_to_ss58(&account, 29);

	println!("\n🏷️ Origin Entity Demo");
	println!("  ↳ • ⛄️Signer account: {}", account_ss58);

	// Load demo templates.
	let demo = load_demo_templates("origin-sdk/examples/sample_data/demo.json")?;
	let mut rng = thread_rng();
	let label = format!("{}-{:04}", &account_ss58[..6], rng.gen_range(0..9999));
	let generated = GeneratedIds::new(&label);

	// 2) Ensure the account has an entity token (create if missing), attributes, and nym.
	let ensure =
		ensure_entity_and_nym(&client, &domain, account.clone(), &demo, &label, &generated).await?;
	let entity_id = ensure.entity;
	// let entity_id_str = ss58_to_string(&entity_id);
	// println!(
	// 	"Entity token for account {} (created: {}, nym set: {}): {}",
	// 	account_ss58, ensure.created, ensure.nym_set, entity_id_str
	// );

	// 3) Fetch the latest overview via a pure view call (no storage RPCs) and render nicely.
	let overview = client.view()?.entity().overview(entity_id).await?;
	println!("\n===== Entity Overview =====");
	render_overview(&overview, &account_ss58);

	Ok(())
}

struct EnsureResult {
	entity: Ss58Identifier,
	created: bool,
	nym_set: bool,
}

#[derive(Clone)]
struct GeneratedIds {
	did_key: String,
	did_web: String,
	did_cord: String,
	public_key: String,
}

impl GeneratedIds {
	fn new(label: &str) -> Self {
		let did_key = format!("did:key:z{}", random_base58(46));
		let did_cord = format!("did:cord:{}", random_base58(48));
		let did_web =
			format!("did:web:{}.example.org", sanitize_label_for_host(label).trim_matches('-'));
		let public_key = format!("pk-{}", random_base58(48));
		Self { did_key, did_web, did_cord, public_key }
	}
}

/// Ensure an entity exists for the account, create + set nym if not, and rotate info once.
async fn ensure_entity_and_nym(
	client: &OriginClient,
	domain: &Query<'_>,
	account: AccountId32,
	demo: &DemoData,
	label: &str,
	generated: &GeneratedIds,
) -> Result<EnsureResult, OriginSdkError> {
	// let storage_link = fetch_linked_entity_storage(client, &account).await?;
	let existing = fetch_linked_entity(client, &account).await?;

	println!("\nℹ️ Entity found");

	let mut created = false;
	let entity = if let Some(id) = existing {
		println!("  ↳ • Token  : {}", ss58_to_string(&id));
		id
	} else {
		println!("🔄 Create Entity\n");
		println!("ℹ️ Setting entity info");
		let info = build_entity_info(demo, label, generated);
		let outcome = submit_logged(client, "Entity", "set_info", vec![info]).await?;
		created = true;
		let token = extract_entity_token(&outcome)
			.ok_or_else(|| OriginSdkError::View("EntityInfoSet event missing".into()))?;
		println!("  ↳ • Token  : {}", ss58_to_string(&token));

		// Batch follow-ups: add attributes and set nym in one extrinsic to avoid nonce/prio races.
		let attrs = build_attribute_ops(&demo.entity, label, generated);
		let prefix = random_nym_prefix(label);
		let mut batch = client.tx()?.batch();
		if !attrs.is_empty() {
			batch = batch.call(client.call().call(
				"Entity",
				"add_attributes",
				vec![attrs_to_value(&attrs)],
			));
		}
		batch = batch.call(client.call().call(
			"Entity",
			"set_entity_nym",
			vec![Value::from_bytes(prefix.as_bytes())],
		));
		let _ = batch.submit_and_wait_finalized().await?;
		let new_nym = format!("{prefix}.nym.org.in"); // pallet appends suffix
		println!("  ↳ • Nym    : {new_nym}");

		fetch_linked_entity(client, &account).await?.unwrap_or(token)
	};

	// // Reconfirm linkage before nym operations.
	// let linked = fetch_linked_entity(client, &account).await?;
	// if linked.is_none() {
	// 	return Err(OriginSdkError::View(
	// 		"account is not linked to any entity after set_info".into(),
	// 	));
	// }

	// Ensure nym is set (view returns Ok(Some(name)) or Ok(None) on NotFound).
	let current_nym = domain.entity().nym(entity.clone()).await?.filter(|n| !n.is_empty());
	let mut nym_set = current_nym.is_some();
	if current_nym.is_none() {
		let prefix = random_nym_prefix(label);
		let _outcome = submit_set_entity_nym_retry(domain, &prefix).await?;
		let new_nym = format!("{prefix}.nym.org.in"); // pallet appends this suffix
		println!("  ↳ • Nym    : {new_nym}");
		nym_set = true;
	} else if let Some(n) = current_nym {
		let nym_str = String::from_utf8_lossy(&n);
		println!("  ↳ • Nym    : {nym_str}");
	}

	// Add or rotate attributes based on template.
	let overview = domain.entity().overview(entity.clone()).await?;
	let existing_keys: std::collections::HashSet<Vec<u8>> = overview
		.info
		.attributes
		.unwrap_or_default()
		.into_iter()
		.map(|a| a.key)
		.collect();

	let rotatable_keys: std::collections::HashSet<Vec<u8>> =
		["did:key", "did:web", "did:cord", "public-key"]
			.iter()
			.map(|k| k.as_bytes().to_vec())
			.collect();
	let all_ops = build_attribute_ops(&demo.entity, label, generated);
	let mut rotate_ops = Vec::new();
	let mut add_ops = Vec::new();
	for (key, val) in all_ops {
		if existing_keys.contains(&key) {
			if rotatable_keys.contains(&key) {
				rotate_ops.push((key, val));
			}
		} else {
			add_ops.push((key, val));
		}
	}

	if !add_ops.is_empty() {
		println!("Adding {} attribute(s): {}", add_ops.len(), key_list(&add_ops));
		let args = vec![attrs_to_value(&add_ops)];
		let _ = submit_logged(client, "Entity", "add_attributes", args).await?;
	}
	if !rotate_ops.is_empty() {
		println!("Rotating {} attribute(s): {}", rotate_ops.len(), key_list(&rotate_ops));
		let args = vec![attrs_to_value(&rotate_ops)];
		let _ = submit_logged(client, "Entity", "rotate_attributes", args).await?;
	}

	Ok(EnsureResult { entity, created, nym_set })
}

/// Try setting nym, retrying once if the account linkage isn't visible yet.
async fn submit_set_entity_nym_retry(
	domain: &Query<'_>,
	prefix: &str,
) -> Result<TxOutcome, OriginSdkError> {
	match domain.entity().tx().submit_set_entity_nym(prefix).await {
		Ok(outcome) => Ok(outcome),
		Err(e) if format!("{e}").contains("AccountNotFound") => {
			// Force a brief pause and recheck linkage before retry.
			tokio::time::sleep(std::time::Duration::from_millis(200)).await;
			let outcome = domain.entity().tx().submit_set_entity_nym(prefix).await?;
			Ok(outcome)
		},
		Err(e) if format!("{e}").contains("EntityNymAlreadySet") => {
			// Nym already present; treat as success.
			Ok(TxOutcome { block: None, hash: Default::default(), events: vec![] })
		},
		Err(e) => Err(e),
	}
}

/// Demo JSON templates
#[derive(Debug, Deserialize)]
struct DemoData {
	entity: EntityTemplate,
}

#[derive(Debug, Deserialize)]
struct EntityTemplate {
	display: String,
	web: String,
	email: String,
	attributes: Vec<AttrTemplate>,
}

#[derive(Debug, Deserialize)]
struct AttrTemplate {
	key: String,
	#[serde(rename = "type")]
	kind: String,
	generate: Option<String>,
	template: Option<String>,
	value: Option<serde_json::Value>,
	source: Option<String>,
}

fn load_demo_templates(path: &str) -> Result<DemoData, OriginSdkError> {
	let bytes =
		fs::read(path).map_err(|e| OriginSdkError::InvalidInput(format!("read {path}: {e}")))?;
	serde_json::from_slice(&bytes)
		.map_err(|e| OriginSdkError::InvalidInput(format!("parse {path}: {e}")))
}

/// Build EntityInfo payload from templates, substituting {label}.
fn build_entity_info(demo: &DemoData, label: &str, generated: &GeneratedIds) -> Value {
	let tpl = &demo.entity;
	let attrs = build_attribute_ops(tpl, label, generated);
	let attrs_val = if attrs.is_empty() {
		Value::variant("None", Composite::Unnamed(vec![]))
	} else {
		Value::variant("Some", Composite::Unnamed(vec![attrs_to_value(&attrs)]))
	};
	Value::named_composite(vec![
		("display".to_owned(), element_from_str(&tpl.display.replace("{label}", label))),
		("web".to_owned(), element_from_str(&tpl.web.replace("{label}", label))),
		("email".to_owned(), element_from_str(&tpl.email.replace("{label}", label))),
		("attributes".to_owned(), attrs_val),
	])
}

fn render_attr_string(attr: &AttrTemplate, label: &str, generated: &GeneratedIds) -> String {
	if let Some(gen) = generated_attr_string(attr, generated, label) {
		return gen;
	}
	if let Some(tpl) = &attr.template {
		let s = tpl.replace("{label}", label);
		return s;
	}
	if let Some(val) = &attr.value {
		if let Some(s) = val.as_str() {
			return s.to_string();
		}
	}
	format!("{label}-{}", random_label("attr"))
}

fn generated_attr_string(
	attr: &AttrTemplate,
	generated: &GeneratedIds,
	label: &str,
) -> Option<String> {
	let tag = attr.generate.as_deref().or_else(|| match attr.key.as_str() {
		"did:key" => Some("did_key"),
		"did:web" => Some("did_web"),
		"did:cord" => Some("did_cord"),
		_ => None,
	})?;
	match tag {
		"did_key" => Some(generated.did_key.clone()),
		"did_web" => Some(generated.did_web.clone()),
		"did_cord" => Some(generated.did_cord.clone()),
		"public_key" => Some(generated.public_key.clone()),
		other if other.starts_with("did:") => Some(random_did(other, label)),
		_ => None,
	}
}

/// Extract the newly issued entity token from EntityInfoSet event fields.
fn extract_entity_token(outcome: &TxOutcome) -> Option<Ss58Identifier> {
	outcome.events.iter().find_map(|ev| {
		if ev.pallet != "Entity" || ev.variant != "EntityInfoSet" {
			return None;
		}
		ev.fields.iter().find_map(value_to_ss58)
	})
}

/// Convert common scale_value encodings back into Ss58Identifier.
fn value_to_ss58(value: &scale_value::Value) -> Option<Ss58Identifier> {
	match &value.value {
		ValueDef::Primitive(Primitive::String(s)) => Ss58Identifier::try_from(s.clone()).ok(),
		ValueDef::Composite(Composite::Unnamed(vals)) => {
			if vals.len() == 1 {
				if let Some(id) = value_to_ss58(&vals[0]) {
					return Some(id);
				}
			}
			let mut bytes = Vec::with_capacity(vals.len());
			for v in vals {
				if let ValueDef::Primitive(Primitive::U128(n)) = &v.value {
					if *n <= 255 {
						bytes.push(*n as u8);
						continue;
					}
				}
				return None;
			}
			Ss58Identifier::try_from(bytes).ok()
		},
		_ => None,
	}
}

fn render_attr_key(attr: &AttrTemplate, label: &str) -> String {
	attr.key.replace("{label}", label)
}

fn element_from_str(s: &str) -> Value {
	Value::variant("Raw", Composite::Unnamed(vec![Value::from_bytes(s.as_bytes())]))
}

fn build_attribute_ops(
	tpl: &EntityTemplate,
	label: &str,
	generated: &GeneratedIds,
) -> Vec<(Vec<u8>, Value)> {
	tpl.attributes
		.iter()
		.map(|attr| {
			(render_attr_key(attr, label).into_bytes(), build_element(attr, label, generated))
		})
		.collect()
}

fn build_element(attr: &AttrTemplate, label: &str, generated: &GeneratedIds) -> Value {
	match attr.kind.as_str() {
		"none" => Value::variant("None", Composite::Unnamed(vec![])),
		"bool" => {
			let v = attr.value.as_ref().and_then(|v| v.as_bool()).unwrap_or(true);
			let b: u128 = if v { 1 } else { 0 };
			Value::variant("Bool", Composite::Unnamed(vec![Value::u128(b)]))
		},
		_ => {
			let s = render_attr_string(attr, label, generated);
			Value::variant("Raw", Composite::Unnamed(vec![Value::from_bytes(s.as_bytes())]))
		},
	}
}

fn attrs_to_value(entries: &[(Vec<u8>, Value)]) -> Value {
	let pairs: Vec<Value> = entries
		.iter()
		.map(|(k, v)| Value::unnamed_composite(vec![Value::from_bytes(k), v.clone()]))
		.collect();
	Value::from(pairs)
}

fn key_list(entries: &[(Vec<u8>, Value)]) -> String {
	entries
		.iter()
		.map(|(k, _)| String::from_utf8_lossy(k).into_owned())
		.collect::<Vec<_>>()
		.join(", ")
}
fn random_label(prefix: &str) -> String {
	let mut rng = rand::thread_rng();
	let suffix: String = (0..6).map(|_| rng.sample(Alphanumeric) as char).collect();
	format!("{prefix}-{suffix}")
}

fn random_identifier(len: usize) -> String {
	let mut rng = rand::thread_rng();
	(0..len).map(|_| rng.sample(Alphanumeric) as char).collect()
}

fn random_base58(len: usize) -> String {
	let charset = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
	let mut rng = rand::thread_rng();
	(0..len)
		.map(|_| {
			let idx = rng.gen_range(0..charset.len());
			char::from(charset[idx])
		})
		.collect()
}

fn random_did(method: &str, label: &str) -> String {
	format!("{method}:{}-{}", sanitize_label_for_host(label), random_identifier(8).to_lowercase())
}

fn sanitize_label_for_host(label: &str) -> String {
	label
		.chars()
		.map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
		.collect::<String>()
}

/// Generate a nym prefix that passes pallet validation: lowercase a-z0-9, no dots, length small.
fn random_nym_prefix(seed: &str) -> String {
	let mut rng = rand::thread_rng();
	let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
	let dist = Uniform::from(0..charset.len());
	// Pallet appends ".nym.org.in" (12 chars). MaxEntityNymLength is 32, so keep prefix <= 20.
	// Allow up to 6 chars from seed + 8 random = 14 (safe).
	let rand_len = 8usize;
	let mut rand_part = String::with_capacity(rand_len);
	for _ in 0..rand_len {
		rand_part.push(char::from(charset[rng.sample(dist)]));
	}
	let seed_part: String = seed
		.chars()
		.filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
		.take(6)
		.collect();
	if seed_part.is_empty() {
		rand_part
	} else {
		format!("{seed_part}{rand_part}")
	}
}

async fn submit_logged(
	client: &OriginClient,
	pallet: &str,
	function: &str,
	args: Vec<scale_value::Value>,
) -> Result<TxOutcome, OriginSdkError> {
	log_call_bytes(pallet, function, &args);
	let mut outcome = client
		.tx()?
		.submit_with_tip(pallet, function, args.clone(), 10)
		.await?
		.wait_in_block()
		.await;
	if let Err(ref e) = outcome {
		let msg = format!("{e}");
		if msg.contains("Priority is too low") || msg.contains("1014") {
			tokio::time::sleep(std::time::Duration::from_millis(400)).await;
			outcome = client
				.tx()?
				.submit_with_tip(pallet, function, args.clone(), 30_000)
				.await?
				.wait_in_block()
				.await;
		}
	}

	if let Ok(ref out) = outcome {
		println!("{}::{} in block {:?}, hash {:?}", pallet, function, out.block, out.hash);
	}
	if let Err(ref e) = outcome {
		println!("{}::{} failed: {}", pallet, function, e);
	}
	outcome
}

fn log_call_bytes(pallet: &str, function: &str, args: &[scale_value::Value]) {
	println!("Call {}::{} with {} arg(s)", pallet, function, args.len());
}

/// Fetch account->entity link and verify it resolves via overview; otherwise fall back to storage.
async fn fetch_linked_entity(
	client: &OriginClient,
	account: &AccountId32,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	// 1) Try view path first
	if let Ok(link) = client.view()?.entity().account_token(account.clone()).await {
		if let Some(id) = link.clone() {
			if client.view()?.entity().overview(id.clone()).await.is_ok() {
				return Ok(Some(id));
			}
		}
	}

	// 2) Fallback: storage path (which we know is correct)
	let storage_link = fetch_linked_entity_storage(client, account).await?;
	Ok(storage_link)
}

// /// Fetch account->entity link and verify it resolves via overview; otherwise treat as None.
// async fn fetch_linked_entity(
// 	client: &OriginClient,
// 	account: &AccountId32,
// ) -> Result<Option<Ss58Identifier>, OriginSdkError> {
// 	let link = client.view().entity().account_token(account.clone()).await?;
// 	if let Some(id) = link.clone() {
// 		if client.entity().overview(id.clone()).await.is_ok() {
// 			return Ok(Some(id));
// 		}
// 		eprintln!("token returned but overview failed: {:?}", id);
// 	}
// 	Ok(None)
// }

/// Debug helper: read EntityTokenOfAccount directly from storage to confirm mapping.
async fn fetch_linked_entity_storage(
	client: &OriginClient,
	account: &AccountId32,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	use subxt::dynamic::Value;
	let address = subxt::dynamic::storage(
		"Entity",
		"EntityTokenOfAccount",
		vec![Value::from_bytes(account.encode())],
	);
	let api = client.online();
	let value = api
		.storage()
		.at_latest()
		.await
		.map_err(|e| OriginSdkError::View(e.to_string()))?
		.fetch(&address)
		.await
		.map_err(|e| OriginSdkError::View(e.to_string()))?;
	if let Some(val) = value {
		let bytes = val.encoded();
		let id = Ss58Identifier::decode(&mut &bytes[..])
			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		return Ok(Some(id));
	}
	Ok(None)
}

/// Pretty print the overview with a friendly CLI layout.
fn render_overview(view: &EntityStateView, account_ss58: &str) {
	let mut out = String::new();
	writeln!(
		&mut out,
		"Owner: {}\nNym: {}\nLinked accounts: {}",
		account_ss58,
		view.nym
			.as_ref()
			.map(|n| String::from_utf8_lossy(n))
			.unwrap_or_else(|| "-".into()),
		view.linked_accounts.len()
	)
	.ok();

	writeln!(&mut out, "\nInfo:").ok();
	writeln!(
		&mut out,
		"  display : {}\n  web     : {}\n  email   : {}",
		format_element(&view.info.display),
		format_element(&view.info.web),
		format_element(&view.info.email)
	)
	.ok();
	if let Some(attrs) = &view.info.attributes {
		if !attrs.is_empty() {
			writeln!(&mut out, "  attributes:").ok();
			for attr in attrs {
				writeln!(
					&mut out,
					"    - {} = {}",
					String::from_utf8_lossy(&attr.key),
					format_element(&attr.value)
				)
				.ok();
			}
		}
	}

	if !view.history.is_empty() {
		writeln!(&mut out, "\nRecent attribute history (latest {}):", view.history.len()).ok();
		for h in &view.history {
			writeln!(
				&mut out,
				"  • {}@v{} <- {} (block {}:{})",
				String::from_utf8_lossy(&h.key),
				h.version,
				format_bytes_compact(&h.old_value),
				h.block.height,
				h.block.index
			)
			.ok();
		}
	}

	println!("{out}");
}

fn format_element(el: &ElementView) -> String {
	match el {
		ElementView::None => "-".into(),
		ElementView::Raw(bytes) => format_bytes_compact(bytes),
		ElementView::Bool(b) => format!("{b}"),
		ElementView::U64(v) => format!("{v}"),
		ElementView::U128(v) => format!("{v}"),
		ElementView::Hash(h) => format!("0x{}", hex::encode(h)),
		ElementView::Token(t) => ss58_to_string(t),
		ElementView::Cid(c) => String::from_utf8(c.clone()).unwrap_or_else(|_| hex::encode(c)),
	}
}

fn format_bytes_compact(bytes: &[u8]) -> String {
	match std::str::from_utf8(bytes) {
		Ok(s) if s.is_ascii() => s.to_string(),
		_ => {
			let preview: String =
				bytes.iter().take(16).map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("");
			if bytes.len() > 16 {
				format!("0x{}..({}b)", preview, bytes.len())
			} else {
				format!("0x{}", preview)
			}
		},
	}
}
