// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

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
		.unwrap_or_else(|| cli.outbox.with_extension("receipts-v3.jsonl"));
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
	let Some(contents) = read_complete_jsonl_snapshot(outbox).await? else { return Ok(()) };
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

async fn read_complete_jsonl_snapshot(
	path: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
	let bytes = match tokio::fs::read(path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => return Err(error.into()),
	};
	let complete_len = if bytes.ends_with(b"\n") {
		bytes.len()
	} else {
		bytes.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1)
	};
	Ok(Some(String::from_utf8(bytes[..complete_len].to_vec())?))
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
					root_sequence: request.root_sequence,
					leaf_index: request.leaf_index,
					leaf_count: request.leaf_count,
					inclusion_proof: request
						.inclusion_proof
						.into_iter()
						.map(|hash| -> Result<_, Box<dyn std::error::Error>> {
							let hash = canonical_hash(hash)?;
							Ok(ProofCommitment::new(hash).map_err(native_error)?)
						})
						.collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
				},
			))
		},
		ProviderSubmission::ProviderRoot(request) => {
			let key = format!("provider-root-{}", request.sequence);
			let appended_leaves = request
				.appended_leaves
				.into_iter()
				.map(|hash| -> Result<_, Box<dyn std::error::Error>> {
					Ok(ProofCommitment::new(canonical_hash(hash)?).map_err(native_error)?)
				})
				.collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
			Ok((
				key,
				StorageProviderCommand::CommitProviderRoot {
					sequence: request.sequence,
					appended_leaves,
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
	repair_incomplete_jsonl_tail(path).await?;
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
	append_receipt_with_parent_sync(path, receipt, |parent| async move {
		tokio::fs::File::open(parent).await?.sync_all().await?;
		Ok::<(), Box<dyn std::error::Error>>(())
	})
	.await
}

async fn append_receipt_with_parent_sync<F, Fut>(
	path: &Path,
	receipt: &FinalizedReceipt,
	sync_parent: F,
) -> Result<(), Box<dyn std::error::Error>>
where
	F: FnOnce(PathBuf) -> Fut,
	Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
	if let Some(parent) = path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	repair_incomplete_jsonl_tail(path).await?;
	let mut encoded = serde_json::to_vec(receipt)?;
	encoded.push(b'\n');
	let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open(path).await?;
	file.write_all(&encoded).await?;
	file.sync_data().await?;
	let parent = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or_else(|| Path::new("."));
	sync_parent(parent.to_path_buf()).await
}

async fn repair_incomplete_jsonl_tail(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let bytes = match tokio::fs::read(path).await {
		Ok(bytes) => bytes,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
		Err(error) => return Err(error.into()),
	};
	if bytes.is_empty() || bytes.ends_with(b"\n") {
		return Ok(());
	}
	let keep = bytes.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1);
	let file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
	file.set_len(keep as u64).await?;
	file.sync_all().await?;
	Ok(())
}

fn native_error(error: impl std::fmt::Display) -> OriginSdkError {
	OriginSdkError::InvalidInput(error.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn receipt(key: &str) -> FinalizedReceipt {
		FinalizedReceipt {
			key: key.into(),
			block_hash: format!("0x{}", "01".repeat(32)),
			extrinsic_hash: format!("0x{}", "02".repeat(32)),
		}
	}

	#[tokio::test]
	async fn torn_receipt_after_finality_is_repaired_then_retry_and_later_work_continue() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("receipts-v3.jsonl");
		append_receipt(&path, &receipt("provider-root-1")).await.unwrap();
		let mut file = tokio::fs::OpenOptions::new().append(true).open(&path).await.unwrap();
		file.write_all(b"{\"key\":\"provider-root-2\"").await.unwrap();
		file.sync_all().await.unwrap();
		drop(file);

		let completed = completed_keys(&path).await.unwrap();
		assert_eq!(completed, BTreeSet::from(["provider-root-1".to_string()]));
		append_receipt(&path, &receipt("provider-root-2")).await.unwrap();
		append_receipt(&path, &receipt("provider-root-3")).await.unwrap();
		assert_eq!(
			completed_keys(&path).await.unwrap(),
			BTreeSet::from([
				"provider-root-1".to_string(),
				"provider-root-2".to_string(),
				"provider-root-3".to_string(),
			]),
		);
	}

	#[tokio::test]
	async fn malformed_complete_receipt_fails_closed() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("receipts-v3.jsonl");
		tokio::fs::write(&path, b"not-json\n").await.unwrap();
		assert!(completed_keys(&path).await.is_err());
	}

	#[tokio::test]
	async fn receipt_never_reports_completion_before_parent_directory_sync() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("receipts-v3.jsonl");
		let result =
			append_receipt_with_parent_sync(&path, &receipt("provider-root-1"), |_| async {
				Err::<(), Box<dyn std::error::Error>>(
					"injected receipt parent directory sync failure".into(),
				)
			})
			.await;
		assert_eq!(
			result.unwrap_err().to_string(),
			"injected receipt parent directory sync failure"
		);
		assert!(tokio::fs::read(&path).await.unwrap().ends_with(b"\n"));
	}

	#[tokio::test]
	async fn live_outbox_snapshot_ignores_partial_tail_without_truncating_producer() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("provider-submissions-v3.jsonl");
		tokio::fs::write(&path, b"{\"first\":1}\n{\"second\":").await.unwrap();
		assert_eq!(read_complete_jsonl_snapshot(&path).await.unwrap().unwrap(), "{\"first\":1}\n");
		assert_eq!(tokio::fs::read(&path).await.unwrap(), b"{\"first\":1}\n{\"second\":");
		let mut file = tokio::fs::OpenOptions::new().append(true).open(&path).await.unwrap();
		file.write_all(b"2}\n").await.unwrap();
		file.sync_all().await.unwrap();
		assert_eq!(
			read_complete_jsonl_snapshot(&path).await.unwrap().unwrap(),
			"{\"first\":1}\n{\"second\":2}\n"
		);
	}
}
