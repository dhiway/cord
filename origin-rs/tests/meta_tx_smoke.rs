//! Ignored integration smoke test: signs a meta-tx and relays it through a local dev node.
//! Run manually with a node started, e.g.:
//! `./target/release/cord --dev --tmp` and then:
//! `cargo test -p origin-sdk meta_tx_roundtrip -- --ignored --nocapture`

use oc::{
	client::{signer::MultiKeySigner, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
};
use subxt::dynamic::Value;

const DEFAULT_WS: &str = "ws://127.0.0.1:9944";

#[tokio::test(flavor = "multi_thread")]
#[ignore] // Requires a running dev node.
async fn meta_tx_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
	let endpoint = std::env::var("CORD_WS").unwrap_or_else(|_| DEFAULT_WS.to_string());
	let client = OriginClient::connect(endpoint).await?;

	// Signer (meta author) and relayer (fee payer).
	let alice = MultiKeySigner::from_seed("//Alice")?;
	let bob = MultiKeySigner::from_seed("//Bob")?;

	// Inner call: system.remark_with_event(b"meta-tx-smoke").
	let inner = DynamicCallBuilder::new().call(
		"System",
		"remark_with_event",
		vec![Value::from_bytes(b"meta-tx-smoke".to_vec())],
	);

	// Alice signs a meta-tx.
	let signed = client.meta_tx().using(alice.clone()).prepare_and_sign(inner.clone()).await?;
	let wire = signed.encode();

	// Relayer decodes from wire bytes with runtime metadata.
	let signed_for_relay =
		oc::tx::meta::SignedMetaTx::decode_with_metadata(&client.metadata(), wire)?;

	// Bob pays and dispatches.
	let handle = client.meta_tx().using(bob).submit_signed(signed_for_relay).await?;
	handle.wait_finalized().await?;
	Ok(())
}
