//! Entity demo that showcases the SDK surface (query/tx/schema), nested -> flat transforms,
//! nym setup, attribute rotation (batch), and optional meta-tx submission.
//! Usage:
//!   cargo run -p origin-sdk --example demo_entity -- --endpoint ws://localhost:9944 --seed //Alice
//!   cargo run -p origin-sdk --example demo_entity -- --endpoint ws://localhost:9944 --seed //Alice --meta
//!   (optional) --data examples/data_entity_demo.json

use std::{fs, path::PathBuf, time::Duration};

use clap::Parser;
use codec::Encode;
use futures::future::join_all;
use origin_primitives::{element::ElementView, AttributeValueView, Ss58Identifier};
use origin_sdk::{
	client::{signer::MultiKeySigner, OriginClient, Signer},
	extrinsic::calls::entity::element_to_value,
	schema::entity::{element_from_view, EntityNestedValue},
	types::entity_input::ElementInput,
};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng, RngCore};
use serde_json::Value as Json;
use subxt::utils::AccountId32;
use tokio::time::sleep;

fn rand_tag(len: usize) -> String {
	rand::thread_rng()
		.sample_iter(&Alphanumeric)
		.take(len)
		.map(char::from)
		.collect()
}

fn rand_phone() -> String {
	let mut rng = rand::thread_rng();
	format!("+1-555-{}-{:04}", rng.gen_range(100..999), rng.gen_range(0..10_000))
}

fn rand_hash32() -> [u8; 32] {
	let mut h = [0u8; 32];
	OsRng.fill_bytes(&mut h);
	h
}

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9910")]
	endpoint: String,
	#[clap(long, default_value = "//Bob")]
	seed: String,
	#[clap(long, default_value = "examples/data_entity_demo.json")]
	data: String,
	/// Use meta-tx submission instead of direct signer.
	#[clap(long)]
	meta: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();

	let signer = MultiKeySigner::from_seed(&args.seed)?;
	let client = OriginClient::connect(&args.endpoint).await?;
	let data = load_profile(&args.data)?;

	let account = signer.account_id();
	let account32 = AccountId32::from(<[u8; 32]>::from(account.clone()));

	println!("🔗 account: {}", account);
	println!("🔌 endpoint: {}", args.endpoint);
	println!("🚦 mode: {}", if args.meta { "meta-tx" } else { "direct signer" });

	let token_opt = client.query().using(signer.clone()).entity().account_token(account32).await?;

	if let Some(entity) = token_opt {
		println!("✅ entity exists: {}", entity.to_string_lossy());
		rotate_attributes(&client, &signer, &data, args.meta).await?;
		show_overview(&client, &signer, entity).await?;
		return Ok(());
	}

	// Create path
	println!("ℹ️ creating entity with info + attrs + nym");
	let nested = build_nested(&data)?;
	let (entity, set_info_handle) = create_entity(&client, &signer, &nested, args.meta).await?;
	show_progress("set_info", set_info_handle).await;

	let nym_handle = set_nym(&client, &signer, &data, args.meta).await?;
	show_progress("set_entity_nym", nym_handle).await;

	println!("🎉 entity ready: {}", entity.to_string_lossy());
	show_overview(&client, &signer, entity).await?;
	Ok(())
}

fn load_profile(path: &str) -> Result<Json, Box<dyn std::error::Error>> {
	let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let p: PathBuf = base.join(path);
	let bytes = fs::read(p)?;
	Ok(serde_json::from_slice(&bytes)?)
}

