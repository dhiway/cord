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

//! Bounded, crash-safe provider outbox consumer for the Orbis finality pipeline.

use std::{
	fs::{self, File, OpenOptions},
	io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	time::Duration,
};

use clap::Parser;
use fs4::FileExt;
use oc::{
	product_sdk::{
		domains::{
			common::{AccountId, ContentCommitment, ProofCommitment, SubmitAndFinalize},
			storage_provider::{ServiceKey, StorageProviderCommand},
		},
		NativeLifecycle, OrbisDomainTransport, OrbisNativeClient,
	},
	types::account::{account_id_to_ss58, CryptoScheme, OriginAccount},
	OriginSdkError, OriginSigner,
};
use origin_orbis_provider::ProviderSubmission;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const STATE_VERSION: u16 = 4;
const MAX_RECORD_BYTES: usize = 1024 * 1024;
const MAX_BATCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_BATCH_RECORDS: usize = 32;
const MAX_STATE_BYTES: u64 = 2 * 1024 * 1024 + 64 * 1024;
const MAX_RECEIPTS: usize = 256;
const COMPACT_AFTER_BYTES: u64 = 4 * 1024 * 1024;
const SOURCE_PREFIX_BYTES: u64 = 64 * 1024;

#[async_trait::async_trait]
trait ProviderOutboxTransport: Send + Sync {
	async fn submit(
		&self,
		intent: &SubmitAndFinalize<StorageProviderCommand>,
	) -> Result<NativeLifecycle, String>;
}

#[async_trait::async_trait]
impl ProviderOutboxTransport for OrbisDomainTransport {
	async fn submit(
		&self,
		intent: &SubmitAndFinalize<StorageProviderCommand>,
	) -> Result<NativeLifecycle, String> {
		self.submit_storage_provider(intent).await.map_err(|error| error.to_string())
	}
}

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
	/// Bounded fsynced finalized receipt ledger. Defaults beside the outbox.
	#[arg(long)]
	receipts: Option<PathBuf>,
	/// Environment variable containing the provider account secret URI.
	#[arg(long, default_value = "ORBIS_PROVIDER_ACCOUNT_SURI")]
	signer_suri_env: String,
	/// Process one bounded batch and exit instead of polling.
	#[arg(long)]
	once: bool,
	/// Poll interval when not using `--once`.
	#[arg(long, default_value_t = 3)]
	poll_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceId {
	device: u64,
	inode: u64,
}

#[cfg(unix)]
fn source_id(metadata: &fs::Metadata) -> Result<SourceId, Box<dyn std::error::Error>> {
	use std::os::unix::fs::MetadataExt;
	Ok(SourceId { device: metadata.dev(), inode: metadata.ino() })
}

#[cfg(not(unix))]
fn source_id(_metadata: &fs::Metadata) -> Result<SourceId, Box<dyn std::error::Error>> {
	Err("provider outbox source identity is unsupported on this platform".into())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
	version: u16,
	source: Option<SourceId>,
	offset: u64,
	prefix_len: u64,
	prefix_hash: String,
}

