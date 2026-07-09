//! Entity demo that showcases the SDK surface (query/tx/schema), nested -> flat transforms,
//! nym setup, attribute rotation (batch), and optional meta-tx submission.
//! Usage:
//!   cargo run -p origin-sdk --example demo_entity -- --endpoint ws://localhost:9944 --seed //Alice
//!   cargo run -p origin-sdk --example demo_entity -- --endpoint ws://localhost:9944 --seed //Alice
//! --meta   (optional) --data examples/data_entity_demo.json

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use codec::Encode;
use futures::future::join_all;
use oc::{
	self,
	client::{signer::OriginSigner, OriginClient},
	extrinsic::{builder::DynamicCallBuilder, calls::entity::element_to_value},
	schema::entity::{element_from_view, EntityNestedValue},
	tx,
	tx::{handle, meta},
	types::{account::CryptoScheme, entity::ElementInput, EntityStateViewSdk, OriginAccount},
};
use origin_primitives::{element::ElementView, AttributeValueView, Ss58Identifier};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng, RngCore};
use scale_value::Composite;
use serde_json::Value as Json;
use sp_core::{crypto::Ss58Codec, sr25519::Public};
use subxt::{dynamic::Value, utils::AccountId32};
use tokio::time::sleep;

fn rand_tag(len: usize) -> String {
	rand::thread_rng()
		.sample_iter(&Alphanumeric)
		.take(len)
		.map(char::from)
		.map(|c| c.to_ascii_lowercase())
		.collect()
}

fn rand_phone() -> String {
	let mut rng = rand::thread_rng();
	format!("+91-922-{}-{:04}", rng.gen_range(100..999), rng.gen_range(0..10_000))
}

fn rand_hash32() -> [u8; 32] {
	let mut h = [0u8; 32];
	OsRng.fill_bytes(&mut h);
	h
}

fn rand_ss58_prefix29() -> String {
	let mut pk = [0u8; 32];
	OsRng.fill_bytes(&mut pk);
	Public::from_raw(pk).to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(29))
}

fn rand_public_key() -> String {
	let mut pk = [0u8; 32];
	OsRng.fill_bytes(&mut pk);
	format!("sr25519:{}", hex::encode(pk))
}

fn attributes_to_value(attrs: &Option<oc::types::entity::AttributesInput>) -> Value {
	match attrs {
		None => Value::variant("None", Composite::unnamed(vec![])),
		Some(list) => {
			let pairs: Vec<Value> = list
				.iter()
				.map(|(k, v)| {
					Value::unnamed_composite(vec![
						Value::from_bytes(k.to_vec()),
						element_to_value(v),
					])
				})
				.collect();
			Value::variant("Some", Composite::unnamed(vec![Value::from(pairs)]))
		},
	}
}

fn entity_info_value(info: &oc::types::entity::EntityInfoInput) -> Value {
	Value::unnamed_composite(vec![
		element_to_value(&info.display),
		element_to_value(&info.web),
		element_to_value(&info.email),
		attributes_to_value(&info.attributes),
	])
}

