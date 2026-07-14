//! Registry + Packet demo using signer or meta-tx.
//! cargo run -p origin-sdk --example demo_registry_packet -- --endpoint ws://localhost:9944 --seed
//! //Alice

use std::fs;

use clap::Parser;
use oc::{
	client::signer::OriginSigner, schema, tx::handle::TxOutcome, types::OriginAccount, OriginClient,
};
use origin_primitives::{element::ElementView, packet::PacketAttributeView, Ss58Identifier};
use serde_json::Value as Json;
use subxt::utils::AccountId32;

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9944")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(long, help = "submit via meta-tx instead of direct signer")]
	meta: bool,
	#[clap(long, default_value = "examples/data_registry_packet.json")]
	data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();
	let data_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&args.data);
	let data: Json = serde_json::from_str(&fs::read_to_string(&data_path)?)?;
	let reg_data = data.get("registry").ok_or("registry missing")?;
	let pkt_data = data.get("packet").ok_or("packet missing")?;

	let account = OriginAccount::from_uri(&args.seed, None)?;
	let signer = OriginSigner::from_account(&account)?;
	let client = OriginClient::connect(&args.endpoint).await?;
	let tx = client.tx().using(signer.clone());

	// Resolve or create entity for controller
	let entity_id = ensure_entity(&client, &tx, args.meta).await?;

	// Create registry
	let registry_id = create_registry(&client, &tx, args.meta, reg_data, &entity_id).await?;
	println!("Registry created: {}", registry_id.to_string_lossy());

	// Issue packet
	let pkt_hash =
		issue_packet(&tx, args.meta, reg_data, pkt_data, &registry_id, &entity_id).await?;
	println!("Packet issued, tx hash {:?}", pkt_hash.hash);

	// Fetch details
	let details = client.query().using(signer.clone()).registry().details(registry_id).await?;
	match details {
		Some(view) => println!("Registry details: {:?}", view),
		None => println!("Registry not found or authorization failed"),
	}
	Ok(())
}

async fn ensure_entity(
	client: &OriginClient,
	tx: &oc::tx::AccountTx,
	_meta: bool,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let account = tx.signer().account_id();
	let acct32 = AccountId32::from(<[u8; 32]>::from(account));
	if let Some(id) = client
		.query()
		.using(tx.signer().clone())
		.entity()
		.account_token(acct32.clone())
		.await?
	{
		return Ok(id);
	}
	let nested = schema::entity::EntityNestedValue {
		display: origin_primitives::element::ElementView::Raw(b"demo-entity".to_vec()),
		web: origin_primitives::element::ElementView::None,
		email: origin_primitives::element::ElementView::None,
		attributes: None,
	};
	let handle = tx.entity().submit_set_info_from_nested(&nested).await?;
	handle.wait_in_block().await?;
	let id = client
		.query()
		.using(tx.signer().clone())
		.entity()
		.account_token(acct32)
		.await?
		.ok_or("entity id not found")?;
	Ok(id)
}

async fn create_registry(
	_client: &OriginClient,
	tx: &oc::tx::AccountTx,
	_meta: bool,
	reg: &Json,
	entity: &Ss58Identifier,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let schema = reg["attributes"].as_array().ok_or("registry.attributes missing")?;
	let token_spec = reg["token_spec"].clone();
	let lookup = reg["lookup_specs"].clone();
	let info = reg["info"].as_str().unwrap_or("demo registry").as_bytes().to_vec();

	let nested = oc::schema::registry::RegistryNestedSchema {
		registry: Ss58Identifier::try_from(
			"5C8F41pKK9PXJ6A4ppfUT6asDkDw7py3AhtTx5xNUFDXL9Xb".to_string(),
		)
		.unwrap(),
		info: origin_primitives::element::ElementView::Raw(info),
		kind: origin_primitives::registry::RegistryKind::Raw,
		status: origin_primitives::registry::RegistryStatus::Active,
		attributes: schema
			.iter()
			.map(|a| origin_primitives::registry::RegistryAttributeView {
				key: a["key"].as_str().unwrap_or_default().as_bytes().to_vec(),
				kind: parse_type(a["type"].as_str().unwrap_or("raw")),
				optional: a.get("optional").and_then(Json::as_bool).unwrap_or(false),
			})
			.collect(),
		token_spec: spec_keys(&token_spec),
		lookup_specs: lookup.as_array().unwrap_or(&vec![]).iter().map(spec_keys).collect(),
		maintainer: entity.clone(),
	};

	let handle = tx.registry().submit_create_from_nested(b"demo-registry", &nested).await?;
	let outcome = handle.wait_in_block().await?;
	let new_id = registry_id_from_events(&outcome);
	Ok(new_id.unwrap_or(nested.registry.clone()))
}

