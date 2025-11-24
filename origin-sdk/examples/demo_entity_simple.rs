//! Simple entity demo: create or rotate attributes using the aligned SDK types.
//! Run: cargo run -p origin-sdk --example demo_entity_simple -- --endpoint ws://localhost:9944 --seed //Alice

use std::fs;

use clap::Parser;
use origin_primitives::{element::ElementType, AttributeValueView, Ss58Identifier};
use origin_sdk::{
	client::{signer::MultiKeySigner, Signer},
	schema::entity::{to_entity_input, EntityNestedValue},
	OriginClient,
};
use serde_json::Value as Json;
use subxt::utils::AccountId32;

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9910")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
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
	let client = OriginClient::connect(&args.endpoint).await?.with_signer(signer.clone());

	let account = signer.account_id();
	let bytes: [u8; 32] = account.clone().into();
	let account32 = AccountId32::from(bytes);

	let token_opt = client.view()?.entity().account_token(account32).await.unwrap_or(None);

	let entity_id = if let Some(id) = token_opt {
		println!("Entity exists: {}", id.to_string_lossy());
		rotate_some(&client, &signer, &id, &data).await?;
		id
	} else {
		println!("No entity found; creating with set_info + nym");
		let id = create_entity(&client, &signer, &data).await?;
		println!("Created entity: {}", id.to_string_lossy());
		id
	};

	let overview = client.view()?.entity().overview(entity_id).await?;
	println!("Entity overview: {:?}", overview);
	Ok(())
}

async fn rotate_some(
	client: &OriginClient,
	_signer: &MultiKeySigner,
	entity: &Ss58Identifier,
	data: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let attrs = data["attributes"].as_object().ok_or("attributes missing")?;
	for (k, v) in attrs.iter().take(3) {
		submit_attr(client, Some(entity), k, v).await?;
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
	client
		.query()
		.using(signer.clone())
		.entity()
		.tx()
		.submit_set_info_from_nested(&nested)
		.await?;

	let id = client
		.view()?
		.entity()
		.account_token({
			let b: [u8; 32] = signer.account_id().into();
			AccountId32::from(b)
		})
		.await?
		.ok_or("entity id not found after creation")?;

	if let Some(nym) = data["nym"].as_str() {
		client.query().entity().tx().submit_set_entity_nym(nym).await?;
	}

	Ok(id)
}

async fn submit_attr(
	client: &OriginClient,
	entity: Option<&Ss58Identifier>,
	key: &str,
	json: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let _etype = parse_type(json.get("type").and_then(Json::as_str).unwrap_or("raw"));
	let val_json = json.get("value").unwrap_or(json);
	let target = entity.ok_or("entity id required for attribute submit")?;
	let handle = client
		.query()
		.entity()
		.tx()
		.submit_rotate_attribute_json(target.clone(), key, val_json)
		.await?;
	println!("rotate_attribute {:?} in block {:?}", key, handle.block);
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

fn element_input_to_value(
	elem: &origin_sdk::types::entity_input::ElementInput,
) -> scale_value::Value {
	use scale_value::{Composite, Value};
	match elem {
		origin_primitives::element::Elum::None => {
			Value::variant("None", Composite::unnamed(vec![]))
		},
		origin_primitives::element::Elum::Raw(bv) => {
			Value::variant("Raw", Composite::unnamed(vec![Value::from_bytes(bv.to_vec())]))
		},
		origin_primitives::element::Elum::Bool(b) => {
			Value::variant("Bool", Composite::unnamed(vec![Value::u128(*b as u128)]))
		},
		origin_primitives::element::Elum::U64(bytes) => {
			Value::variant("U64", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
		},
		origin_primitives::element::Elum::U128(bytes) => {
			Value::variant("U128", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
		},
		origin_primitives::element::Elum::Hash(bytes) => {
			Value::variant("Hash", Composite::unnamed(vec![Value::from_bytes(bytes.to_vec())]))
		},
		origin_primitives::element::Elum::Token(id) => {
			Value::variant("Token", Composite::unnamed(vec![Value::from_bytes(id.as_ref())]))
		},
		origin_primitives::element::Elum::CID(bv) => {
			Value::variant("CID", Composite::unnamed(vec![Value::from_bytes(bv.to_vec())]))
		},
	}
}

fn entity_info_value(info: &origin_sdk::types::EntityInfoInput) -> scale_value::Value {
	use scale_value::{Composite, Value};
	let attrs_val = match &info.attributes {
		Some(attrs) => {
			let pairs: Vec<Value> = attrs
				.iter()
				.map(|(k, v)| {
					Value::unnamed_composite(vec![
						Value::from_bytes(k.to_vec()),
						element_input_to_value(v),
					])
				})
				.collect();
			Value::variant("Some", Composite::unnamed(vec![Value::from(pairs)]))
		},
		None => Value::variant("None", Composite::unnamed(vec![])),
	};

	Value::named_composite(vec![
		("display", element_input_to_value(&info.display)),
		("web", element_input_to_value(&info.web)),
		("email", element_input_to_value(&info.email)),
		("attributes", attrs_val),
	])
}