fn rand_account_id32_from_scheme(scheme: CryptoScheme) -> (OriginAccount, AccountId32) {
	let (acct, _) = OriginAccount::generate_with_scheme(scheme);
	let id = AccountId32::from(<[u8; 32]>::from(acct.account_id()));
	(acct, id)
}

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9910")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(long, default_value = "examples/data_entity_demo.json")]
	data: String,
	/// Use meta-tx submission instead of direct signer.
	#[clap(long)]
	meta: bool,
	/// Optional metadata hash (32-byte hex) to enable CheckMetadataHash when signing meta-tx.
	#[clap(long)]
	metadata_hash: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();

	let relayer_acct = OriginAccount::from_uri(&args.seed, None).map_err(|e| format!("{e:?}"))?;
	let relayer = OriginSigner::from_account(&relayer_acct).map_err(|e| format!("{e:?}"))?;
	let meta_signer = if args.meta {
		let meta_acct =
			OriginAccount::from_uri("//MetaSigner", None).map_err(|e| format!("{e:?}"))?;
		Some(OriginSigner::from_account(&meta_acct).map_err(|e| format!("{e:?}"))?)
	} else {
		None
	};
	let signer_for_entity = meta_signer.as_ref().unwrap_or(&relayer).clone();
	let client = OriginClient::connect(&args.endpoint).await?;
	let tx = client.tx().using(relayer.clone());
	let data = load_profile(&args.data)?;

	let account = signer_for_entity.account_id();
	let account32 = AccountId32::from(<[u8; 32]>::from(account.clone()));

	let hash_bytes = args
		.metadata_hash
		.as_ref()
		.map(|h| hex::decode(h.trim_start_matches("0x")))
		.transpose()?
		.map(|v| <[u8; 32]>::try_from(v.as_slice()).map_err(|_| "metadata hash must be 32 bytes"))
		.transpose()?;

	let account_fmt = oc::account_id_to_ss58(&account);
	println!("🔗 account: {}", account_fmt);
	println!("🔌 endpoint: {}", args.endpoint);
	println!("🚦 mode: {}", if args.meta { "meta-tx" } else { "direct signer" });
	if args.meta {
		println!(
			"ℹ️ meta-tx signing: metadata_hash={} (pass --metadata-hash 0x...)",
			hash_bytes
				.as_ref()
				.map(|h| format!("0x{}", hex::encode(h)))
				.unwrap_or_else(|| "None".into())
		);
	}

	let token_opt = client
		.query()
		.using(signer_for_entity.clone())
		.entity()
		.account_token(account32)
		.await?;

	if let Some(entity) = token_opt {
		println!("✅ entity exists: {}", entity.to_string_lossy());
		let mut scheme_map = std::collections::HashMap::new();
		scheme_map.insert(account_fmt.clone(), "sr25519".to_string());

		let state = client
			.query()
			.using(signer_for_entity.clone())
			.entity()
			.overview(entity.clone())
			.await?
			.unwrap_or_else(|| EntityStateViewSdk {
				info: origin_primitives::entity::EntityInfoView {
					display: ElementView::None,
					web: ElementView::None,
					email: ElementView::None,
					attributes: None,
				},
				nym: None,
				linked_accounts: Vec::new(),
				history: Vec::new(),
			});

		set_nym_if_missing(
			&client,
			&tx,
			&data,
			args.meta,
			meta_signer.as_ref(),
			state.nym.is_none(),
			hash_bytes,
		)
		.await?;

		let missing_links = 3usize.saturating_sub(state.linked_accounts.len());
		if missing_links > 0 {
			link_extra_accounts(&client, &tx, &mut scheme_map, missing_links).await?;
		}

		rotate_attributes(&client, &tx, &data, args.meta, meta_signer.as_ref(), hash_bytes).await?;
		show_overview(&client, &signer_for_entity, entity, Some(&scheme_map)).await?;
		return Ok(());
	}

	// Entity not found: only set info + attributes (no nym or linked accounts yet).
	println!("ℹ️ creating entity with info + attrs");
	let nested = build_nested(&data)?;
	let set_info_handle =
		create_entity(&client, &tx, &nested, args.meta, meta_signer.as_ref(), hash_bytes).await?;
	show_progress("set_info", set_info_handle.clone()).await;

	// Wait for finalization before fetching the new entity id.
	set_info_handle.clone().wait_finalized().await?;

	let entity = wait_for_entity_id(&client, &signer_for_entity).await?;

	let mut scheme_map = std::collections::HashMap::new();
	scheme_map.insert(account_fmt.clone(), "sr25519".to_string());

	println!("🎉 entity ready: {}", entity.to_string_lossy());
	show_overview(&client, &signer_for_entity, entity, Some(&scheme_map)).await?;
	Ok(())
}

fn load_profile(path: &str) -> Result<Json, Box<dyn std::error::Error>> {
	let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let p: PathBuf = base.join(path);
	let bytes = fs::read(p)?;
	Ok(serde_json::from_slice(&bytes)?)
}

