//! Registry + Packet demo using signer or meta-tx.
//! cargo run -p origin-sdk --example demo_registry_packet -- --endpoint ws://localhost:9944 --seed //Alice [--meta]

use std::fs;

use origin_primitives::Ss58Identifier;
use origin_sdk::{
	client::{signer::MultiKeySigner, Signer},
	extrinsic::builder::DynamicCall,
	extrinsic::calls::{packet, registry},
	OriginClient, OriginSdkError,
};
use scale_value::Value;
use subxt::utils::AccountId32;
use serde_json::Value as Json;
use clap::Parser;

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
	let client = OriginClient::connect(&args.endpoint).await?.with_signer(signer.clone());

	// Resolve or create entity for controller
	let entity_id = ensure_entity(&client, &signer, args.meta).await?;

	// Create registry
	let registry_id = create_registry(&client, &signer, args.meta, reg_data, &entity_id).await?;
	println!("Registry created: {}", registry_id.to_string_lossy());

	// Issue packet
	let pkt_hash = issue_packet(&client, &signer, args.meta, reg_data, pkt_data, &registry_id, &entity_id).await?;
	println!("Packet issued, tx hash {:?}", pkt_hash);

	// Fetch overview
	let overview = client.view()?.registry().overview(registry_id).await?;
	println!("Registry overview: {:?}", overview);
	Ok(())
}

async fn ensure_entity(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let acct = signer.account_id();
	let acct32 = AccountId32::from(acct.0);
	if let Some(id) = client.view()?.entity().account_token(acct32.clone()).await? {
		return Ok(id);
	}
	let call = DynamicCall {
		pallet: "Entity".into(),
		function: "set_info".into(),
		args: vec![Value::from_bytes(b"demo-entity")],
	};
	submit_call(client, signer, meta, call).await?;
	let id = client
		.view()?
		.entity()
		.account_token(acct32)
		.await?
		.ok_or("entity id not found")?;
	Ok(id)
}

async fn create_registry(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	reg: &Json,
	entity: &Ss58Identifier,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let schema = reg["attributes"].as_array().ok_or("registry.attributes missing")?;
	let token_spec = reg["token_spec"].clone();
	let lookup = reg["lookup_specs"].clone();
	let info = reg["info"].as_str().unwrap_or("demo registry").as_bytes().to_vec();

	let registry_id = b"demo-registry";
	let schema_bytes = serde_json::to_vec(schema)?;
	let config_bytes = serde_json::to_vec(&serde_json::json!({
		"token_spec": token_spec,
		"lookup_specs": lookup,
		"owner": entity.to_string_lossy(),
		"info": info,
	}))?;

	let call = registry::create_call(&client.metadata(), registry_id, &schema_bytes, &config_bytes)?;
	let dyn_call = DynamicCall {
		pallet: "Register".into(),
		function: "create_registry".into(),
		args: call.args().to_vec(),
	};
	let handle = submit_call(client, signer, meta, dyn_call).await?;
	let outcome = handle;
	let new_id = registry_id_from_events(&outcome)?;
	Ok(new_id.unwrap_or_else(|| Ss58Identifier::try_from("5C8F41pKK9PXJ6A4ppfUT6asDkDw7py3AhtTx5xNUFDXL9Xb").unwrap()))
}

async fn issue_packet(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
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

	let mut body = pkt["attributes"]
		.as_object()
		.ok_or("packet.attributes missing")?
		.clone();
	// fill controller if source == entity
	for (_k, v) in body.iter_mut() {
		if let Some(src) = v.get("source").and_then(Json::as_str) {
			if src == "entity" {
				*v = serde_json::json!({ "type": "token", "value": entity.to_string_lossy() });
			}
		}
	}
	let call = packet::issue_call(
		&client.metadata(),
		registry.clone(),
		&schema_to_views(&schema),
		&serde_json::Value::Object(body),
	)?;
	let dyn_call = DynamicCall {
		pallet: "Register".into(),
		function: "create_packet".into(),
		args: call.args().to_vec(),
	};
	let handle = submit_call(client, signer, meta, dyn_call).await?;
	Ok(handle.hash)
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

async fn submit_call(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	call: scale_value::dynamic::DynamicPayload,
) -> Result<origin_sdk::client::submit::TxOutcome, Box<dyn std::error::Error>> {
	if meta {
		let handle = client.metatx_with(signer.clone()).sign_submit_and_wait_checked(call.into()).await?;
		let outcome = handle;
		println!("meta-tx finalized {:?}", outcome.block);
		Ok(outcome)
	} else {
		let handle = client.tx_with(signer.clone()).submit_payload(call).await?;
		let outcome = handle.wait_finalized().await?;
		println!("tx finalized {:?}", outcome.block);
		Ok(outcome)
	}
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

fn registry_id_from_events(outcome: &origin_sdk::client::submit::TxOutcome) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	for ev in &outcome.events {
		if ev.pallet == "Register" && ev.variant == "Created" {
			if let Some(Value::Primitive(scale_value::Primitive::Bytes(b))) = ev.fields.get(0) {
				if let Ok(id) = Ss58Identifier::try_from(b.clone()) {
					return Ok(Some(id));
				}
			}
		}
	}
	Ok(None)
}