impl Default for Cursor {
	fn default() -> Self {
		Self {
			version: STATE_VERSION,
			source: None,
			offset: 0,
			prefix_len: 0,
			prefix_hash: blake3::hash(&[]).to_hex().to_string(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FinalizedReceipt {
	key: String,
	record_hash: String,
	source: SourceId,
	start: u64,
	end: u64,
	prefix_len: u64,
	prefix_hash: String,
	block_hash: String,
	extrinsic_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReceiptLedger {
	version: u16,
	entries: Vec<FinalizedReceipt>,
}

impl Default for ReceiptLedger {
	fn default() -> Self {
		Self { version: STATE_VERSION, entries: Vec::new() }
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingRecord {
	version: u16,
	source: SourceId,
	start: u64,
	end: u64,
	record_hash: String,
	prefix_len: u64,
	prefix_hash: String,
	key: String,
	line: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CompactionMarker {
	version: u16,
	old_source: SourceId,
	consumed_len: u64,
	prefix_len: u64,
	prefix_hash: String,
}

struct StatePaths {
	receipts: PathBuf,
	cursor: PathBuf,
	pending: PathBuf,
	compaction: PathBuf,
	consumer_lock: PathBuf,
}

impl StatePaths {
	fn new(receipts: &Path) -> Self {
		Self {
			receipts: receipts.to_path_buf(),
			cursor: suffixed(receipts, ".cursor"),
			pending: suffixed(receipts, ".pending"),
			compaction: suffixed(receipts, ".compaction"),
			consumer_lock: suffixed(receipts, ".consumer.lock"),
		}
	}
}

struct LockedFile(File);

impl Drop for LockedFile {
	fn drop(&mut self) {
		let _ = FileExt::unlock(&self.0);
	}
}

fn exclusive_lock(path: &Path) -> Result<LockedFile, Box<dyn std::error::Error>> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}
	let file = OpenOptions::new().create(true).read(true).write(true).open(path)?;
	FileExt::lock_exclusive(&file)?;
	Ok(LockedFile(file))
}

fn exclusive_existing_lock(path: &Path) -> Result<LockedFile, Box<dyn std::error::Error>> {
	use rustix::fs::{Mode, OFlags};

	let path_metadata = fs::symlink_metadata(path)?;
	if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
		return Err("canonical provider outbox lock is not a regular file".into())
	}
	let file = File::from(rustix::fs::open(
		path,
		OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
		Mode::empty(),
	)?);
	let identity = source_id(&file.metadata()?)?;
	if source_id(&path_metadata)? != identity {
		return Err("canonical provider outbox lock changed while opening".into())
	}
	FileExt::lock_exclusive(&file)?;
	let current = fs::symlink_metadata(path)?;
	if current.file_type().is_symlink() || !current.is_file() || source_id(&current)? != identity {
		FileExt::unlock(&file)?;
		return Err("canonical provider outbox lock changed while acquiring".into())
	}
	Ok(LockedFile(file))
}

#[derive(Debug)]
struct OutboxRecord {
	source: SourceId,
	start: u64,
	end: u64,
	line: Vec<u8>,
	prefix_len: u64,
	prefix_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();
	let receipts = cli
		.receipts
		.clone()
		.unwrap_or_else(|| cli.outbox.with_extension("receipts-v4.json"));
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

async fn consume<T: ProviderOutboxTransport>(
	outbox: &Path,
	receipts: &Path,
	signer: &AccountId,
	transport: &T,
) -> Result<(), Box<dyn std::error::Error>> {
	let paths = StatePaths::new(receipts);
	let _consumer = exclusive_lock(&paths.consumer_lock)?;
	let outbox_lock_path = suffixed(outbox, ".lock");
	// One exact, no-follow canonical lock covers the complete consume transaction. Helper phases
	// never reopen or create the provider-owned coordination file.
	let _outbox_lock = exclusive_existing_lock(&outbox_lock_path)?;
	let mut cursor = load_json::<Cursor>(&paths.cursor)?.unwrap_or_default();
	let mut ledger = load_json::<ReceiptLedger>(&paths.receipts)?.unwrap_or_default();
	let pending = load_json::<PendingRecord>(&paths.pending)?;
	let marker = load_json::<CompactionMarker>(&paths.compaction)?;
	validate_cursor(&cursor)?;
	validate_ledger(&ledger)?;
	if let Some(pending) = &pending {
		validate_pending(pending)?;
	}
	if let Some(marker) = &marker {
		validate_marker(marker)?;
	}
	recover_compaction(
		outbox,
		&outbox_lock_path,
		&paths,
		&mut cursor,
		marker.as_ref(),
		pending.is_some(),
	)?;

	if let Some(pending) = pending {
		verify_pending_source(outbox, &outbox_lock_path, &pending)?;
		if !recover_local_finality(&paths, &mut cursor, &ledger, &pending)? {
			finalize_pending(
				outbox,
				&outbox_lock_path,
				&paths,
				&mut cursor,
				&mut ledger,
				pending,
				signer,
				transport,
			)
			.await?;
		}
	}

	let batch = read_batch(outbox, &outbox_lock_path, &paths.cursor, &mut cursor)?;
	for record in batch {
		let line = String::from_utf8(record.line.clone())?;
		let request: ProviderSubmission = serde_json::from_str(line.trim_end_matches('\n'))?;
		let (key, _) = command(request)?;
		let pending = PendingRecord {
			version: STATE_VERSION,
			source: record.source,
			start: record.start,
			end: record.end,
			record_hash: blake3::hash(&record.line).to_hex().to_string(),
			prefix_len: record.prefix_len,
			prefix_hash: record.prefix_hash,
			key,
			line,
		};
		atomic_json(&paths.pending, &pending)?;
		finalize_pending(
			outbox,
			&outbox_lock_path,
			&paths,
			&mut cursor,
			&mut ledger,
			pending,
			signer,
			transport,
		)
		.await?;
	}
	compact_if_drained(outbox, &outbox_lock_path, &paths, &mut cursor, COMPACT_AFTER_BYTES)?;
	Ok(())
}

fn recover_local_finality(
	paths: &StatePaths,
	cursor: &mut Cursor,
	ledger: &ReceiptLedger,
	pending: &PendingRecord,
) -> Result<bool, Box<dyn std::error::Error>> {
	validate_cursor(cursor)?;
	validate_ledger(ledger)?;
	validate_pending(pending)?;
	if cursor.source != Some(pending.source)
		|| cursor.prefix_len != pending.prefix_len
		|| cursor.prefix_hash != pending.prefix_hash
	{
		return Err("pending provider outbox cursor binding changed".into());
	}
	let finalized = ledger.entries.iter().any(|entry| receipt_matches_pending(entry, pending));
	if cursor.offset == pending.end {
		if !finalized {
			return Err("pending provider outbox end cursor has no exact finalized receipt".into());
		}
		remove_durable(&paths.pending)?;
		return Ok(true);
	}
	if cursor.offset != pending.start {
		return Err("pending provider outbox cursor is not at its exact start or end".into());
	}
	if finalized {
		advance_cursor(&paths.cursor, cursor, pending)?;
		remove_durable(&paths.pending)?;
		return Ok(true);
	}
	Ok(false)
}

async fn finalize_pending<T: ProviderOutboxTransport>(
	outbox: &Path,
	outbox_lock_path: &Path,
	paths: &StatePaths,
	cursor: &mut Cursor,
	ledger: &mut ReceiptLedger,
	pending: PendingRecord,
	signer: &AccountId,
	transport: &T,
) -> Result<(), Box<dyn std::error::Error>> {
	validate_pending(&pending)?;
	verify_pending_source(outbox, outbox_lock_path, &pending)?;
	let request: ProviderSubmission = serde_json::from_str(pending.line.trim_end_matches('\n'))?;
	let (key, command) = command(request)?;
	if key != pending.key {
		return Err("pending provider outbox key changed".into());
	}
	let intent = SubmitAndFinalize::new(
		format!("orbis-provider-outbox-{}", pending.record_hash),
		signer.clone(),
		command,
	)
	.map_err(native_error)?;
	let lifecycle = transport.submit(&intent).await.map_err(native_error)?;
	let receipt = FinalizedReceipt {
		key,
		record_hash: pending.record_hash.clone(),
		source: pending.source,
		start: pending.start,
		end: pending.end,
		prefix_len: pending.prefix_len,
		prefix_hash: pending.prefix_hash.clone(),
		block_hash: lifecycle.block_hash.ok_or("finalized provider call has no block hash")?,
		extrinsic_hash: lifecycle
			.extrinsic_hash
			.ok_or("finalized provider call has no extrinsic hash")?,
	};
	verify_pending_source(outbox, outbox_lock_path, &pending)?;
	record_receipt(&paths.receipts, ledger, receipt)?;
	advance_cursor(&paths.cursor, cursor, &pending)?;
	remove_durable(&paths.pending)?;
	Ok(())
}

fn command(
	request: ProviderSubmission,
) -> Result<(String, StorageProviderCommand), Box<dyn std::error::Error>> {
	match request {
		ProviderSubmission::ManifestDeletion(request) => {
			let key = format!(
				"manifest-deletion-{}-{}-{}",
				request.manifest, request.duty_fingerprint, request.evidence_hash,
			);
			let service_key =
				decode_prefixed(&request.service_key, "manifest deletion service key")?;
			let signature = decode_prefixed(&request.signature, "manifest deletion signature")?;
			Ok((
				key,
				StorageProviderCommand::AcknowledgeManifestDeletion {
					manifest: ContentCommitment::new(canonical_hash(request.manifest)?)
						.map_err(native_error)?,
					evidence_hash: ProofCommitment::new(canonical_hash(request.evidence_hash)?)
						.map_err(native_error)?,
					service_key: ServiceKey::new(service_key).map_err(native_error)?,
					signature,
				},
			))
		},
	}
}

fn decode_prefixed(value: &str, field: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
	let encoded = value.strip_prefix("0x").ok_or_else(|| format!("{field} is not 0x-prefixed"))?;
	Ok(hex::decode(encoded)?)
}

fn canonical_hash(value: String) -> Result<String, Box<dyn std::error::Error>> {
	let normalized = value.strip_prefix("0x").unwrap_or(&value).to_ascii_lowercase();
	if normalized.len() != 64 || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err("provider outbox hash must contain exactly 32 hex bytes".into());
	}
	Ok(format!("0x{normalized}"))
}

fn validate_pending(pending: &PendingRecord) -> Result<(), Box<dyn std::error::Error>> {
	ensure_version(pending.version)?;
	if pending.line.len() > MAX_RECORD_BYTES || !pending.line.ends_with('\n') {
		return Err("pending provider outbox record is not one bounded complete line".into());
	}
	if pending.start >= pending.end || pending.end - pending.start != pending.line.len() as u64 {
		return Err("pending provider outbox offsets do not match its exact bytes".into());
	}
	if !is_hash(&pending.record_hash)
		|| blake3::hash(pending.line.as_bytes()).to_hex().as_str() != pending.record_hash
	{
		return Err("pending provider outbox record hash mismatch".into());
	}
	if pending.prefix_len == 0
		|| pending.prefix_len > SOURCE_PREFIX_BYTES
		|| !is_hash(&pending.prefix_hash)
	{
		return Err("pending provider outbox prefix binding is invalid".into());
	}
	if pending.key.is_empty() || pending.key.len() > 512 {
		return Err("pending provider outbox key is invalid".into());
	}
	Ok(())
}

fn validate_cursor(cursor: &Cursor) -> Result<(), Box<dyn std::error::Error>> {
	ensure_version(cursor.version)?;
	if cursor.prefix_len > SOURCE_PREFIX_BYTES || !is_hash(&cursor.prefix_hash) {
		return Err("provider outbox cursor prefix binding is invalid".into());
	}
	if cursor.source.is_none()
		&& (cursor.offset != 0
			|| cursor.prefix_len != 0
			|| cursor.prefix_hash != blake3::hash(&[]).to_hex().to_string())
	{
		return Err("unbound provider outbox cursor contains source state".into());
	}
	Ok(())
}

fn validate_cursor_source(
	cursor: &Cursor,
	source: SourceId,
	source_len: u64,
) -> Result<(), Box<dyn std::error::Error>> {
	validate_cursor(cursor)?;
	if cursor.source != Some(source)
		|| cursor.offset > source_len
		|| cursor.prefix_len > source_len
		|| (source_len > 0 && cursor.prefix_len == 0)
	{
		return Err("provider outbox cursor is not bound to the exact source generation".into());
	}
	Ok(())
}

fn validate_receipt(receipt: &FinalizedReceipt) -> Result<(), Box<dyn std::error::Error>> {
	if receipt.key.is_empty()
		|| receipt.key.len() > 512
		|| !is_hash(&receipt.record_hash)
		|| receipt.start >= receipt.end
		|| receipt.end - receipt.start > MAX_RECORD_BYTES as u64
		|| receipt.prefix_len == 0
		|| receipt.prefix_len > SOURCE_PREFIX_BYTES
		|| !is_hash(&receipt.prefix_hash)
		|| !is_prefixed_hash(&receipt.block_hash)
		|| !is_prefixed_hash(&receipt.extrinsic_hash)
	{
		return Err("finalized provider receipt binding is invalid".into());
	}
	Ok(())
}

fn validate_ledger(ledger: &ReceiptLedger) -> Result<(), Box<dyn std::error::Error>> {
	ensure_version(ledger.version)?;
	if ledger.entries.len() > MAX_RECEIPTS {
		return Err("finalized provider receipt ledger exceeds its entry limit".into());
	}
	for (index, receipt) in ledger.entries.iter().enumerate() {
		validate_receipt(receipt)?;
		if ledger.entries[..index]
			.iter()
			.any(|existing| receipt_binding_matches(existing, receipt))
		{
			return Err("finalized provider receipt ledger contains duplicate bindings".into());
		}
	}
	Ok(())
}

fn validate_marker(marker: &CompactionMarker) -> Result<(), Box<dyn std::error::Error>> {
	ensure_version(marker.version)?;
	if marker.consumed_len == 0
		|| marker.prefix_len == 0
		|| marker.prefix_len > marker.consumed_len
		|| marker.prefix_len > SOURCE_PREFIX_BYTES
		|| !is_hash(&marker.prefix_hash)
	{
		return Err("provider outbox compaction marker binding is invalid".into());
	}
	Ok(())
}

fn receipt_matches_pending(receipt: &FinalizedReceipt, pending: &PendingRecord) -> bool {
	receipt.key == pending.key
		&& receipt.record_hash == pending.record_hash
		&& receipt.source == pending.source
		&& receipt.start == pending.start
		&& receipt.end == pending.end
		&& receipt.prefix_len == pending.prefix_len
		&& receipt.prefix_hash == pending.prefix_hash
}

fn receipt_binding_matches(left: &FinalizedReceipt, right: &FinalizedReceipt) -> bool {
	left.key == right.key
		&& left.record_hash == right.record_hash
		&& left.source == right.source
		&& left.start == right.start
		&& left.end == right.end
		&& left.prefix_len == right.prefix_len
		&& left.prefix_hash == right.prefix_hash
}

fn is_hash(value: &str) -> bool {
	value.len() == 64
		&& value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_prefixed_hash(value: &str) -> bool {
	value.strip_prefix("0x").is_some_and(is_hash)
}

fn read_batch(
	outbox: &Path,
	_lock_path: &Path,
	cursor_path: &Path,
	cursor: &mut Cursor,
) -> Result<Vec<OutboxRecord>, Box<dyn std::error::Error>> {
	validate_cursor(cursor)?;
	let open_outbox = || {
		File::open(outbox).map_err(|error| {
			if error.kind() == std::io::ErrorKind::NotFound {
				std::io::Error::new(error.kind(), "provider outbox unavailable")
			} else {
				error
			}
		})
	};
	let mut file = open_outbox()?;
	let original_metadata = file.metadata()?;
	let source = source_id(&original_metadata)?;
	if cursor.source.is_some() {
		validate_cursor_source(cursor, source, original_metadata.len())?;
		verify_source_prefix(&mut file, cursor.prefix_len, &cursor.prefix_hash)?;
	}
	let repaired_len = complete_tail_len(&mut file, original_metadata.len())?;
	if cursor.source.is_some() && (cursor.offset > repaired_len || cursor.prefix_len > repaired_len)
	{
		return Err("provider outbox cursor binding crosses its incomplete tail".into());
	}
	drop(file);
	repair_incomplete_tail_exact(outbox, source, original_metadata.len(), repaired_len)?;
	let mut file = open_outbox()?;
	let metadata = file.metadata()?;
	let repaired_source = source_id(&metadata)?;
	if repaired_source != source {
		return Err("provider outbox source changed while repairing its incomplete tail".into());
	}
	match cursor.source {
		None => {
			cursor.source = Some(repaired_source);
			cursor.offset = 0;
			assign_source_prefix(&mut file, cursor, metadata.len())?;
			atomic_json(cursor_path, cursor)?;
		},
		Some(_) => {},
	}
	validate_cursor_source(cursor, repaired_source, metadata.len())?;
	verify_source_prefix(&mut file, cursor.prefix_len, &cursor.prefix_hash)?;
	file.seek(SeekFrom::Start(cursor.offset))?;
	let mut reader = BufReader::with_capacity(8192, file);
	let mut records = Vec::new();
	let mut bytes = 0usize;
	let mut start = cursor.offset;
	while records.len() < MAX_BATCH_RECORDS {
		let Some(line) = read_bounded_line(&mut reader)? else { break };
		if bytes + line.len() > MAX_BATCH_BYTES && !records.is_empty() {
			break;
		}
		if bytes + line.len() > MAX_BATCH_BYTES {
			return Err("provider outbox record exceeds the bounded batch limit".into());
		}
		let end = start + line.len() as u64;
		bytes += line.len();
		records.push(OutboxRecord {
			source: repaired_source,
			start,
			end,
			line,
			prefix_len: cursor.prefix_len,
			prefix_hash: cursor.prefix_hash.clone(),
		});
		start = end;
	}
	Ok(records)
}

fn assign_source_prefix(
	file: &mut File,
	cursor: &mut Cursor,
	source_len: u64,
) -> Result<(), Box<dyn std::error::Error>> {
	cursor.prefix_len = source_len.min(SOURCE_PREFIX_BYTES);
	cursor.prefix_hash = hash_prefix(file, cursor.prefix_len)?;
	Ok(())
}

fn verify_source_prefix(
	file: &mut File,
	prefix_len: u64,
	expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	if hash_prefix(file, prefix_len)? != expected {
		return Err("provider outbox durable prefix hash changed".into());
	}
	Ok(())
}

fn hash_prefix(file: &mut File, len: u64) -> Result<String, Box<dyn std::error::Error>> {
	file.seek(SeekFrom::Start(0))?;
	let mut remaining = len;
	let mut hasher = blake3::Hasher::new();
	let mut buffer = [0u8; 8192];
	while remaining > 0 {
		let width = remaining.min(buffer.len() as u64) as usize;
		file.read_exact(&mut buffer[..width])?;
		hasher.update(&buffer[..width]);
		remaining -= width as u64;
	}
	Ok(hasher.finalize().to_hex().to_string())
}

fn verify_pending_source(
	outbox: &Path,
	_lock_path: &Path,
	pending: &PendingRecord,
) -> Result<(), Box<dyn std::error::Error>> {
	validate_pending(pending)?;
	let mut file = File::open(outbox)?;
	let metadata = file.metadata()?;
	if source_id(&metadata)? != pending.source
		|| metadata.len() < pending.end
		|| pending.prefix_len > metadata.len()
	{
		return Err("pending provider outbox source generation changed".into());
	}
	verify_source_prefix(&mut file, pending.prefix_len, &pending.prefix_hash)?;
	file.seek(SeekFrom::Start(pending.start))?;
	let width = (pending.end - pending.start).min(MAX_RECORD_BYTES as u64) as usize;
	let mut exact = vec![0u8; width];
	file.read_exact(&mut exact)?;
	if exact.len() != (pending.end - pending.start) as usize
		|| blake3::hash(&exact).to_hex().as_str() != pending.record_hash
	{
		return Err("pending provider outbox exact source bytes changed".into());
	}
	Ok(())
}

fn read_bounded_line<R: BufRead>(
	reader: &mut R,
) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
	let mut line = Vec::new();
	loop {
		let available = reader.fill_buf()?;
		if available.is_empty() {
			return if line.is_empty() {
				Ok(None)
			} else {
				Err("provider outbox ended with an incomplete record".into())
			};
		}
		let take = available
			.iter()
			.position(|byte| *byte == b'\n')
			.map_or(available.len(), |index| index + 1);
		if line.len() + take > MAX_RECORD_BYTES {
			return Err("provider outbox record exceeds the bounded line limit".into());
		}
		let complete = available[take - 1] == b'\n';
		line.extend_from_slice(&available[..take]);
		reader.consume(take);
		if complete {
			return Ok(Some(line));
		}
	}
}

fn repair_incomplete_tail(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let mut file = OpenOptions::new().read(true).open(path).map_err(|error| {
		if error.kind() == std::io::ErrorKind::NotFound {
			std::io::Error::new(error.kind(), "provider outbox unavailable")
		} else {
			error
		}
	})?;
	let metadata = file.metadata()?;
	let source = source_id(&metadata)?;
	let len = metadata.len();
	let keep = complete_tail_len(&mut file, len)?;
	drop(file);
	repair_incomplete_tail_exact(path, source, len, keep)
}

fn complete_tail_len(file: &mut File, len: u64) -> Result<u64, Box<dyn std::error::Error>> {
	if len == 0 {
		return Ok(0);
	}
	file.seek(SeekFrom::End(-1))?;
	let mut last = [0u8; 1];
	file.read_exact(&mut last)?;
	if last[0] == b'\n' {
		return Ok(len);
	}
	let start = len.saturating_sub(MAX_RECORD_BYTES as u64 + 1);
	file.seek(SeekFrom::Start(start))?;
	let mut tail = vec![0u8; (len - start) as usize];
	file.read_exact(&mut tail)?;
	Ok(match tail.iter().rposition(|byte| *byte == b'\n') {
		Some(index) => start + index as u64 + 1,
		None if start == 0 => 0,
		None => return Err("incomplete provider outbox tail exceeds the bounded line limit".into()),
	})
}

fn repair_incomplete_tail_exact(
	path: &Path,
	expected_source: SourceId,
	expected_len: u64,
	expected_keep: u64,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut file = OpenOptions::new().read(true).write(true).open(path)?;
	let metadata = file.metadata()?;
	if source_id(&metadata)? != expected_source || metadata.len() != expected_len {
		return Err("provider outbox source changed before incomplete-tail repair".into());
	}
	if complete_tail_len(&mut file, metadata.len())? != expected_keep {
		return Err("provider outbox incomplete tail changed before repair".into());
	}
	if expected_keep < expected_len {
		file.set_len(expected_keep)?;
		file.sync_all()?;
	}
	Ok(())
}

fn record_receipt(
	path: &Path,
	ledger: &mut ReceiptLedger,
	receipt: FinalizedReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
	validate_ledger(ledger)?;
	validate_receipt(&receipt)?;
	if let Some(existing) =
		ledger.entries.iter().find(|entry| receipt_binding_matches(entry, &receipt))
	{
		if existing != &receipt {
			return Err("finalized receipt replay changed its payload".into());
		}
		return Ok(());
	}
	ledger.entries.push(receipt);
	if ledger.entries.len() > MAX_RECEIPTS {
		let remove = ledger.entries.len() - MAX_RECEIPTS;
		ledger.entries.drain(..remove);
	}
	atomic_json(path, ledger)
}

fn advance_cursor(
	path: &Path,
	cursor: &mut Cursor,
	pending: &PendingRecord,
) -> Result<(), Box<dyn std::error::Error>> {
	if cursor.source != Some(pending.source)
		|| cursor.offset != pending.start
		|| cursor.prefix_len != pending.prefix_len
		|| cursor.prefix_hash != pending.prefix_hash
	{
		return Err(
			"provider outbox cursor can advance only across the exact contiguous record".into()
		);
	}
	cursor.offset = pending.end;
	validate_cursor(cursor)?;
	atomic_json(path, cursor)
}

fn compact_if_drained(
	outbox: &Path,
	_lock_path: &Path,
	paths: &StatePaths,
	cursor: &mut Cursor,
	threshold: u64,
) -> Result<(), Box<dyn std::error::Error>> {
	validate_cursor(cursor)?;
	if cursor.offset < threshold {
		return Ok(());
	}
	let mut file = File::open(outbox)?;
	let metadata = file.metadata()?;
	let old_source = source_id(&metadata)?;
	if cursor.source != Some(old_source) || cursor.offset != metadata.len() {
		return Ok(());
	}
	if paths.pending.exists() {
		return Err("provider outbox cannot compact while a pending record exists".into());
	}
	validate_cursor_source(cursor, old_source, metadata.len())?;
	verify_source_prefix(&mut file, cursor.prefix_len, &cursor.prefix_hash)?;
	let marker = CompactionMarker {
		version: STATE_VERSION,
		old_source,
		consumed_len: cursor.offset,
		prefix_len: cursor.prefix_len,
		prefix_hash: cursor.prefix_hash.clone(),
	};
	validate_marker(&marker)?;
	atomic_json(&paths.compaction, &marker)?;
	install_empty_source(outbox)?;
	let new_source = source_id(&File::open(outbox)?.metadata()?)?;
	cursor.source = Some(new_source);
	cursor.offset = 0;
	cursor.prefix_len = 0;
	cursor.prefix_hash = blake3::hash(&[]).to_hex().to_string();
	atomic_json(&paths.cursor, cursor)?;
	remove_durable(&paths.compaction)
}

fn recover_compaction(
	outbox: &Path,
	_lock_path: &Path,
	paths: &StatePaths,
	cursor: &mut Cursor,
	marker: Option<&CompactionMarker>,
	pending_present: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let Some(marker) = marker else { return Ok(()) };
	validate_marker(&marker)?;
	if pending_present {
		return Err(
			"provider outbox cannot recover compaction while a pending record exists".into()
		);
	}
	validate_cursor(&cursor)?;
	let mut file = File::open(outbox)?;
	let metadata = file.metadata()?;
	let current = source_id(&metadata)?;
	if current == marker.old_source {
		if cursor.source != Some(marker.old_source)
			|| cursor.offset != marker.consumed_len
			|| cursor.prefix_len != marker.prefix_len
			|| cursor.prefix_hash != marker.prefix_hash
		{
			return Err(
				"provider outbox compaction marker does not match the durable cursor".into()
			);
		}
		validate_cursor_source(&cursor, current, metadata.len())?;
		if metadata.len() != marker.consumed_len {
			return Err("provider outbox compaction source length changed".into());
		}
		verify_source_prefix(&mut file, marker.prefix_len, &marker.prefix_hash)?;
		install_empty_source(outbox)?;
		let new_source = source_id(&File::open(outbox)?.metadata()?)?;
		cursor.source = Some(new_source);
		cursor.offset = 0;
		cursor.prefix_len = 0;
		cursor.prefix_hash = blake3::hash(&[]).to_hex().to_string();
		atomic_json(&paths.cursor, &cursor)?;
		return remove_durable(&paths.compaction);
	}

	if metadata.len() != 0 {
		return Err("provider outbox rotated compaction source is not empty".into());
	}
	let old_cursor = cursor.source == Some(marker.old_source)
		&& cursor.offset == marker.consumed_len
		&& cursor.prefix_len == marker.prefix_len
		&& cursor.prefix_hash == marker.prefix_hash;
	let reset_cursor = cursor.source == Some(current)
		&& cursor.offset == 0
		&& cursor.prefix_len == 0
		&& cursor.prefix_hash == blake3::hash(&[]).to_hex().to_string();
	if old_cursor {
		cursor.source = Some(current);
		cursor.offset = 0;
		cursor.prefix_len = 0;
		cursor.prefix_hash = blake3::hash(&[]).to_hex().to_string();
		atomic_json(&paths.cursor, &cursor)?;
	} else if !reset_cursor {
		return Err("provider outbox compaction marker does not match a recoverable cursor".into());
	}
	remove_durable(&paths.compaction)
}

fn install_empty_source(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let temp = suffixed(path, ".rotate.tmp");
	let file = OpenOptions::new().create(true).truncate(true).write(true).open(&temp)?;
	file.sync_all()?;
	fs::rename(&temp, path)?;
	sync_parent(path)
}

fn ensure_version(version: u16) -> Result<(), Box<dyn std::error::Error>> {
	if version == STATE_VERSION {
		Ok(())
	} else {
		Err("unsupported provider outbox state version".into())
	}
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, Box<dyn std::error::Error>> {
	let mut file = match File::open(path) {
		Ok(file) => file,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(error) => return Err(error.into()),
	};
	let len = file.metadata()?.len();
	if len > MAX_STATE_BYTES {
		return Err("provider outbox state exceeds its hard byte limit".into());
	}
	let mut bytes = vec![0u8; len as usize];
	file.read_exact(&mut bytes)?;
	if file.metadata()?.len() != len {
		return Err("provider outbox state changed while being read".into());
	}
	Ok(Some(serde_json::from_slice(&bytes)?))
}

fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn std::error::Error>> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent)?;
	}
	let encoded = serde_json::to_vec(value)?;
	if encoded.len() as u64 > MAX_STATE_BYTES {
		return Err("provider outbox state exceeds its hard byte limit".into());
	}
	let temp = suffixed(path, ".tmp");
	let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&temp)?;
	file.write_all(&encoded)?;
	file.sync_all()?;
	fs::rename(&temp, path)?;
	sync_parent(path)
}

fn remove_durable(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
	match fs::remove_file(path) {
		Ok(()) => sync_parent(path),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(error.into()),
	}
}

fn sync_parent(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
	let parent = path
		.parent()
		.filter(|parent| !parent.as_os_str().is_empty())
		.unwrap_or_else(|| Path::new("."));
	File::open(parent)?.sync_all()?;
	Ok(())
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
	let mut value = path.as_os_str().to_os_string();
	value.push(suffix);
	PathBuf::from(value)
}

fn native_error(error: impl std::fmt::Display) -> OriginSdkError {
	OriginSdkError::InvalidInput(error.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn provider_lock_must_preexist_and_is_never_created_by_the_consumer() {
		let temp = tempfile::tempdir().unwrap();
		let lock = temp.path().join("provider-submissions-v3.jsonl.lock");
		assert!(exclusive_existing_lock(&lock).is_err());
		assert!(!lock.exists());
		fs::write(&lock, b"").unwrap();
		let held = exclusive_existing_lock(&lock).unwrap();
		let contender = OpenOptions::new().read(true).write(true).open(&lock).unwrap();
		assert!(FileExt::try_lock_exclusive(&contender).is_err());
		drop(held);
		FileExt::try_lock_exclusive(&contender).unwrap();
		FileExt::unlock(&contender).unwrap();
	}
	use origin_orbis_provider::ManifestDeletionSubmission;

	fn write_records(path: &Path, count: usize, payload: usize) {
		let mut file = File::create(path).unwrap();
		for index in 0..count {
			let line = format!("{{\"index\":{index},\"padding\":\"{}\"}}\n", "x".repeat(payload));
			file.write_all(line.as_bytes()).unwrap();
		}
		file.sync_all().unwrap();
	}

	fn manifest_submission() -> ProviderSubmission {
		ProviderSubmission::ManifestDeletion(ManifestDeletionSubmission {
			manifest: format!("0x{}", "21".repeat(32)),
			bucket_id: format!("0x{}", "22".repeat(32)),
			provider_commitment: format!("0x{}", "23".repeat(32)),
			evidence_hash: format!("0x{}", "24".repeat(32)),
			tombstoned_at: 10,
			service_key: format!("0x{}", "25".repeat(32)),
			signature: format!("0x{}", "26".repeat(64)),
			duty_fingerprint: format!("0x{}", "27".repeat(32)),
		})
	}

	fn finalized_receipt(pending: &PendingRecord) -> FinalizedReceipt {
		FinalizedReceipt {
			key: pending.key.clone(),
			record_hash: pending.record_hash.clone(),
			source: pending.source,
			start: pending.start,
			end: pending.end,
			prefix_len: pending.prefix_len,
			prefix_hash: pending.prefix_hash.clone(),
			block_hash: format!("0x{}", "31".repeat(32)),
			extrinsic_hash: format!("0x{}", "32".repeat(32)),
		}
	}

	#[test]
	fn supported_submission_maps_only_to_runtime_valid_idempotent_manifest_deletion() {
		let (_, deletion) = command(manifest_submission()).unwrap();
		assert!(matches!(deletion, StorageProviderCommand::AcknowledgeManifestDeletion { .. }));
	}

	#[test]
	fn large_history_is_read_in_hard_bounded_batches() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		let cursor_path = temp.path().join("cursor.json");
		write_records(&outbox, 30_000, 64);
		let mut cursor = Cursor::default();
		let batch =
			read_batch(&outbox, &suffixed(&outbox, ".lock"), &cursor_path, &mut cursor).unwrap();
		assert_eq!(batch.len(), MAX_BATCH_RECORDS);
		assert!(batch.iter().map(|record| record.line.len()).sum::<usize>() <= MAX_BATCH_BYTES);
		assert!(File::open(&outbox).unwrap().metadata().unwrap().len() > MAX_BATCH_BYTES as u64);
	}

	#[test]
	fn bounded_torn_tail_repair_preserves_complete_records() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		fs::write(&outbox, b"one\ntwo\npartial").unwrap();
		repair_incomplete_tail(&outbox).unwrap();
		assert_eq!(fs::read(&outbox).unwrap(), b"one\ntwo\n");
	}

	#[test]
	fn invalid_source_bindings_preserve_an_incomplete_tail() {
		for corruption in 0..3 {
			let temp = tempfile::tempdir().unwrap();
			let outbox = temp.path().join("outbox.jsonl");
			let cursor_path = temp.path().join("cursor.json");
			let bytes = b"one\npartial";
			fs::write(&outbox, bytes).unwrap();
			let source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			let mut cursor = Cursor {
				version: STATE_VERSION,
				source: Some(if corruption == 0 {
					SourceId { device: source.device, inode: source.inode.wrapping_add(1) }
				} else {
					source
				}),
				offset: if corruption == 2 { bytes.len() as u64 } else { 0 },
				prefix_len: 4,
				prefix_hash: if corruption == 1 {
					blake3::hash(b"xxxx").to_hex().to_string()
				} else {
					blake3::hash(b"one\n").to_hex().to_string()
				},
			};

			assert!(read_batch(&outbox, &suffixed(&outbox, ".lock"), &cursor_path, &mut cursor,)
				.is_err());
			assert_eq!(fs::read(&outbox).unwrap(), bytes);
			assert!(!cursor_path.exists());
		}
	}

	#[test]
	fn cursor_fails_closed_when_bound_prefix_is_rewritten_in_place() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		let cursor_path = temp.path().join("cursor.json");
		fs::write(&outbox, b"first\nsecond\n").unwrap();
		let mut cursor = Cursor::default();
		read_batch(&outbox, &suffixed(&outbox, ".lock"), &cursor_path, &mut cursor).unwrap();
		fs::write(&outbox, b"FIRST\nsecond\n").unwrap();
		assert!(read_batch(&outbox, &suffixed(&outbox, ".lock"), &cursor_path, &mut cursor)
			.unwrap_err()
			.to_string()
			.contains("prefix hash changed"));
	}

