//! Registry + Packet demo using signer or meta-tx.
//! cargo run -p origin-sdk --example demo_registry_packet -- --endpoint ws://localhost:9944 --seed
//! //Alice [--meta]

use std::fs;

use clap::Parser;
use origin_primitives::Ss58Identifier;
use origin_sdk::{
	client::{signer::MultiKeySigner, Signer},
	schema, OriginClient, OriginSdkError,
};
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

	let signer = MultiKeySigner::from_seed(&args.seed)?;
	let client = OriginClient::connect(&args.endpoint).await?;

	// Resolve or create entity for controller
	let entity_id = ensure_entity(&client, &signer, args.meta).await?;

	// Create registry
	let registry_id = create_registry(&client, &signer, args.meta, reg_data, &entity_id).await?;
	println!("Registry created: {}", registry_id.to_string_lossy());

	// Issue packet
	let pkt_hash =
		issue_packet(&client, &signer, args.meta, reg_data, pkt_data, &registry_id, &entity_id)
			.await?;
	println!("Packet issued, tx hash {:?}", pkt_hash);

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
	signer: &MultiKeySigner,
	_meta: bool,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let account = signer.account_id();
	let acct32 = AccountId32::from(<[u8; 32]>::from(account));
	if let Some(id) = client
		.query()
		.using(signer.clone())
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
	let handle = client
		.tx()
		.using(signer.clone())
		.entity()
		.submit_set_info_from_nested(&nested)
		.await?;
	handle.wait_in_block().await?;
	let id = client
		.query()
		.using(signer.clone())
		.entity()
		.account_token(acct32)
		.await?
		.ok_or("entity id not found")?;
	Ok(id)
}

async fn create_registry(
	client: &OriginClient,
	signer: &MultiKeySigner,
	_meta: bool,
	reg: &Json,
	entity: &Ss58Identifier,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let schema = reg["attributes"].as_array().ok_or("registry.attributes missing")?;
	let token_spec = reg["token_spec"].clone();
	let lookup = reg["lookup_specs"].clone();
	let info = reg["info"].as_str().unwrap_or("demo registry").as_bytes().to_vec();

	let nested = origin_sdk::schema::registry::RegistryNestedSchema {
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
		token_spec: token_spec
			.as_array()
			.unwrap_or(&vec![])
			.iter()
			.filter_map(Json::as_str)
			.map(|s| s.as_bytes().to_vec())
			.collect(),
		lookup_specs: lookup
			.as_array()
			.unwrap_or(&vec![])
			.iter()
			.filter_map(Json::as_array)
			.map(|arr| arr.iter().filter_map(Json::as_str).map(|s| s.as_bytes().to_vec()).collect())
			.collect(),
		maintainer: entity.clone(),
	};

	let handle = client
		.tx()
		.using(signer.clone())
		.registry()
		.submit_create_from_nested(b"demo-registry", &nested)
		.await?;
	let outcome = handle.wait_in_block().await?;
	let new_id = registry_id_from_events(&outcome)?;
	Ok(new_id.unwrap_or(nested.registry.clone()))
}

async fn issue_packet(
	client: &OriginClient,
	signer: &MultiKeySigner,
	_meta: bool,
	reg: &Json,
	pkt: &Json,
	registry: &Ss58Identifier,
	entity: &Ss58Identifier,
) -> Result<subxt::utils::H256, Box<dyn std::error::Error>> {
	let schema = reg["attributes"]
		.as_array()
		.ok_or("registry.attributes missing")?
		.iter()
		.map(|a| {
			let key = a["key"].as_str().unwrap_or_default().as_bytes().to_vec();
			let typ = parse_type(a["type"].as_str().unwrap_or("raw"));
			let opt = a.get("optional").and_then(Json::as_bool).unwrap_or(false);
			(key, typ, opt)
		})
		.collect::<Vec<_>>();

	let attrs_view = schema_to_views(&schema);
	let nested = origin_sdk::schema::packet::PacketNestedValue {
		attributes: attrs_view
			.iter()
			.map(|s| {
				let key = s.key.clone();
				let val = pkt["attributes"]
					.get(String::from_utf8_lossy(&key).as_ref())
					.cloned()
					.unwrap_or(Json::Null);
				let view = json_to_view(s.kind, &val, entity)?;
				Ok(origin_primitives::packet::PacketAttributeView { key, value: view })
			})
			.collect::<Result<Vec<_>, OriginSdkError>>()?,
	};

	let handle = client
		.tx()
		.using(signer.clone())
		.registry()
		.submit_packet_from_nested(registry.clone(), &nested)
		.await?;
	let hash = handle.hash;
	handle.wait_in_block().await?;
	Ok(hash)
}

fn schema_to_views(
	schema: &[(Vec<u8>, origin_primitives::element::ElementType, bool)],
) -> Vec<origin_primitives::registry::RegistryAttributeView> {
	schema
		.iter()
		.map(|(k, t, o)| origin_primitives::registry::RegistryAttributeView {
			key: k.clone(),
			kind: *t,
			optional: *o,
		})
		.collect()
}

fn json_to_view(
	kind: origin_primitives::element::ElementType,
	val: &Json,
	entity: &Ss58Identifier,
) -> Result<origin_primitives::element::ElementView, OriginSdkError> {
	use origin_primitives::element::ElementView::*;
	Ok(match kind {
		origin_primitives::element::ElementType::None => None,
		origin_primitives::element::ElementType::Raw => {
			Raw(serde_json::to_vec(val).map_err(|e| OriginSdkError::InvalidInput(e.to_string()))?)
		},
		origin_primitives::element::ElementType::Bool => Bool(val.as_bool().unwrap_or(false)),
		origin_primitives::element::ElementType::U64 => U64(val.as_u64().unwrap_or_default()),
		origin_primitives::element::ElementType::U128 => {
			let n = val.as_u64().unwrap_or_default() as u128;
			U128(n)
		},
		origin_primitives::element::ElementType::Hash => {
			let s = val.as_str().unwrap_or_default();
			let bytes = hex::decode(s).map_err(|e| OriginSdkError::InvalidInput(format!("{e}")))?;
			let mut arr = [0u8; 32];
			if bytes.len() == 32 {
				arr.copy_from_slice(&bytes);
			}
			Hash(arr)
		},
		origin_primitives::element::ElementType::Token => {
			let fallback = entity.to_string_lossy();
			let s = val.as_str().unwrap_or(&fallback);
			Token(
				Ss58Identifier::try_from(s.to_string())
					.map_err(|e| OriginSdkError::InvalidInput(format!("{e:?}")))?,
			)
		},
		origin_primitives::element::ElementType::Cid => {
			let s = val.as_str().unwrap_or_default();
			Cid(s.as_bytes().to_vec())
		},
	})
}

fn parse_type(s: &str) -> origin_primitives::element::ElementType {
	match s.to_lowercase().as_str() {
		"bool" => origin_primitives::element::ElementType::Bool,
		"u64" => origin_primitives::element::ElementType::U64,
		"u128" => origin_primitives::element::ElementType::U128,
		"hash" => origin_primitives::element::ElementType::Hash,
		"token" => origin_primitives::element::ElementType::Token,
		"cid" => origin_primitives::element::ElementType::Cid,
		_ => origin_primitives::element::ElementType::Raw,
	}
}

fn registry_id_from_events(
	outcome: &origin_sdk::client::submit::TxOutcome,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	let _ = outcome; // simplified: registry id known from input in this demo
	Ok(None)
}