fn build_nested(data: &Json) -> Result<EntityNestedValue, Box<dyn std::error::Error>> {
	let tag8 = rand_tag(8);
	let display = ElementView::Raw(
		format!("{} {}", data.get("display").and_then(Json::as_str).unwrap_or("Demo Entity"), tag8)
			.into_bytes(),
	);
	let web = ElementView::Raw(
		format!(
			"{}/{}",
			data.get("web").and_then(Json::as_str).unwrap_or("https://example.org"),
			tag8
		)
		.into_bytes(),
	);
	let email = ElementView::Raw(
		format!(
			"{}+{}@{}",
			data.get("email_user").and_then(Json::as_str).unwrap_or("hello"),
			tag8,
			data.get("email_domain").and_then(Json::as_str).unwrap_or("example.org")
		)
		.into_bytes(),
	);

	let mut attrs: Vec<AttributeValueView> = Vec::new();
	let mut rng = rand::thread_rng();

	// Start with any provided attributes
	if let Some(obj) = data.get("attributes").and_then(Json::as_object) {
		for (k, v) in obj {
			let ev = if v.is_object() {
				ElementView::Raw(serde_json::to_vec(v)?)
			} else {
				ElementView::Raw(v.as_str().unwrap_or_default().as_bytes().to_vec())
			};
			attrs.push(AttributeValueView { key: k.as_bytes().to_vec(), value: ev });
		}
	}

	// Ensure varied Element types with randomised values per run.
	let rand_tag: String = rand_tag(8);
	let rand_phone: String = rand_phone();

	// Remove any predefined keys we'll overwrite.
	let overwrite_keys = [
		b"did:web".to_vec(),
		b"did:cord".to_vec(),
		b"public-key".to_vec(),
		b"telephone".to_vec(),
		b"kyc".to_vec(),
		b"email-verified".to_vec(),
		b"login-count".to_vec(),
	];
	attrs.retain(|a| !overwrite_keys.contains(&a.key));

	// did:web (Raw)
	attrs.push(AttributeValueView {
		key: b"did:web".to_vec(),
		value: ElementView::Raw(format!("did:web:example.org:user:alice-{rand_tag}").into_bytes()),
	});
	// did:cord (Raw)
	attrs.push(AttributeValueView {
		key: b"did:cord".to_vec(),
		value: ElementView::Raw(format!("did:cord:{}", rand_ss58_prefix29()).into_bytes()),
	});
	// public-key (Raw with random suffix)
	attrs.push(AttributeValueView {
		key: b"public-key".to_vec(),
		value: ElementView::Raw(rand_public_key().into_bytes()),
	});
	// telephone (Raw)
	attrs.push(AttributeValueView {
		key: b"telephone".to_vec(),
		value: ElementView::Raw(rand_phone.into_bytes()),
	});
	// kyc (Hash)
	let mut kyc_hash = [0u8; 32];
	OsRng.fill_bytes(&mut kyc_hash);
	attrs.push(AttributeValueView { key: b"kyc".to_vec(), value: ElementView::Hash(kyc_hash) });
	// email-verified (Bool)
	attrs.push(AttributeValueView {
		key: b"email-verified".to_vec(),
		value: ElementView::Bool(true),
	});
	// login-count (U64)
	attrs.push(AttributeValueView {
		key: b"login-count".to_vec(),
		value: ElementView::U64(rng.gen_range(1_000u64..9_999u64)),
	});

	// memberships: array of strings (Raw JSON)
	let memberships = serde_json::to_vec(&vec!["bronze", "silver", "gold"])?;
	attrs.push(AttributeValueView {
		key: b"memberships".to_vec(),
		value: ElementView::Raw(memberships),
	});

	Ok(EntityNestedValue { display, web, email, attributes: Some(attrs) })
}