	#[test]
	fn oversized_record_and_state_fail_closed() {
		let mut input = BufReader::new(std::io::Cursor::new(vec![b'x'; MAX_RECORD_BYTES + 2]));
		assert!(read_bounded_line(&mut input).is_err());
		let temp = tempfile::tempdir().unwrap();
		let state = temp.path().join("state");
		fs::write(&state, vec![0u8; MAX_STATE_BYTES as usize + 1]).unwrap();
		assert!(load_json::<Cursor>(&state).is_err());
	}

	#[test]
	fn pending_record_is_exact_bytes_and_cursor_advances_only_after_receipt() {
		let temp = tempfile::tempdir().unwrap();
		let paths = StatePaths::new(&temp.path().join("receipts.json"));
		let source = SourceId { device: 1, inode: 2 };
		let line = serde_json::to_string(&manifest_submission()).unwrap() + "\n";
		let pending = PendingRecord {
			version: STATE_VERSION,
			source,
			start: 0,
			end: line.len() as u64,
			record_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
			prefix_len: line.len() as u64,
			prefix_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
			key: "manifest-deletion-1".into(),
			line: line.clone(),
		};
		atomic_json(&paths.pending, &pending).unwrap();
		assert_eq!(load_json::<PendingRecord>(&paths.pending).unwrap().unwrap().line, line);
		assert!(load_json::<Cursor>(&paths.cursor).unwrap().is_none());
		let mut ledger = ReceiptLedger::default();
		record_receipt(&paths.receipts, &mut ledger, finalized_receipt(&pending)).unwrap();
		let mut cursor = Cursor {
			source: Some(source),
			prefix_len: pending.prefix_len,
			prefix_hash: pending.prefix_hash.clone(),
			..Cursor::default()
		};
		advance_cursor(&paths.cursor, &mut cursor, &pending).unwrap();
		assert_eq!(load_json::<Cursor>(&paths.cursor).unwrap().unwrap().offset, pending.end);
	}

