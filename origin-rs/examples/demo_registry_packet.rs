//! Registry + Packet demo using signer or meta-tx.
//! cargo run -p origin-sdk --example demo_registry_packet -- --endpoint ws://localhost:9944 --seed
//! //Alice

use std::fs;

use clap::Parser;
use origin_primitives::Ss58Identifier;
use origin_sdk::{
	client::signer::OriginSigner, schema, tx::handle::TxOutcome, types::OriginAccount,
	OriginClient, OriginSdkError,
};
use scale_value;
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
	println!("Packet issued, tx hash {:?}", pkt_hash.hash());

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
	tx: &origin_sdk::tx::AccountTx,
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
	client: &OriginClient,
	tx: &origin_sdk::tx::AccountTx,
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

	let handle = tx.registry().submit_create_from_nested(b"demo-registry", &nested).await?;
	let outcome = handle.wait_in_block().await?;
	let new_id = registry_id_from_events(&outcome)?;
	Ok(new_id.unwrap_or(nested.registry.clone()))
}

async fn issue_packet(
	tx: &origin_sdk::tx::AccountTx,
	_meta: bool,
	reg: &Json,
	pkt: &Json,
	registry: &Ss58Identifier,
	controller: &Ss58Identifier,
) -> Result<TxOutcome, Box<dyn std::error::Error>> {
	let nested = schema::packet::PacketNestedValue {
		name: reg["info"].as_str().unwrap_or("demo packet").as_bytes().to_vec(),
		data: pkt["data"].as_str().unwrap_or("demo data").as_bytes().to_vec(),
		controller: Some(controller.clone()),
		attributes: None,
	};
	let handle = tx.packet().submit_issue_from_nested(registry.clone(), &nested).await?;
	let outcome = handle.wait_finalized().await?;
	Ok(outcome)
}

fn parse_type(s: &str) -> origin_primitives::element::ElementType {
	match s.to_ascii_lowercase().as_str() {
		"bool" => origin_primitives::element::ElementType::Bool,
		"u8" => origin_primitives::element::ElementType::U8,
		"u16" => origin_primitives::element::ElementType::U16,
		"u32" => origin_primitives::element::ElementType::U32,
		"u64" => origin_primitives::element::ElementType::U64,
		"i128" => origin_primitives::element::ElementType::I128,
		"u128" => origin_primitives::element::ElementType::U128,
		_ => origin_primitives::element::ElementType::Raw,
	}
}

fn registry_id_from_events(outcome: &TxOutcome) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	for ev in &outcome.events {
		if ev.pallet == "Register" && ev.variant == "RegistryCreated" {
			if let Some(scale_value::Value { value: scale_value::ValueDef::Bytes(b), .. }) =
				ev.fields.get(0)
			{
				return Ok(Ss58Identifier::try_from(b.clone()).ok());
			}
		}
	}
	Ok(None)
}
