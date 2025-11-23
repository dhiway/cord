//! Entity flow demo: ensure an entity exists, set/rotate info, ensure nym, then print overview.
use codec::{Decode, Encode};
use hex;
use origin_primitives::{element::ElementView, Ss58Identifier};
use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::client::submit::TxOutcome;
use origin_sdk::client::Signer;
use origin_sdk::domain::Domain;
use origin_sdk::types::identifiers::{account_to_ss58, ss58_to_string};
use origin_sdk::types::EntityStateView;
use origin_sdk::{OriginClient, OriginSdkError};
use rand::distributions::Uniform;
use rand::thread_rng;
use rand::{distributions::Alphanumeric, Rng};
use scale_value::{Composite, Primitive, Value, ValueDef};
use serde::Deserialize;
use sp_core::hashing::blake2_256;
use std::fmt::Write;
use std::fs;
use subxt::utils::AccountId32;

#[tokio::main]
async fn main() -> Result<(), OriginSdkError> {
	// 1) Simple client init with sr25519 signer (Alice is funded on dev chains).
	let signer = MultiKeySigner::from_seed("//Alice", "")
		.map_err(|e| OriginSdkError::InvalidInput(e.to_string()))?;
	let client = OriginClient::connect("ws://localhost:9910", signer.clone()).await?;
	let domain = Domain::new(&client);
	let account = signer.account_id();
	let account_ss58 = account_to_ss58(&account, 29);
	println!("Using signer account: {:?} ({})", account, account_ss58);

	// Load demo templates.
	let demo = load_demo_templates("origin-sdk/examples/sample_data/demo.json")?;
	let mut rng = thread_rng();
	let label = format!("{}-{:04}", &account_ss58[..6], rng.gen_range(0..9999));

	// 2) Ensure the account has an entity token (create if missing) and nym.
	let ensure = ensure_entity_and_nym(&client, &domain, account.clone(), &demo, &label).await?;
	let entity_id = ensure.entity;
	let entity_id_str = ss58_to_string(&entity_id);
	println!(
		"Entity token for account {} (created: {}, nym set: {}): {}",
		account_ss58, ensure.created, ensure.nym_set, entity_id_str
	);

	// 3) Fetch the latest overview via a pure view call (no storage RPCs) and render nicely.
	let overview = client.entity().overview(entity_id).await?;
	println!("\n===== Entity Overview =====");
	render_overview(&overview, &account_ss58);

	Ok(())
}

struct EnsureResult {
	entity: Ss58Identifier,
	created: bool,
	nym_set: bool,
}

/// Ensure an entity exists for the account, create + set nym if not, and rotate info once.
async fn ensure_entity_and_nym(
	client: &OriginClient,
	domain: &Domain<'_>,
	account: AccountId32,
	demo: &DemoData,
	label: &str,
) -> Result<EnsureResult, OriginSdkError> {
	let storage_link = fetch_linked_entity_storage(client, &account).await?;
	let existing = fetch_linked_entity(client, &account).await?;
	println!(
		"Existing entity as it is (if any): {:?} {:?} (storage: {:?})",
		existing, account, storage_link
	);
	println!("Existing entity link (if any): {:?}", existing.as_ref().map(ss58_to_string));
	if let Some(id) = existing.clone() {
		println!("Attempting overview for {:?}", id);
		// Fetch raw dynamic value for debugging
		match client
			.view()
			.call_bytes("Entity", "overview", vec![id.encode(), Option::<u32>::None.encode()])
			.await
		{
			Ok(bytes) => {
				let preview: Vec<String> =
					bytes.iter().take(24).map(|b| format!("{:02x}", b)).collect();
				println!(
					"overview raw bytes (len={}): {}{}",
					bytes.len(),
					preview.join(" "),
					if bytes.len() > 24 { " ..." } else { "" }
				);
			},
			Err(e) => println!("overview raw fetch error: {e}"),
		}
		match client.view().entity().overview(id.clone()).await {
			Ok(view) => println!("Overview decode ok: display={:?} {:?}", view.info.display, view),
			Err(e) => println!("Overview decode error: {e}"),
		}
	}
	let mut created = false;
	let entity = if let Some(id) = existing {
		// Reuse existing; do not call set_info again.
		id
	} else {
		let info = build_entity_info(demo, label, None, &account);
		let call = client.call().call("Entity", "set_info", vec![info]);
		println!("Submitting set_info with args count {}", call.args.len());
		log_call_bytes("Entity", "set_info", &call.args);
		match submit_logged(client, &call.pallet, &call.function, call.args).await {
			Ok(outcome) => {
				created = true;
				let token = extract_entity_token(&outcome)
					.ok_or_else(|| OriginSdkError::View("EntityInfoSet event missing".into()))?;
				// Allow some time for linkage to appear.
				fetch_linked_entity(client, &account).await?.unwrap_or(token)
			},
			Err(e) if format!("{e}").contains("AccountAlreadyLinked") => {
				// Someone linked this account between checks; just fetch the link.
				fetch_linked_entity(client, &account).await?.ok_or_else(|| {
					OriginSdkError::View("account linked but token not retrievable".into())
				})?
			},
			Err(e) => return Err(e),
		}
	};

	// Reconfirm linkage before nym operations.
	let linked = fetch_linked_entity(client, &account).await?;
	if linked.is_none() {
		return Err(OriginSdkError::View(
			"account is not linked to any entity after set_info".into(),
		));
	}

	// Ensure nym is set.
	let current_nym = domain.entity().nym(entity.clone()).await?;
	let current_nym = current_nym.filter(|n| !n.is_empty() && n != b"\x01\x01");
	let mut nym_set = current_nym.is_some();
	if current_nym.is_none() {
		let prefix = random_nym_prefix(label);
		let outcome = submit_set_entity_nym_retry(domain, &prefix).await?;
		println!("set_entity_nym finalized in block {:?}, hash {:?}", outcome.block, outcome.hash);
		nym_set = true;
	} else {
		println!("Nym already set: {:?}", current_nym.as_ref().map(|n| String::from_utf8_lossy(n)));
	}

	Ok(EnsureResult { entity, created, nym_set })
}

