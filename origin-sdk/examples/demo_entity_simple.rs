//! Simple entity demo: create or rotate attributes using the aligned SDK types.
//! Run: cargo run -p origin-sdk --example demo_entity_simple -- --endpoint ws://localhost:9944
//! --seed //Alice

use std::fs;

use clap::Parser;
use origin_primitives::{element::ElementType, AttributeValueView, Ss58Identifier};
use origin_sdk::{
	client::{signer::MultiKeySigner, Signer},
	schema::entity::EntityNestedValue,
	OriginClient,
};
use serde_json::Value as Json;
use subxt::utils::AccountId32;

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9910")]
	endpoint: String,
	#[clap(long, default_value = "//Bob")]
	seed: String,
	#[clap(long, default_value = "examples/data_entity.json")]
	data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();
	let data_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&args.data);
	let data: Json = serde_json::from_str(&fs::read_to_string(&data_path)?)?;

	let signer = MultiKeySigner::from_seed(&args.seed)?;
	let client = OriginClient::connect(&args.endpoint).await?;

	let account = signer.account_id();
	let bytes: [u8; 32] = account.clone().into();
	let account32 = AccountId32::from(bytes);

	let token_opt = client
		.query()
		.using(signer.clone())
		.entity()
		.account_token(account32.clone())
		.await?;

	let entity_id =
		if let Some(id) = token_opt {
			println!("Entity exists: {}", id.to_string_lossy());
			rotate_some(&client, &signer, &id, &data).await?;
			id
		} else {
			println!("No entity found for this account; skipping set_info to avoid InvalidFormat. Exiting.");
			return Ok(());
		};

	let overview = client.query().using(signer.clone()).entity().overview(entity_id).await?;
	match overview {
		Some(view) => println!("Entity overview: {:?}", view),
		None => println!("Entity not found or authorization failed"),
	}
	Ok(())
}

async fn rotate_some(
	client: &OriginClient,
	signer: &MultiKeySigner,
	entity: &Ss58Identifier,
	data: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let attrs = data["attributes"].as_object().ok_or("attributes missing")?;
	for (k, v) in attrs.iter().take(3) {
		submit_attr(client, signer, Some(entity), k, v).await?;
	}
	Ok(())
}

async fn create_entity(
	client: &OriginClient,
	signer: &MultiKeySigner,
	data: &Json,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let attrs = data["attributes"].as_object().ok_or("attributes missing")?;
	let display = to_element_view(attrs, "display")?;
	let web = to_element_view(attrs, "web")?;
	let email = to_element_view(attrs, "email")?;

	let mut dyn_attrs = Vec::new();
	for (k, v) in attrs {
		if k == "display" || k == "web" || k == "email" {
			continue;
		}
		let etype = parse_type(v.get("type").and_then(Json::as_str).unwrap_or("raw"));
		let val_json = v.get("value").unwrap_or(v);
		let ev = element_view_from_json(etype, val_json)?;
		dyn_attrs.push(AttributeValueView { key: k.as_bytes().to_vec(), value: ev });
	}

	let nested = EntityNestedValue { display, web, email, attributes: Some(dyn_attrs) };
	if let Err(e) = client
		.tx()
		.using(signer.clone())
		.entity()
		.submit_set_info_from_nested(&nested)
		.await
	{
		// If creation failed (likely already linked), try to fetch existing entity and return.
		let account = signer.account_id();
		let acct32 = AccountId32::from(<[u8; 32]>::from(account));
		let msg = e.to_string();
		if msg.contains("AccountAlreadyLinked") || msg.contains("InvalidFormat") {
			if let Some(id) = client
				.query()
				.using(signer.clone())
				.entity()
				.account_token(acct32)
				.await?
			{
				println!("Entity already exists; skipping create. id={}", id.to_string_lossy());
				return Ok(id);
			}
		}
		return Err(Box::new(e));
	}

	let id = client
		.query()
		.using(signer.clone())
		.entity()
		.account_token({
			let b: [u8; 32] = signer.account_id().into();
			AccountId32::from(b)
		})
		.await?
		.ok_or("entity id not found after creation")?;

	if let Some(nym) = data["nym"].as_str() {
		client
			.tx()
			.using(signer.clone())
			.entity()
			.submit_set_entity_nym(nym.as_bytes())
			.await?;
	}

	Ok(id)
}

async fn submit_attr(
	client: &OriginClient,
	signer: &MultiKeySigner,
	entity: Option<&Ss58Identifier>,
	key: &str,
	json: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let _etype = parse_type(json.get("type").and_then(Json::as_str).unwrap_or("raw"));
	let val_json = json.get("value").unwrap_or(json);
	let target = entity.ok_or("entity id required for attribute submit")?;
	let etype = parse_type(val_json.get("type").and_then(Json::as_str).unwrap_or("raw"));
	let ev = element_view_from_json(etype, val_json.get("value").unwrap_or(val_json))?;
	let handle = client
		.tx()
		.using(signer.clone())
		.entity()
		.submit_rotate_attribute_from_view(target.clone(), key.as_bytes(), ev)
		.await?;
	println!("rotate_attribute {:?} submitted hash {:?}", key, handle.hash);
	Ok(())
}

fn parse_type(s: &str) -> ElementType {
	match s.to_lowercase().as_str() {
		"bool" => ElementType::Bool,
		"u64" => ElementType::U64,
		"u128" => ElementType::U128,
		"hash" => ElementType::Hash,
		"token" => ElementType::Token,
		"cid" => ElementType::Cid,
		_ => ElementType::Raw,
	}
}

fn to_element_view(
	attrs: &serde_json::Map<String, Json>,
	key: &str,
) -> Result<origin_primitives::element::ElementView, Box<dyn std::error::Error>> {
	let v = attrs.get(key).ok_or_else(|| format!("missing {key}"))?;
	let etype = parse_type(v.get("type").and_then(Json::as_str).unwrap_or("raw"));
	let val_json = v.get("value").unwrap_or(v);
	element_view_from_json(etype, val_json)
}

fn element_view_from_json(
	etype: ElementType,
	val: &Json,
) -> Result<origin_primitives::element::ElementView, Box<dyn std::error::Error>> {
	use origin_primitives::element::ElementView::*;
	Ok(match etype {
		ElementType::None => None,
		ElementType::Raw => Raw(serde_json::to_vec(val)?),
		ElementType::Bool => Bool(val.as_bool().ok_or("expected bool")?),
		ElementType::U64 => U64(val.as_u64().ok_or("expected u64")?),
		ElementType::U128 => {
			let n = if let Some(u) = val.as_u64() {
				u as u128
			} else {
				val.as_str().ok_or("expected u128")?.parse()?
			};
			U128(n)
		},
		ElementType::Hash => {
			let s = val.as_str().ok_or("expected hash hex")?;
			let mut arr = [0u8; 32];
			let bytes = hex::decode(s)?;
			if bytes.len() != 32 {
				return Err("hash must be 32 bytes".into());
			}
			arr.copy_from_slice(&bytes);
			Hash(arr)
		},
		ElementType::Token => {
			let s = val.as_str().ok_or("expected ss58 string")?;
			Token(Ss58Identifier::try_from(s.to_string()).map_err(|e| format!("{e:?}"))?)
		},
		ElementType::Cid => {
			let s = val.as_str().ok_or("expected cid string")?;
			Cid(s.as_bytes().to_vec())
		},
	})
}
