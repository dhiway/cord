use codec::Decode;
use oc::{client::Client, flavors::ChainFlavor, ConnectionConfig};
use sp_core::blake2_256;
use std::{env, fs};
use subxt::Metadata;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut node = "ws://127.0.0.1:9944".to_string();
	let mut file: Option<String> = None;
	let mut iter = env::args().skip(1);
	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--node" => {
				if let Some(val) = iter.next() {
					node = val
				}
			},
			"--file" => {
				if let Some(val) = iter.next() {
					file = Some(val)
				}
			},
			_ => {},
		}
	}

	let (metadata, raw_bytes) = if let Some(path) = file {
		let bytes = fs::read(&path)?;
		let meta = Metadata::decode(&mut &bytes[..])?;
		(meta, bytes)
	} else {
		let config = ConnectionConfig::new(node, ChainFlavor::Auto).skip_view_validation(); // diagnostic helper
		let client = Client::connect_with(config).await?;
		let blob = client.fetch_metadata_blob().await?;
		let meta = client.metadata();
		(meta, blob)
	};

	println!("Metadata hash: 0x{}", hex::encode(blake2_256(&raw_bytes)));

	for (pallet, views) in [
		(
			"Entity",
			["account_token", "details", "linked_accounts", "overview", "attribute_history"]
				.as_slice(),
		),
		("Register", ["details", "packet_snapshot"].as_slice()),
		("Token", ["timeline", "resolve_identifier"].as_slice()),
	] {
		for view in views {
			print_view(&metadata, pallet, view);
		}
	}
	Ok(())
}

fn print_view(metadata: &Metadata, pallet: &str, function: &str) {
	let Some(pallet_meta) = metadata.pallet_by_name(pallet) else {
		println!("{pallet}.{function}: pallet missing");
		return;
	};
	let Some(view) = pallet_meta.view_function_by_name(function) else {
		println!("{pallet}.{function}: view missing");
		return;
	};
	let ty_id = view.output_ty();
	let ty = metadata.types().resolve(ty_id).expect("type");
	println!("{pallet}.{function} -> {:?}", ty.path);
	println!("  type id: {ty_id}");
	println!("  type def: {:?}\n", ty.type_def);
}