fn build_nested(data: &Json) -> Result<EntityNestedValue, Box<dyn std::error::Error>> {
	let display = ElementView::Raw(
		data.get("display")
			.and_then(Json::as_str)
			.unwrap_or("Demo Entity")
			.as_bytes()
			.to_vec(),
	);
	let web =
		ElementView::Raw(data.get("web").and_then(Json::as_str).unwrap_or("").as_bytes().to_vec());
	let email = ElementView::Raw(
		data.get("email").and_then(Json::as_str).unwrap_or("").as_bytes().to_vec(),
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
	let rand_tag: String =
		rand::thread_rng().sample_iter(&Alphanumeric).take(6).map(char::from).collect();

	let rand_phone: String =
		format!("+1-555-{}-{:04}", rng.gen_range(100..999), rng.gen_range(0..10_000));

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
		value: ElementView::Raw(format!("did:cord:{}", rand_tag).into_bytes()),
	});
	// public-key (Raw with random suffix)
	let mut pk_bytes = [0u8; 16];
	OsRng.fill_bytes(&mut pk_bytes);
	attrs.push(AttributeValueView {
		key: b"public-key".to_vec(),
		value: ElementView::Raw(format!("ed25519:{}", hex::encode(pk_bytes)).into_bytes()),
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

	Ok(EntityNestedValue { display, web, email, attributes: Some(attrs) })
}

async fn create_entity(
	client: &OriginClient,
	signer: &MultiKeySigner,
	nested: &EntityNestedValue,
	use_meta: bool,
) -> Result<(Ss58Identifier, origin_sdk::client::submit::TxHandle), Box<dyn std::error::Error>> {
	if use_meta {
		let info_input = origin_sdk::schema::entity::to_entity_input(nested)?;
		let call = origin_sdk::extrinsic::builder::DynamicCallBuilder::new().call(
			"Entity",
			"set_info",
			vec![subxt::dynamic::Value::from_bytes(info_input.encode())],
		);
		let handle = client.metatx().sign_and_submit(call).await?;
		handle.clone().wait_in_block().await?;
		let acct32 = AccountId32::from(<[u8; 32]>::from(signer.account_id()));
		let entity = client
			.query()
			.using(signer.clone())
			.entity()
			.account_token(acct32)
			.await?
			.ok_or("entity id not found after set_info")?;
		return Ok((entity, handle));
	}

	let handle = client
		.tx()
		.using(signer.clone())
		.entity()
		.submit_set_info_from_nested(nested)
		.await?;
	let acct32 = AccountId32::from(<[u8; 32]>::from(signer.account_id()));
	let entity = client
		.query()
		.using(signer.clone())
		.entity()
		.account_token(acct32)
		.await?
		.ok_or("entity id not found after set_info")?;
	Ok((entity, handle))
}

async fn set_nym(
	client: &OriginClient,
	signer: &MultiKeySigner,
	data: &Json,
	use_meta: bool,
) -> Result<origin_sdk::client::submit::TxHandle, Box<dyn std::error::Error>> {
	let nym_bytes = data
		.get("nym")
		.and_then(Json::as_str)
		.unwrap_or("demo.nym.cord")
		.as_bytes()
		.to_vec();

	if use_meta {
		let call = origin_sdk::extrinsic::builder::DynamicCallBuilder::new().call(
			"Entity",
			"set_entity_nym",
			vec![subxt::dynamic::Value::from_bytes(nym_bytes)],
		);
		return Ok(client.metatx().sign_and_submit(call).await?);
	}

	Ok(client
		.tx()
		.using(signer.clone())
		.entity()
		.submit_set_entity_nym(&nym_bytes)
		.await?)
}

async fn rotate_attributes(
	client: &OriginClient,
	signer: &MultiKeySigner,
	data: &Json,
	use_meta: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let keys: Vec<String> = data
		.get("rotate_keys")
		.and_then(Json::as_array)
		.map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
		.unwrap_or_else(|| {
			vec!["public-key".into(), "did:cord".into(), "telephone".into(), "kyc".into()]
		});

	let attrs_obj = data.get("attributes").and_then(Json::as_object).ok_or("attributes missing")?;

	let mut calls: Vec<(String, origin_sdk::extrinsic::builder::DynamicCall)> = Vec::new();
	let mut ops_views: Vec<(Vec<u8>, ElementView)> = Vec::new();
	let mut ops_inputs: Vec<(Vec<u8>, ElementInput)> = Vec::new();
	for key in keys {
		let ev = match key.as_str() {
			"public-key" => {
				let mut pk_bytes = [0u8; 16];
				OsRng.fill_bytes(&mut pk_bytes);
				ElementView::Raw(format!("ed25519:{}", hex::encode(pk_bytes)).into_bytes())
			},
			"did:cord" => ElementView::Raw(format!("did:cord:{}", rand_tag(10)).into_bytes()),
			"telephone" => ElementView::Raw(rand_phone().into_bytes()),
			"kyc" => ElementView::Hash(rand_hash32()),
			other => {
				if let Some(val) = attrs_obj.get(other) {
					if val.is_object() {
						ElementView::Raw(serde_json::to_vec(val)?)
					} else {
						ElementView::Raw(val.as_str().unwrap_or_default().as_bytes().to_vec())
					}
				} else {
					continue;
				}
			},
		};
		let elem: ElementInput = element_from_view(&ev)?;
		calls.push((key.clone(), build_rotate_call(key.as_bytes(), &elem)));
		ops_views.push((key.as_bytes().to_vec(), ev.clone()));
		ops_inputs.push((key.as_bytes().to_vec(), elem));
	}

	if use_meta {
		let mut progress = Vec::new();
		// Build a single rotate_attributes call (one arg: Vec<(Attribute, Element)>)
		let encoded = ops_inputs.encode();
		let call = origin_sdk::extrinsic::builder::DynamicCallBuilder::new().call(
			"Entity",
			"rotate_attributes",
			vec![subxt::dynamic::Value::from_bytes(encoded)],
		);
		let h = client.metatx().sign_and_submit(call).await?;
		progress.push(show_progress("rotate_attributes (meta)", h));
		join_all(progress).await;
	} else {
		let handle = client
			.tx()
			.using(signer.clone())
			.entity()
			.submit_rotate_attributes_from_nested(&ops_views)
			.await?;
		show_progress("rotate_attributes", handle).await;
	}
	Ok(())
}

fn build_rotate_call(
	key: &[u8],
	elem: &ElementInput,
) -> origin_sdk::extrinsic::builder::DynamicCall {
	origin_sdk::extrinsic::builder::DynamicCallBuilder::new().call(
		"Entity",
		"rotate_attribute",
		vec![subxt::dynamic::Value::from_bytes(key), element_to_value(elem)],
	)
}

async fn show_progress(label: impl AsRef<str>, handle: origin_sdk::client::submit::TxHandle) {
	let label = label.as_ref();
	println!("⏳ watching {label} (hash {:?})", handle.hash);
	let in_block = handle.clone().wait_in_block();
	tokio::pin!(in_block);
	if in_block.await.is_ok() {
		println!("✅ {label} included");
	} else {
		println!("⚠️  {label} failed");
	}
}

async fn show_overview(
	client: &OriginClient,
	signer: &MultiKeySigner,
	entity: Ss58Identifier,
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
				println!("    [{}] {}", i + 1, acc);
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

			println!("🕒 Attribute history (latest {} shown):", state.history.len().min(5));
			for h in state.history.iter().take(5) {
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
		ElementView::Raw(b) => {
			String::from_utf8(b.clone()).unwrap_or_else(|_| format!("0x{}", hex::encode(b)))
		},
		ElementView::Bool(b) => format!("{b}"),
		ElementView::U64(v) => format!("{v}"),
		ElementView::U128(v) => format!("{v}"),
		ElementView::Hash(h) => format!("0x{}", hex::encode(h)),
		ElementView::Token(t) => t.to_string_lossy(),
		ElementView::Cid(c) => format!("cid:{}", hex::encode(c)),
	}
}