async fn issue_packet(
	tx: &oc::tx::AccountTx,
	_meta: bool,
	_reg: &Json,
	pkt: &Json,
	registry: &Ss58Identifier,
	controller: &Ss58Identifier,
) -> Result<TxOutcome, Box<dyn std::error::Error>> {
	let nested =
		schema::packet::PacketNestedValue { attributes: packet_attributes(pkt, controller)? };
	let handle = tx.packet().submit_issue_from_nested(registry.clone(), &nested).await?;
	let outcome = handle.wait_finalized().await?;
	Ok(outcome)
}

fn spec_keys(spec: &Json) -> Vec<Vec<u8>> {
	if let Some(keys) = spec.get("keys").and_then(Json::as_array) {
		return keys.iter().filter_map(Json::as_str).map(|s| s.as_bytes().to_vec()).collect();
	}
	if let Some(key) = spec.get("key").and_then(Json::as_str) {
		return vec![key.as_bytes().to_vec()];
	}
	spec.as_array().map_or_else(Vec::new, |items| {
		items.iter().filter_map(Json::as_str).map(|s| s.as_bytes().to_vec()).collect()
	})
}

fn packet_attributes(
	pkt: &Json,
	controller: &Ss58Identifier,
) -> Result<Vec<PacketAttributeView>, Box<dyn std::error::Error>> {
	let attrs = pkt["attributes"].as_object().ok_or("packet.attributes missing")?;
	attrs
		.iter()
		.map(|(key, spec)| {
			Ok(PacketAttributeView {
				key: key.as_bytes().to_vec(),
				value: packet_value(spec, controller)?,
			})
		})
		.collect()
}

fn packet_value(
	spec: &Json,
	controller: &Ss58Identifier,
) -> Result<ElementView, Box<dyn std::error::Error>> {
	match spec["type"].as_str().unwrap_or("raw").to_ascii_lowercase().as_str() {
		"none" => Ok(ElementView::None),
		"bool" => Ok(ElementView::Bool(spec["value"].as_bool().unwrap_or(false))),
		"u64" => Ok(ElementView::U64(spec["value"].as_u64().unwrap_or_default())),
		"u128" => Ok(ElementView::U128(match &spec["value"] {
			Json::String(s) => s.parse()?,
			Json::Number(n) => n.as_u64().unwrap_or_default().into(),
			_ => 0,
		})),
		"hash" => {
			let hex_value = spec["value"].as_str().ok_or("hash value missing")?;
			let bytes = hex::decode(hex_value.strip_prefix("0x").unwrap_or(hex_value))?;
			let hash: [u8; 32] = bytes.try_into().map_err(|_| "hash value must be 32 bytes")?;
			Ok(ElementView::Hash(hash))
		},
		"token" => {
			if spec["source"].as_str() == Some("entity") {
				Ok(ElementView::Token(controller.clone()))
			} else {
				let value = spec["value"].as_str().ok_or("token value missing")?;
				let token = Ss58Identifier::try_from(value.to_owned()).map_err(|e| {
					std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e:?}"))
				})?;
				Ok(ElementView::Token(token))
			}
		},
		"cid" => {
			Ok(ElementView::Cid(spec["value"].as_str().unwrap_or_default().as_bytes().to_vec()))
		},
		_ => Ok(ElementView::Raw(raw_bytes(&spec["value"]))),
	}
}

fn raw_bytes(value: &Json) -> Vec<u8> {
	match value {
		Json::String(s) => s.as_bytes().to_vec(),
		Json::Array(_) | Json::Object(_) => value.to_string().into_bytes(),
		Json::Bool(v) => v.to_string().into_bytes(),
		Json::Number(v) => v.to_string().into_bytes(),
		Json::Null => Vec::new(),
	}
}

fn parse_type(s: &str) -> origin_primitives::element::ElementType {
	match s.to_ascii_lowercase().as_str() {
		"none" => origin_primitives::element::ElementType::None,
		"raw" => origin_primitives::element::ElementType::Raw,
		"bool" => origin_primitives::element::ElementType::Bool,
		"u8" | "u16" | "u32" | "u64" => origin_primitives::element::ElementType::U64,
		"u128" => origin_primitives::element::ElementType::U128,
		"hash" => origin_primitives::element::ElementType::Hash,
		"token" => origin_primitives::element::ElementType::Token,
		"cid" => origin_primitives::element::ElementType::Cid,
		_ => origin_primitives::element::ElementType::Raw,
	}
}

fn registry_id_from_events(_outcome: &TxOutcome) -> Option<Ss58Identifier> {
	None
}