async fn create_entity(
	client: &OriginClient,
	tx: &tx::AccountTx,
	nested: &EntityNestedValue,
	use_meta: bool,
	meta_signer: Option<&OriginSigner>,
	meta_hash: Option<[u8; 32]>,
) -> Result<handle::TxHandle, Box<dyn std::error::Error>> {
	let handle = if use_meta {
		let meta_signer = meta_signer.ok_or("meta signer missing")?;
		let info_input = oc::schema::entity::to_entity_input(nested)?;
		let call = DynamicCallBuilder::new().call(
			"Entity",
			"set_info",
			vec![entity_info_value(&info_input)],
		);
		let meta = client.meta_tx().using(tx.signer().clone());
		let signed = if let Some(hash) = meta_hash {
			meta.prepare_and_sign_with_metadata_hash(
				call.clone(),
				Arc::new(meta_signer.clone()),
				Some(hash),
			)
			.await?
		} else {
			meta.prepare_and_sign_with(call.clone(), Arc::new(meta_signer.clone())).await?
		};
		if let Some(debug) = signed.debug() {
			let implicit = debug.bare.implicit_bytes();
			let call_bytes =
				call.encode_call_data(&client.metadata()).unwrap_or_else(|_| debug.call.clone());
			let preimage =
				meta::meta_tx_sign_payload(meta::META_TX_VERSION, &call_bytes, &debug.bare);
			println!(
				"meta-tx debug: implicit={} preimage_hash=0x{} call_bytes_len={}",
				hex::encode(&implicit),
				hex::encode(preimage),
				call_bytes.len()
			);
		}
		println!("meta-tx payload (hex): 0x{}", hex::encode(signed.encode()));
		meta.submit_signed(signed).await?
	} else {
		tx.entity().submit_set_info_from_nested(nested).await?
	};

	Ok(handle)
}

async fn set_nym(
	client: &OriginClient,
	tx: &tx::AccountTx,
	data: &Json,
	use_meta: bool,
	meta_signer: Option<&OriginSigner>,
	meta_hash: Option<[u8; 32]>,
) -> Result<handle::TxHandle, Box<dyn std::error::Error>> {
	let suffix_owned: String;
	let suffix = if let Some(s) = data.get("nym_suffix").and_then(Json::as_str) {
		s
	} else {
		suffix_owned = rand_tag(6);
		&suffix_owned
	};
	let nym = format!("demo.{}", suffix);
	let nym_bytes = nym.as_bytes().to_vec();

	if use_meta {
		let meta_signer = meta_signer.ok_or("meta signer missing")?;
		let call = DynamicCallBuilder::new().call(
			"Entity",
			"set_entity_nym",
			vec![subxt::dynamic::Value::from_bytes(nym_bytes)],
		);
		let meta = client.meta_tx().using(tx.signer().clone());
		let signed = if let Some(hash) = meta_hash {
			meta.prepare_and_sign_with_metadata_hash(
				call.clone(),
				Arc::new(meta_signer.clone()),
				Some(hash),
			)
			.await?
		} else {
			meta.prepare_and_sign_with(call.clone(), Arc::new(meta_signer.clone())).await?
		};
		println!("meta-tx payload (hex): 0x{}", hex::encode(signed.encode()));
		return Ok(meta.submit_signed(signed).await?);
	}

	Ok(tx.entity().submit_set_entity_nym(&nym_bytes).await?)
}

async fn set_nym_if_missing(
	client: &OriginClient,
	tx: &tx::AccountTx,
	data: &Json,
	use_meta: bool,
	meta_signer: Option<&OriginSigner>,
	missing: bool,
	meta_hash: Option<[u8; 32]>,
) -> Result<(), Box<dyn std::error::Error>> {
	if !missing {
		return Ok(());
	}
	let attempt = set_nym(client, tx, data, use_meta, meta_signer, meta_hash).await;
	match attempt {
		Ok(handle) => {
			show_progress("set_entity_nym", handle).await;
			Ok(())
		},
		Err(e) => {
			println!("⚠️  set_entity_nym failed (will retry once): {e}");
			let retry = set_nym(client, tx, data, use_meta, meta_signer, meta_hash).await;
			match retry {
				Ok(h) => {
					show_progress("set_entity_nym (retry)", h).await;
					Ok(())
				},
				Err(e2) => {
					println!("⚠️  set_entity_nym retry failed: {e2}");
					Ok(()) // continue demo even if nym fails
				},
			}
		},
	}
}

async fn wait_for_entity_id(
	client: &OriginClient,
	signer: &OriginSigner,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let acct32 = AccountId32::from(<[u8; 32]>::from(signer.account_id()));
	for _ in 0..20 {
		if let Some(entity) = client
			.query()
			.using(signer.clone())
			.entity()
			.account_token(acct32.clone())
			.await?
		{
			return Ok(entity);
		}
		tokio::time::sleep(Duration::from_millis(500)).await;
	}
	Err("entity id not found after set_info".into())
}

