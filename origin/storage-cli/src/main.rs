use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use codec::Encode;
use oc::{
	client::signer::OriginSigner,
	types::{build_register_blob_authorization, ss58_to_account_id, OriginAccount},
	OriginClient,
};
use sp_core::H256;
use sp_crypto_hashing::blake2_256;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "origin-storage")]
struct Args {
	#[arg(long, default_value = "ws://127.0.0.1:9944")]
	endpoint: String,

	#[command(subcommand)]
	cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
	/// Store a file in local FS and register it on-chain (publisher-submitted, owner-authorized).
	Store {
		/// Publisher signing seed (e.g. //Alice for dev).
		#[arg(long)]
		publisher_seed: String,

		/// Owner signing seed (used only to sign the authorization payload).
		#[arg(long)]
		owner_seed: String,

		/// Owner SS58 address (must match owner_seed).
		#[arg(long)]
		owner: String,

		/// Path to file to store.
		#[arg(long)]
		path: PathBuf,

		/// Destination directory for local FS blob storage.
		#[arg(long, default_value = "./blobs")]
		data_dir: PathBuf,

		/// Nonce for authorization payload (replay protection).
		#[arg(long, default_value_t = 0)]
		nonce: u64,
	},
}

#[tokio::main]
async fn main() -> Result<()> {
	tracing_subscriber::fmt().with_env_filter("info").init();
	let args = Args::parse();

	match args.cmd {
		Cmd::Store { publisher_seed, owner_seed, owner, path, data_dir, nonce } => {
			run_store(
				&args.endpoint,
				&publisher_seed,
				&owner_seed,
				&owner,
				&path,
				&data_dir,
				nonce,
			)
			.await?;
		},
	}
	Ok(())
}

async fn run_store(
	endpoint: &str,
	publisher_seed: &str,
	owner_seed: &str,
	owner_ss58: &str,
	path: &Path,
	data_dir: &Path,
	nonce: u64,
) -> Result<()> {
	let client = OriginClient::connect(endpoint).await?;

	let publisher_account =
		OriginAccount::from_uri(publisher_seed, None).with_context(|| "parse publisher_seed")?;
	let publisher_signer = OriginSigner::from_account(&publisher_account)
		.map_err(|e| anyhow::anyhow!("publisher signer: {e}"))?;

	let owner_account =
		OriginAccount::from_uri(owner_seed, None).with_context(|| "parse owner_seed")?;
	let owner_signer = OriginSigner::from_account(&owner_account)
		.map_err(|e| anyhow::anyhow!("owner signer: {e}"))?;

	let owner_expected = ss58_to_account_id(owner_ss58).with_context(|| "parse owner ss58")?;
	if owner_expected != owner_account.account_id() {
		warn!("owner SS58 does not match owner_seed account_id; continuing anyway");
	}

	let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
	let size_bytes = bytes.len() as u64;
	let blob_id = H256::from(blake2_256(&bytes));
	let root_hash = blob_id;
	let checksum = blob_id;

	std::fs::create_dir_all(data_dir).with_context(|| format!("create {}", data_dir.display()))?;
	let dest = data_dir.join(format!("{:x}.blob", blob_id));
	std::fs::write(&dest, &bytes).with_context(|| format!("write {}", dest.display()))?;

	let reference_block = client
		.online()
		.blocks()
		.at_latest()
		.await
		.context("fetch latest block")?
		.number();
	let reference_block_u32: u32 = reference_block
		.try_into()
		.map_err(|_| anyhow::anyhow!("block number overflow"))?;

	let auth = build_register_blob_authorization(
		&owner_signer,
		&publisher_signer.account_id(),
		blob_id,
		root_hash,
		size_bytes,
		0u8,
		nonce,
		reference_block_u32,
	)
	.await;

	info!("stored locally: blob_id=0x{:x} size={} path={}", blob_id, size_bytes, dest.display());

	let tx = client.tx().using(publisher_signer.clone());
	let owner_encoded = owner_expected.encode();
	let owner_bytes: [u8; 32] = owner_encoded
		.as_slice()
		.try_into()
		.map_err(|_| anyhow::anyhow!("invalid owner account length"))?;
	let owner_account_id: origin_primitives::AccountId = owner_bytes.into();

	let handle = tx
		.blob_store()
		.submit_register_blob_by_publisher(
			&owner_account_id,
			blob_id,
			root_hash,
			size_bytes,
			0u8,
			&auth,
		)
		.await?;
	let outcome = handle.wait_in_block().await?;
	info!("register_blob_by_publisher included: {:?}", outcome.hash);

	let handle = tx.blob_store().submit_confirm_stored(blob_id, checksum).await?;
	let outcome = handle.wait_in_block().await?;
	info!("confirm_stored included: {:?}", outcome.hash);

	Ok(())
}