/// Try setting nym, retrying once if the account linkage isn't visible yet.
async fn submit_set_entity_nym_retry(
	domain: &Domain<'_>,
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
fn build_entity_info(
	demo: &DemoData,
	label: &str,
	entity_token_opt: Option<&Ss58Identifier>,
	account: &AccountId32,
) -> Value {
	let tpl = &demo.entity;
	Value::named_composite(vec![
		("display".to_owned(), element_from_str(&tpl.display.replace("{label}", label))),
		("web".to_owned(), element_from_str(&tpl.web.replace("{label}", label))),
		("email".to_owned(), element_from_str(&tpl.email.replace("{label}", label))),
		// For reliability keep attributes empty; toggle to build_attributes if needed.
		("attributes".to_owned(), Value::variant("None", Composite::Unnamed(vec![]))),
	])
}

fn render_attr_string(
	attr: &AttrTemplate,
	label: &str,
	entity_token_opt: Option<&Ss58Identifier>,
	account: &AccountId32,
) -> String {
	if let Some(tpl) = &attr.template {
		let mut s = tpl.replace("{label}", label);
		if s.contains("{entity}") {
			if let Some(tok) = entity_token_opt {
				s = s.replace("{entity}", &ss58_to_string(tok));
			}
		}
		if s.contains("{account}") {
			s = s.replace("{account}", &account_to_ss58(account, 29));
		}
		return s;
	}
	if let Some(val) = &attr.value {
		if let Some(s) = val.as_str() {
			return s.to_string();
		}
	}
	format!("{label}-{}", random_label("attr"))
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

fn build_attributes(
	tpl: &EntityTemplate,
	label: &str,
	entity_token_opt: Option<&Ss58Identifier>,
	account: &AccountId32,
) -> Value {
	if tpl.attributes.is_empty() {
		return Value::variant("None", Composite::Unnamed(vec![]));
	}
	let mut entries = Vec::new();
	for attr in &tpl.attributes {
		let key = render_attr_key(attr, label);
		let ev = build_element(attr, label, entity_token_opt, account);
		// BTreeMap encoded as Vec<(key, value)>
		let pair = Value::unnamed_composite(vec![Value::from_bytes(key.as_bytes()), ev]);
		entries.push(pair);
	}
	Value::variant("Some", Composite::Unnamed(vec![Value::from(entries)]))
}

fn build_element(
	attr: &AttrTemplate,
	label: &str,
	entity_token_opt: Option<&Ss58Identifier>,
	account: &AccountId32,
) -> Value {
	match attr.kind.as_str() {
		"none" => Value::variant("None", Composite::Unnamed(vec![])),
		"raw" => {
			let s = render_attr_string(attr, label, entity_token_opt, account);
			Value::variant("Raw", Composite::Unnamed(vec![Value::from_bytes(s.as_bytes())]))
		},
		"bool" => {
			let v = attr.value.as_ref().and_then(|v| v.as_bool()).unwrap_or(true);
			let b: u8 = if v { 1 } else { 0 };
			Value::variant("Bool", Composite::Unnamed(vec![v_u8(b)]))
		},
		"u64" => {
			let v = attr.value.as_ref().and_then(|v| v.as_u64()).unwrap_or(42);
			let bytes = v.to_le_bytes();
			let array_vals = bytes.iter().copied().map(v_u8).collect::<Vec<_>>();
			Value::variant("U64", Composite::Unnamed(vec![Value::unnamed_composite(array_vals)]))
		},
		"u128" => {
			let v_str = attr
				.value
				.as_ref()
				.and_then(|v| v.as_str().map(|s| s.to_string()))
				.unwrap_or_else(|| "1000".into());
			let v: u128 = v_str.parse().unwrap_or(1000);
			let bytes = v.to_le_bytes();
			let array_vals = bytes.iter().copied().map(v_u8).collect::<Vec<_>>();
			Value::variant("U128", Composite::Unnamed(vec![Value::unnamed_composite(array_vals)]))
		},
		"hash" => {
			let s = render_attr_string(attr, label, entity_token_opt, account);
			let digest = blake2_256(s.as_bytes());
			let array_vals = digest.iter().copied().map(v_u8).collect::<Vec<_>>();
			Value::variant("Hash", Composite::Unnamed(vec![Value::unnamed_composite(array_vals)]))
		},
		"token" => {
			let source = attr.source.as_deref().unwrap_or("account");
			let token_bytes = match source {
				"entity" => entity_token_opt
					.map(|t| t.as_ref().to_vec())
					.unwrap_or_else(|| account.0.to_vec()),
				_ => account.0.to_vec(),
			};
			Value::variant("Token", Composite::Unnamed(vec![Value::from_bytes(&token_bytes)]))
		},
		"cid" => {
			let s = render_attr_string(attr, label, entity_token_opt, account);
			Value::variant("CID", Composite::Unnamed(vec![Value::from_bytes(s.as_bytes())]))
		},
		_ => Value::variant("None", Composite::Unnamed(vec![])),
	}
}

fn v_u8(b: u8) -> Value {
	Value::u128(b as u128)
}
fn random_label(prefix: &str) -> String {
	let mut rng = rand::thread_rng();
	let suffix: String = (0..6).map(|_| rng.sample(Alphanumeric) as char).collect();
	format!("{prefix}-{suffix}")
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
	let outcome = client.tx().submit(pallet, function, args).await?.wait_finalized().await;
	if let Ok(ref out) = outcome {
		println!("{}::{} finalized in block {:?}, hash {:?}", pallet, function, out.block, out.hash);
	}
	if let Err(ref e) = outcome {
		println!("{}::{} failed: {}", pallet, function, e);
	}
	outcome
}

fn log_call_bytes(pallet: &str, function: &str, args: &[scale_value::Value]) {
	let previews: Vec<_> = args
		.iter()
		.enumerate()
		.map(|(i, v)| format!("#{} {:?}", i, v))
		.collect();
	println!("Call {}::{} args: {}", pallet, function, previews.join(" | "));
}

fn hex_preview(data: &[u8], n: usize) -> String {
	let mut out = String::new();
	for (i, b) in data.iter().take(n).enumerate() {
		if i > 0 {
			out.push(' ');
		}
		write!(&mut out, "{:02x}", b).ok();
	}
	if data.len() > n {
		out.push_str(" ...");
	}
	out
}

/// Fetch account->entity link and verify it resolves via overview; otherwise fall back to storage.
async fn fetch_linked_entity(
	client: &OriginClient,
	account: &AccountId32,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	// 1) Try view path first
	if let Ok(link) = client.view().entity().account_token(account.clone()).await {
		if let Some(id) = link.clone() {
			println!("view account_token returned: {:?}", id);
			if client.entity().overview(id.clone()).await.is_ok() {
				return Ok(Some(id));
			}
			eprintln!("view token invalid for overview, falling back to storage: {:?}", id);
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
				String::from_utf8_lossy(&h.old_value),
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
		ElementView::Raw(bytes) => {
			String::from_utf8(bytes.clone()).unwrap_or_else(|_| hex::encode(bytes))
		},
		ElementView::Bool(b) => format!("{b}"),
		ElementView::U64(v) => format!("{v}"),
		ElementView::U128(v) => format!("{v}"),
		ElementView::Hash(h) => format!("0x{}", hex::encode(h)),
		ElementView::Token(t) => ss58_to_string(t),
		ElementView::Cid(c) => String::from_utf8(c.clone()).unwrap_or_else(|_| hex::encode(c)),
	}
}