async fn link_extra_accounts(
	_client: &OriginClient,
	tx: &tx::AccountTx,
	scheme_map: &mut std::collections::HashMap<String, String>,
	missing: usize,
) -> Result<(), Box<dyn std::error::Error>> {
	if missing == 0 {
		return Ok(());
	}

	let mut tasks = Vec::new();

	if missing >= 1 {
		let (_ed_acct, ed_id) = rand_account_id32_from_scheme(CryptoScheme::Ed25519);
		let ed_handle = tx.entity().submit_set_linked_account(ed_id.clone()).await?;
		tasks.push(("link_ed25519", ed_handle));
		let ed_ss58 = oc::account_id_to_ss58(&sp_core::crypto::AccountId32::from(ed_id.0));
		scheme_map.insert(ed_ss58, "ed25519".into());
	}

	if missing >= 2 {
		let (_ec_acct, ec_id) = rand_account_id32_from_scheme(CryptoScheme::Ecdsa);
		let ec_handle = tx.entity().submit_set_linked_account(ec_id.clone()).await?;
		tasks.push(("link_ecdsa", ec_handle));
		let ec_ss58 = oc::account_id_to_ss58(&sp_core::crypto::AccountId32::from(ec_id.0));
		scheme_map.insert(ec_ss58, "ecdsa".into());
	}

	for (label, handle) in tasks {
		show_progress(label, handle).await;
	}

	Ok(())
}

async fn rotate_attributes(
	client: &OriginClient,
	tx: &tx::AccountTx,
	data: &Json,
	use_meta: bool,
	meta_signer: Option<&OriginSigner>,
	meta_hash: Option<[u8; 32]>,
) -> Result<(), Box<dyn std::error::Error>> {
	let keys: Vec<String> = data
		.get("rotate_keys")
		.and_then(Json::as_array)
		.map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
		.unwrap_or_else(|| {
			vec!["public-key".into(), "did:cord".into(), "telephone".into(), "kyc".into()]
		});

	let attrs_obj = data.get("attributes").and_then(Json::as_object).ok_or("attributes missing")?;

	let mut calls: Vec<(String, oc::extrinsic::builder::DynamicCall)> = Vec::new();
	let mut ops_views: Vec<(Vec<u8>, ElementView)> = Vec::new();
	let mut ops_inputs: Vec<(Vec<u8>, ElementInput)> = Vec::new();
	for key in keys {
		let ev = match key.as_str() {
			"public-key" => ElementView::Raw(rand_public_key().into_bytes()),
			"did:cord" =>
				ElementView::Raw(format!("did:cord:{}", rand_ss58_prefix29()).into_bytes()),
			"telephone" => ElementView::Raw(rand_phone().into_bytes()),
			"kyc" => ElementView::Hash(rand_hash32()),
			other =>
				if let Some(val) = attrs_obj.get(other) {
					if val.is_object() {
						ElementView::Raw(serde_json::to_vec(val)?)
					} else {
						ElementView::Raw(val.as_str().unwrap_or_default().as_bytes().to_vec())
					}
				} else {
					continue;
				},
		};
		let elem: ElementInput = element_from_view(&ev)?;
		calls.push((key.clone(), build_rotate_call(key.as_bytes(), &elem)));
		ops_views.push((key.as_bytes().to_vec(), ev.clone()));
		ops_inputs.push((key.as_bytes().to_vec(), elem));
	}

	if use_meta {
		let meta_signer = meta_signer.ok_or("meta signer missing")?;
		let mut progress = Vec::new();
		// Build a single rotate_attributes call (one arg: Vec<(Attribute, Element)>)
		let encoded = ops_inputs.encode();
		let call = DynamicCallBuilder::new().call(
			"Entity",
			"rotate_attributes",
			vec![subxt::dynamic::Value::from_bytes(encoded)],
		);
		let meta = client.meta_tx().using(tx.signer().clone());
		let signed = if let Some(hash) = meta_hash {
			meta.prepare_and_sign_with_metadata_hash(
				call.clone(),
				Arc::new(meta_signer.clone()),
				Some(hash),
			)
			.await?
		} else {
			meta.prepare_and_sign_with(call.clone(), Arc::new(meta_signer.clone())).await?
		};
		let h = meta.submit_signed(signed).await?;
		progress.push(show_progress("rotate_attributes (meta)", h));
		join_all(progress).await;
	} else {
		let handle = tx.entity().submit_rotate_attributes_from_nested(&ops_views).await?;
		show_progress("rotate_attributes", handle).await;
	}
	Ok(())
}

