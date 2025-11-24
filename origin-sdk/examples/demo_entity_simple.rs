//! Simple entity demo: create or rotate attributes, works with direct signer or meta-tx relay.
//! cargo run -p origin-sdk --example demo_entity_simple -- --endpoint ws://localhost:9944 --seed //Alice [--meta]

use std::fs;

use clap::Parser;
use origin_primitives::{element::ElementType, Ss58Identifier};
use origin_sdk::{
	client::{signer::MultiKeySigner, Signer},
	extrinsic::builder::DynamicCall,
	util::codec::element_value_from_json,
	OriginClient,
};
use scale_value::Value;
use serde_json::Value as Json;
use subxt::utils::AccountId32;

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9910")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(long, help = "submit via meta-tx instead of direct signer")]
	meta: bool,
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
	let bytes: [u8; 32] = account.into();
	let account32 = AccountId32::from(bytes);
	let token_opt = client.view()?.entity().account_token(account32).await?;

	let entity_id = if let Some(id) = token_opt {
		println!("Entity exists: {}", id.to_string_lossy());
		rotate_some(&client, &signer, args.meta, &id, &data).await?;
		id
	} else {
		let id = create_entity(&client, &signer, args.meta, &data).await?;
		println!("Created entity: {}", id.to_string_lossy());
		id
	};

	let overview = client.view()?.entity().overview(entity_id).await?;
	println!("Entity overview: {:?}", overview);
	Ok(())
}

async fn create_entity(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	data: &Json,
) -> Result<Ss58Identifier, Box<dyn std::error::Error>> {
	let attrs = data["attributes"].as_object().ok_or("attributes missing")?;
	for (k, v) in attrs {
		submit_attr(client, signer, meta, None, k, v).await?;
	}
	if let Some(nym) = data["nym"].as_str() {
		let call = DynamicCall {
			pallet: "Entity".into(),
			function: "set_entity_nym".into(),
			args: vec![Value::from_bytes(nym.as_bytes())],
		};
		submit_call(client, signer, meta, call).await?;
	}
	let id = client
		.view()?
		.entity()
		.account_token({
			let b: [u8; 32] = signer.account_id().into();
			AccountId32::from(b)
		})
		.await?
		.ok_or("entity id not found after creation")?;
	Ok(id)
}

async fn rotate_some(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	entity: &Ss58Identifier,
	data: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let attrs = data["attributes"].as_object().ok_or("attributes missing")?;
	for (k, v) in attrs.iter().take(3) {
		submit_attr(client, signer, meta, Some(entity), k, v).await?;
	}
	Ok(())
}

async fn submit_attr(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	entity: Option<&Ss58Identifier>,
	key: &str,
	json: &Json,
) -> Result<(), Box<dyn std::error::Error>> {
	let etype = parse_type(json.get("type").and_then(Json::as_str).unwrap_or("raw"));
	let val_json = json.get("value").unwrap_or(json);
	let elem = element_value_from_json(etype, val_json)?;
	let target = entity.cloned().unwrap_or_else(dummy_entity);
	let call = DynamicCall {
		pallet: "Entity".into(),
		function: "rotate_attribute".into(),
		args: vec![Value::from_bytes(target.as_ref()), Value::from_bytes(key.as_bytes()), elem],
	};
	submit_call(client, signer, meta, call).await
}

async fn submit_call(
	client: &OriginClient,
	signer: &MultiKeySigner,
	meta: bool,
	call: DynamicCall,
) -> Result<(), Box<dyn std::error::Error>> {
	if meta {
		let _handle = client.metatx_with(signer.clone()).sign_submit_and_wait_checked(call).await?;
		println!("meta-tx finalized");
	} else {
		let handle = client
			.tx_with(signer.clone())
			.submit(&call.pallet, &call.function, call.args.clone())
			.await?;
		let res = handle.wait_in_block().await?;
		println!("in block {:?}", res.block);
	}
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

fn dummy_entity() -> Ss58Identifier {
	Ss58Identifier::try_from("5C8F41pKK9PXJ6A4ppfUT6asDkDw7py3AhtTx5xNUFDXL9Xb".to_string())
		.expect("valid")
}
