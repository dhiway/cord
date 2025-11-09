use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use origin::{
	tx::{self, TxOptions},
	types::entity::{AttributeEntry, ElementJson},
};

#[tokio::main]
async fn main() -> Result<()> {
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();

	let entry = AttributeEntry {
		key_hex: origin::types::to_key_hex_from_utf8("email"),
		key_utf8: Some("email".into()),
		value: ElementJson::RawBase64(BASE64.encode("alice@example.com")),
	};

	let call = client.tx().entity_rotate_attribute(entry).await?;
	let _in_block = client.tx().sign_and_submit(call, &signer, TxOptions::default()).await?;
	println!("entity attribute rotated for Alice");
	Ok(())
}