fn build_rotate_call(key: &[u8], elem: &ElementInput) -> oc::extrinsic::builder::DynamicCall {
	DynamicCallBuilder::new().call(
		"Entity",
		"rotate_attribute",
		vec![subxt::dynamic::Value::from_bytes(key), element_to_value(elem)],
	)
}

async fn show_progress(label: impl AsRef<str>, handle: handle::TxHandle) {
	let label = label.as_ref();
	println!("⏳ watching {label} (hash {:?})", handle.hash());
	let in_block = handle.clone().wait_in_block();
	tokio::pin!(in_block);
	match in_block.await {
		Ok(_) => println!("✅ {label} included"),
		Err(e) => println!("⚠️  {label} failed: {e}"),
	}
}

async fn show_overview(
	client: &OriginClient,
	signer: &OriginSigner,
	entity: Ss58Identifier,
	schemes: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), Box<dyn std::error::Error>> {
	println!("📖 Overview");
	let overview = client.query().using(signer.clone()).entity().overview(entity.clone()).await?;
	match overview {
		Some(state) => {
			println!("👤 Display : {}", fmt_element(&state.info.display));
			println!("🌐 Web     : {}", fmt_element(&state.info.web));
			println!("✉️  Email   : {}", fmt_element(&state.info.email));

			let nym = state
				.nym
				.as_ref()
				.map(|b| String::from_utf8_lossy(b).to_string())
				.unwrap_or_else(|| "—".into());
			println!("🏷️  Nym     : {}", nym);

			println!("🔗 Linked  : {} account(s)", state.linked_accounts.len());
			for (i, acc) in state.linked_accounts.iter().enumerate() {
				let acc_fmt = oc::account_id_to_ss58_subxt(acc);
				if let Some(map) = schemes {
					if let Some(s) = map.get(&acc_fmt) {
						println!("    [{}] {} ({})", i + 1, acc_fmt, s);
						continue;
					}
				}
				println!("    [{}] {}", i + 1, acc_fmt);
			}

			if let Some(attrs) = state.info.attributes {
				println!("🗝️  Attributes:");
				for a in attrs {
					let key = String::from_utf8_lossy(&a.key);
					println!("    • {:<14} = {}", key, fmt_element(&a.value));
				}
			} else {
				println!("🗝️  Attributes: none");
			}

			// Use overview history if present, otherwise fallback to dedicated history view.
			let mut history = state.history.clone();
			if history.is_empty() {
				if let Some(h) = client
					.query()
					.using(signer.clone())
					.entity()
					.attribute_history(entity.clone())
					.await?
				{
					history = h;
				}
			}

			println!("🕒 Attribute history (latest {} shown):", history.len().min(5));
			for h in history.iter().take(5) {
				let key = String::from_utf8_lossy(&h.key);
				let old = fmt_element(&h.old_value);
				println!(
					"    • {key} v{} @{}:{}  prev={}",
					h.version, h.block.height, h.block.index, old
				);
			}
		},
		None => println!("⚠️  entity not found or unauthorized"),
	}
	sleep(Duration::from_millis(300)).await;
	Ok(())
}

fn fmt_element(ev: &ElementView) -> String {
	match ev {
		ElementView::None => "∅".into(),
		ElementView::Raw(b) =>
			String::from_utf8(b.clone()).unwrap_or_else(|_| format!("0x{}", hex::encode(b))),
		ElementView::Bool(b) => format!("{b}"),
		ElementView::U64(v) => format!("{v}"),
		ElementView::U128(v) => format!("{v}"),
		ElementView::Hash(h) => format!("0x{}", hex::encode(h)),
		ElementView::Token(t) => t.to_string_lossy(),
		ElementView::Cid(c) => format!("cid:{}", hex::encode(c)),
	}
}