	#[test]
	fn cursor_rejects_gap_and_overlap_without_durable_advance() {
		let temp = tempfile::tempdir().unwrap();
		let paths = StatePaths::new(&temp.path().join("receipts.json"));
		let source = SourceId { device: 1, inode: 2 };
		let prefix_hash = blake3::hash(&[]).to_hex().to_string();
		let pending = PendingRecord {
			version: STATE_VERSION,
			source,
			start: 10,
			end: 20,
			record_hash: blake3::hash(b"record\n").to_hex().to_string(),
			prefix_len: 0,
			prefix_hash: prefix_hash.clone(),
			key: "manifest-deletion-1".into(),
			line: "record\n".into(),
		};

		let mut gap_cursor = Cursor {
			version: STATE_VERSION,
			source: Some(source),
			offset: 9,
			prefix_len: 0,
			prefix_hash: prefix_hash.clone(),
		};
		assert!(advance_cursor(&paths.cursor, &mut gap_cursor, &pending).is_err());
		assert_eq!(gap_cursor.offset, 9);
		assert!(!paths.cursor.exists());

		let mut overlap_cursor = Cursor {
			version: STATE_VERSION,
			source: Some(source),
			offset: 11,
			prefix_len: 0,
			prefix_hash,
		};
		assert!(advance_cursor(&paths.cursor, &mut overlap_cursor, &pending).is_err());
		assert_eq!(overlap_cursor.offset, 11);
		assert!(!paths.cursor.exists());
	}

