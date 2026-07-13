// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed provider outbox consumer using the CORD Origin SDK Orbis finality pipeline.

use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
	time::Duration,
};

use clap::Parser;
use oc::{
	product_sdk::{
		domains::{
			common::{
				AccountId, AgreementId, ChallengeId, ContentCommitment, ProofCommitment,
				SubmitAndFinalize,
			},
			storage_provider::StorageProviderCommand,
		},
		OrbisDomainTransport, OrbisNativeClient,
	},
	types::account::{account_id_to_ss58, CryptoScheme, OriginAccount},
	OriginSdkError, OriginSigner,
};
use origin_orbis_provider::ProviderSubmission;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Parser)]
#[command(
	name = "origin-orbis-provider-outbox",
	about = "Submit native provider outbox calls and wait for Orbis finality"
)]
struct Cli {
	/// Orbis WebSocket RPC endpoint.
	#[arg(long, default_value = "ws://127.0.0.1:9944")]
	orbis_rpc: String,
	/// Provider JSONL outbox emitted by origin-orbis-provider.
	#[arg(long)]
	outbox: PathBuf,
	/// Fsynced finalized receipt journal. Defaults beside the outbox.
	#[arg(long)]
	receipts: Option<PathBuf>,
	/// Environment variable containing the provider account secret URI.
	#[arg(long, default_value = "ORBIS_PROVIDER_ACCOUNT_SURI")]
	signer_suri_env: String,
	/// Process existing work once and exit instead of polling.
	#[arg(long)]
	once: bool,
	/// Poll interval when not using `--once`.
	#[arg(long, default_value_t = 3)]
	poll_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FinalizedReceipt {
	key: String,
	block_hash: String,
	extrinsic_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();
	let receipts = cli
		.receipts
		.clone()
		.unwrap_or_else(|| cli.outbox.with_extension("receipts-v1.jsonl"));
	let suri = std::env::var(&cli.signer_suri_env).map_err(|_| {
		format!("{} must contain the provider account secret URI", cli.signer_suri_env)
	})?;
	let account = OriginAccount::from_uri(&suri, Some(CryptoScheme::Sr25519))?;
	let signer = OriginSigner::from_account(&account)?;
	let signer_id = AccountId::new(account_id_to_ss58(&signer.account_id()))?;
	let client = OrbisNativeClient::connect(&cli.orbis_rpc).await.map_err(native_error)?;
	let transport = OrbisDomainTransport::new(client, signer);
	loop {
		consume(&cli.outbox, &receipts, &signer_id, &transport).await?;
		if cli.once {
			break;
		}
		tokio::time::sleep(Duration::from_secs(cli.poll_seconds.max(1))).await;
	}
	Ok(())
}

async fn consume(
	outbox: &Path,
	receipts: &Path,
	signer: &AccountId,
	transport: &OrbisDomainTransport,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut completed = completed_keys(receipts).await?;
	let contents = match tokio::fs::read_to_string(outbox).await {
		Ok(contents) => contents,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(error.into()),
	};
	for (line_number, line) in contents.lines().enumerate() {
		if line.trim().is_empty() {
			continue;
		}
		let request: ProviderSubmission = serde_json::from_str(line)
			.map_err(|error| format!("invalid outbox line {}: {error}", line_number + 1))?;
		let (key, command) = command(request)?;
		if completed.contains(&key) {
			continue;
		}
		let intent =
			SubmitAndFinalize::new(format!("orbis-provider-outbox-{key}"), signer.clone(), command)
				.map_err(native_error)?;
		let lifecycle = transport.submit_storage_provider(&intent).await.map_err(native_error)?;
		let receipt = FinalizedReceipt {
			key,
			block_hash: lifecycle.block_hash.ok_or("finalized provider call has no block hash")?,
			extrinsic_hash: lifecycle
				.extrinsic_hash
				.ok_or("finalized provider call has no extrinsic hash")?,
		};
		append_receipt(receipts, &receipt).await?;
		completed.insert(receipt.key);
	}
	Ok(())
}

fn command(
	request: ProviderSubmission,
) -> Result<(String, StorageProviderCommand), Box<dyn std::error::Error>> {
	match request {
		ProviderSubmission::Checkpoint(request) => {
			let key = format!("checkpoint-{}", request.duty.challenge_id);
			Ok((
				key,
				StorageProviderCommand::SubmitCheckpoint {
					challenge: ChallengeId::new(request.duty.challenge_id).map_err(native_error)?,
					proof_commitment: ProofCommitment::new(request.proof_commitment)
						.map_err(native_error)?,
				},
			))
		},
		ProviderSubmission::ContentDeletion(request) => {
			let key = format!("deletion-{}", request.agreement_id);
			Ok((
				key,
				StorageProviderCommand::AcknowledgeDeletion {
					agreement: AgreementId::new(request.agreement_id).map_err(native_error)?,
					content_commitment: ContentCommitment::new(canonical_hash(
						request.content_commitment,
					)?)
					.map_err(native_error)?,
					tombstone_root: ProofCommitment::new(canonical_hash(request.tombstone_root)?)
						.map_err(native_error)?,
					proof_commitment: ProofCommitment::new(canonical_hash(
						request.proof_commitment,
					)?)
					.map_err(native_error)?,
				},
			))
		},
	}
}

fn canonical_hash(value: String) -> Result<String, Box<dyn std::error::Error>> {
	let normalized = value.strip_prefix("0x").unwrap_or(&value).to_ascii_lowercase();
	if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err("provider outbox hash must contain exactly 32 hex bytes".into());
	}
	Ok(format!("0x{normalized}"))
}

async fn completed_keys(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
	let contents = match tokio::fs::read_to_string(path).await {
		Ok(contents) => contents,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
		Err(error) => return Err(error.into()),
	};
	contents
		.lines()
		.filter(|line| !line.trim().is_empty())
		.map(|line| {
			serde_json::from_str::<FinalizedReceipt>(line)
				.map(|receipt| receipt.key)
				.map_err(Into::into)
		})
		.collect()
}

async fn append_receipt(
	path: &Path,
	receipt: &FinalizedReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
	if let Some(parent) = path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	let mut encoded = serde_json::to_vec(receipt)?;
	encoded.push(b'\n');
	let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open(path).await?;
	file.write_all(&encoded).await?;
	file.sync_data().await?;
	Ok(())
}

fn native_error(error: impl std::fmt::Display) -> OriginSdkError {
	OriginSdkError::InvalidInput(error.to_string())
}