	#[test]
	fn restart_recovers_crashes_after_finality_receipt_and_cursor_without_resubmission() {
		let temp = tempfile::tempdir().unwrap();
		let paths = StatePaths::new(&temp.path().join("receipts.json"));
		let source = SourceId { device: 7, inode: 9 };
		let line = serde_json::to_string(&manifest_submission()).unwrap() + "\n";
		let pending = PendingRecord {
			version: STATE_VERSION,
			source,
			start: 0,
			end: line.len() as u64,
			record_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
			prefix_len: line.len() as u64,
			prefix_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
			key: "manifest-deletion-restart".into(),
			line,
		};
		atomic_json(&paths.pending, &pending).unwrap();
		let mut cursor = Cursor {
			source: Some(source),
			prefix_len: pending.prefix_len,
			prefix_hash: pending.prefix_hash.clone(),
			..Cursor::default()
		};
		let mut ledger = ReceiptLedger::default();
		assert!(!recover_local_finality(&paths, &mut cursor, &ledger, &pending).unwrap());
		assert!(paths.pending.exists());

		record_receipt(&paths.receipts, &mut ledger, finalized_receipt(&pending)).unwrap();
		assert!(recover_local_finality(&paths, &mut cursor, &ledger, &pending).unwrap());
		assert_eq!(cursor.offset, pending.end);
		assert!(!paths.pending.exists());

		atomic_json(&paths.pending, &pending).unwrap();
		assert!(recover_local_finality(&paths, &mut cursor, &ReceiptLedger::default(), &pending)
			.is_err());
		assert!(paths.pending.exists());
	}

	struct CountingTransport(std::sync::atomic::AtomicUsize);

	#[async_trait::async_trait]
	impl ProviderOutboxTransport for CountingTransport {
		async fn submit(
			&self,
			_intent: &SubmitAndFinalize<StorageProviderCommand>,
		) -> Result<NativeLifecycle, String> {
			self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
			Err("transport must not be called".into())
		}
	}

	struct FinalizingTransport(std::sync::atomic::AtomicUsize);

	#[async_trait::async_trait]
	impl ProviderOutboxTransport for FinalizingTransport {
		async fn submit(
			&self,
			_intent: &SubmitAndFinalize<StorageProviderCommand>,
		) -> Result<NativeLifecycle, String> {
			self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
			Ok(NativeLifecycle {
				version: 1,
				intent_id: "intent-0000000001".into(),
				state: oc::product_sdk::NativeLifecycleState::Finalized,
				block_hash: Some(format!("0x{}", "41".repeat(32))),
				extrinsic_hash: Some(format!("0x{}", "42".repeat(32))),
				error: None,
			})
		}
	}

	#[tokio::test]
	async fn identical_lines_finalize_as_distinct_bindings_and_restart_without_resubmission() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		let receipts = temp.path().join("receipts.json");
		let line = serde_json::to_string(&manifest_submission()).unwrap() + "\n";
		fs::write(&outbox, format!("{line}{line}")).unwrap();
		let transport = FinalizingTransport(std::sync::atomic::AtomicUsize::new(0));
		let signer = AccountId::new("provider").unwrap();

		consume(&outbox, &receipts, &signer, &transport).await.unwrap();
		assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 2);
		let ledger = load_json::<ReceiptLedger>(&receipts).unwrap().unwrap();
		assert_eq!(ledger.entries.len(), 2);
		assert_eq!(ledger.entries[0].record_hash, ledger.entries[1].record_hash);
		assert_ne!(ledger.entries[0].start, ledger.entries[1].start);
		validate_ledger(&ledger).unwrap();
		let durable_ledger = fs::read(&receipts).unwrap();

		consume(&outbox, &receipts, &signer, &transport).await.unwrap();
		assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 2);
		assert_eq!(fs::read(&receipts).unwrap(), durable_ledger);
	}

	#[tokio::test]
	async fn restart_rejects_cursor_ahead_or_overlap_before_transport_and_receipts() {
		let line = serde_json::to_string(&manifest_submission()).unwrap() + "\n";
		for corrupt_offset in [line.len() as u64 + 1, 1] {
			let temp = tempfile::tempdir().unwrap();
			let outbox = temp.path().join("outbox.jsonl");
			let receipts = temp.path().join("receipts.json");
			let paths = StatePaths::new(&receipts);
			fs::write(&outbox, &line).unwrap();
			let source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			let pending = PendingRecord {
				version: STATE_VERSION,
				source,
				start: 0,
				end: line.len() as u64,
				record_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				prefix_len: line.len() as u64,
				prefix_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				key: "manifest-deletion-corrupt-cursor".into(),
				line: line.clone(),
			};
			atomic_json(&paths.pending, &pending).unwrap();
			let cursor = Cursor {
				version: STATE_VERSION,
				source: Some(source),
				offset: corrupt_offset,
				prefix_len: pending.prefix_len,
				prefix_hash: pending.prefix_hash.clone(),
			};
			atomic_json(&paths.cursor, &cursor).unwrap();
			let transport = CountingTransport(std::sync::atomic::AtomicUsize::new(0));
			let signer = AccountId::new("provider").unwrap();

			assert!(consume(&outbox, &receipts, &signer, &transport).await.is_err());
			assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 0);
			assert!(paths.pending.exists());
			assert!(!paths.receipts.exists());
		}
	}

	#[tokio::test]
	async fn end_cursor_requires_exact_receipt_before_pending_deletion_or_transport() {
		let line = serde_json::to_string(&manifest_submission()).unwrap() + "\n";
		for wrong_receipt in [false, true] {
			let temp = tempfile::tempdir().unwrap();
			let outbox = temp.path().join("outbox.jsonl");
			let receipts = temp.path().join("receipts.json");
			let paths = StatePaths::new(&receipts);
			fs::write(&outbox, &line).unwrap();
			let source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			let pending = PendingRecord {
				version: STATE_VERSION,
				source,
				start: 0,
				end: line.len() as u64,
				record_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				prefix_len: line.len() as u64,
				prefix_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				key: "manifest-deletion-end-cursor".into(),
				line: line.clone(),
			};
			atomic_json(&paths.pending, &pending).unwrap();
			atomic_json(
				&paths.cursor,
				&Cursor {
					version: STATE_VERSION,
					source: Some(source),
					offset: pending.end,
					prefix_len: pending.prefix_len,
					prefix_hash: pending.prefix_hash.clone(),
				},
			)
			.unwrap();
			if wrong_receipt {
				let mut receipt = finalized_receipt(&pending);
				receipt.start = 1;
				atomic_json(
					&paths.receipts,
					&ReceiptLedger { version: STATE_VERSION, entries: vec![receipt] },
				)
				.unwrap();
			}
			let transport = CountingTransport(std::sync::atomic::AtomicUsize::new(0));
			let signer = AccountId::new("provider").unwrap();

			assert!(consume(&outbox, &receipts, &signer, &transport).await.is_err());
			assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 0);
			assert!(paths.pending.exists());
			assert_eq!(fs::read(&outbox).unwrap(), line.as_bytes());
		}
	}

	#[tokio::test]
	async fn invalid_ledger_prevents_valid_compaction_without_mutating_durable_state() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		let receipts = temp.path().join("receipts.json");
		let paths = StatePaths::new(&receipts);
		let bytes = b"one\n";
		fs::write(&outbox, bytes).unwrap();
		let source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
		let prefix_hash = blake3::hash(bytes).to_hex().to_string();
		let cursor = Cursor {
			version: STATE_VERSION,
			source: Some(source),
			offset: bytes.len() as u64,
			prefix_len: bytes.len() as u64,
			prefix_hash: prefix_hash.clone(),
		};
		let marker = CompactionMarker {
			version: STATE_VERSION,
			old_source: source,
			consumed_len: bytes.len() as u64,
			prefix_len: bytes.len() as u64,
			prefix_hash,
		};
		let entries = (0..=MAX_RECEIPTS)
			.map(|index| FinalizedReceipt {
				key: format!("key-{index}"),
				record_hash: blake3::hash(format!("record-{index}").as_bytes())
					.to_hex()
					.to_string(),
				source,
				start: index as u64,
				end: index as u64 + 1,
				prefix_len: bytes.len() as u64,
				prefix_hash: blake3::hash(bytes).to_hex().to_string(),
				block_hash: format!("0x{}", "51".repeat(32)),
				extrinsic_hash: format!("0x{}", "52".repeat(32)),
			})
			.collect();
		atomic_json(&paths.cursor, &cursor).unwrap();
		atomic_json(&paths.compaction, &marker).unwrap();
		atomic_json(&paths.receipts, &ReceiptLedger { version: STATE_VERSION, entries }).unwrap();
		let before_outbox = fs::read(&outbox).unwrap();
		let before_cursor = fs::read(&paths.cursor).unwrap();
		let before_marker = fs::read(&paths.compaction).unwrap();
		let transport = CountingTransport(std::sync::atomic::AtomicUsize::new(0));
		let signer = AccountId::new("provider").unwrap();

		assert!(consume(&outbox, &receipts, &signer, &transport).await.is_err());
		assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 0);
		assert_eq!(fs::read(&outbox).unwrap(), before_outbox);
		assert_eq!(fs::read(&paths.cursor).unwrap(), before_cursor);
		assert_eq!(fs::read(&paths.compaction).unwrap(), before_marker);
	}

	#[test]
	fn receipts_are_bounded_and_exact_replays_are_stable() {
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("receipts.json");
		let mut ledger = ReceiptLedger::default();
		for index in 0..MAX_RECEIPTS + 50 {
			let line = format!("record-{index}\n");
			let pending = PendingRecord {
				version: STATE_VERSION,
				source: SourceId { device: 1, inode: index as u64 + 1 },
				start: index as u64,
				end: index as u64 + line.len() as u64,
				record_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				prefix_len: line.len() as u64,
				prefix_hash: blake3::hash(line.as_bytes()).to_hex().to_string(),
				key: format!("key-{index}"),
				line,
			};
			record_receipt(&path, &mut ledger, finalized_receipt(&pending)).unwrap();
		}
		assert_eq!(ledger.entries.len(), MAX_RECEIPTS);
		assert_eq!(ledger.entries.first().unwrap().key, "key-50");
	}

	#[test]
	fn drained_rotation_rebinds_cursor_and_recovery_never_drops_late_append() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("outbox.jsonl");
		fs::write(&outbox, b"one\ntwo\n").unwrap();
		let paths = StatePaths::new(&temp.path().join("receipts.json"));
		let old = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
		let mut cursor = Cursor {
			source: Some(old),
			offset: 8,
			prefix_len: 8,
			prefix_hash: blake3::hash(b"one\ntwo\n").to_hex().to_string(),
			..Cursor::default()
		};
		atomic_json(&paths.cursor, &cursor).unwrap();
		compact_if_drained(&outbox, &suffixed(&outbox, ".lock"), &paths, &mut cursor, 0).unwrap();
		assert!(fs::read(&outbox).unwrap().is_empty());
		assert_ne!(cursor.source, Some(old));

		let new_source = cursor.source.unwrap();
		let marker = CompactionMarker {
			version: STATE_VERSION,
			old_source: new_source,
			consumed_len: 1,
			prefix_len: 1,
			prefix_hash: blake3::hash(b"x").to_hex().to_string(),
		};
		atomic_json(&paths.compaction, &marker).unwrap();
		fs::write(&outbox, b"late\n").unwrap();
		assert!(recover_compaction(
			&outbox,
			&suffixed(&outbox, ".lock"),
			&paths,
			&mut cursor,
			Some(&marker),
			false,
		)
		.is_err());
		assert_eq!(fs::read(&outbox).unwrap(), b"late\n");
		assert!(paths.compaction.exists());
	}

	#[test]
	fn compaction_recovery_accepts_only_exact_crash_phase_bindings() {
		for crash_phase in 0..3 {
			let temp = tempfile::tempdir().unwrap();
			let outbox = temp.path().join("outbox.jsonl");
			let paths = StatePaths::new(&temp.path().join("receipts.json"));
			let bytes = b"one\ntwo\n";
			fs::write(&outbox, bytes).unwrap();
			let old_source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			let marker = CompactionMarker {
				version: STATE_VERSION,
				old_source,
				consumed_len: bytes.len() as u64,
				prefix_len: bytes.len() as u64,
				prefix_hash: blake3::hash(bytes).to_hex().to_string(),
			};
			let old_cursor = Cursor {
				version: STATE_VERSION,
				source: Some(old_source),
				offset: bytes.len() as u64,
				prefix_len: bytes.len() as u64,
				prefix_hash: marker.prefix_hash.clone(),
			};
			atomic_json(&paths.cursor, &old_cursor).unwrap();
			atomic_json(&paths.compaction, &marker).unwrap();

			if crash_phase > 0 {
				install_empty_source(&outbox).unwrap();
			}
			if crash_phase > 1 {
				let new_source =
					source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
				atomic_json(
					&paths.cursor,
					&Cursor { source: Some(new_source), ..Cursor::default() },
				)
				.unwrap();
			}

			let mut recovery_cursor = load_json::<Cursor>(&paths.cursor).unwrap().unwrap();
			recover_compaction(
				&outbox,
				&suffixed(&outbox, ".lock"),
				&paths,
				&mut recovery_cursor,
				Some(&marker),
				false,
			)
			.unwrap();
			assert!(fs::read(&outbox).unwrap().is_empty());
			assert!(!paths.compaction.exists());
			let recovered = load_json::<Cursor>(&paths.cursor).unwrap().unwrap();
			let current = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			assert_eq!(recovered.source, Some(current));
			assert_eq!(recovered.offset, 0);
		}
	}

	#[tokio::test]
	async fn corrupted_loaded_state_fails_before_transport_or_source_mutation() {
		for corruption in 0..4 {
			let temp = tempfile::tempdir().unwrap();
			let outbox = temp.path().join("outbox.jsonl");
			let receipts = temp.path().join("receipts.json");
			let paths = StatePaths::new(&receipts);
			fs::write(&outbox, b"record\n").unwrap();
			let source = source_id(&File::open(&outbox).unwrap().metadata().unwrap()).unwrap();
			match corruption {
				0 => atomic_json(
					&paths.cursor,
					&Cursor {
						version: STATE_VERSION,
						source: Some(source),
						offset: 0,
						prefix_len: SOURCE_PREFIX_BYTES + 1,
						prefix_hash: blake3::hash(&[]).to_hex().to_string(),
					},
				)
				.unwrap(),
				1 => {
					let prototype = PendingRecord {
						version: STATE_VERSION,
						source,
						start: 0,
						end: 7,
						record_hash: blake3::hash(b"record\n").to_hex().to_string(),
						prefix_len: 7,
						prefix_hash: blake3::hash(b"record\n").to_hex().to_string(),
						key: "key".into(),
						line: "record\n".into(),
					};
					let entries = (0..=MAX_RECEIPTS)
						.map(|index| {
							let mut receipt = finalized_receipt(&prototype);
							receipt.key = format!("key-{index}");
							receipt.record_hash =
								blake3::hash(format!("record-{index}").as_bytes())
									.to_hex()
									.to_string();
							receipt
						})
						.collect();
					atomic_json(
						&paths.receipts,
						&ReceiptLedger { version: STATE_VERSION, entries },
					)
					.unwrap();
				},
				2 => {
					let pending = PendingRecord {
						version: STATE_VERSION + 1,
						source,
						start: 0,
						end: 7,
						record_hash: blake3::hash(b"record\n").to_hex().to_string(),
						prefix_len: 7,
						prefix_hash: blake3::hash(b"record\n").to_hex().to_string(),
						key: "key".into(),
						line: "record\n".into(),
					};
					atomic_json(&paths.pending, &pending).unwrap();
				},
				_ => {
					let marker = CompactionMarker {
						version: STATE_VERSION,
						old_source: source,
						consumed_len: 7,
						prefix_len: SOURCE_PREFIX_BYTES + 1,
						prefix_hash: blake3::hash(b"record\n").to_hex().to_string(),
					};
					atomic_json(&paths.compaction, &marker).unwrap();
				},
			}
			let transport = CountingTransport(std::sync::atomic::AtomicUsize::new(0));
			let signer = AccountId::new("provider").unwrap();

			assert!(consume(&outbox, &receipts, &signer, &transport).await.is_err());
			assert_eq!(transport.0.load(std::sync::atomic::Ordering::SeqCst), 0);
			assert_eq!(fs::read(&outbox).unwrap(), b"record\n");
			if corruption == 2 {
				assert!(paths.pending.exists());
			}
			if corruption == 3 {
				assert!(paths.compaction.exists());
			}
		}
	}

	#[test]
	fn unavailable_outbox_fails_closed() {
		let temp = tempfile::tempdir().unwrap();
		let outbox = temp.path().join("missing");
		let mut cursor = Cursor::default();
		assert!(read_batch(
			&outbox,
			&suffixed(&outbox, ".lock"),
			&temp.path().join("cursor"),
			&mut cursor,
		)
		.is_err());
	}
}
