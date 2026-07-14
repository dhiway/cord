//! Offline fault injector for Substrate's indexed-transaction RocksDB column.
//!
//! The Orbis node must be stopped: opening the primary RocksDB fails while the
//! node owns its lock.  This is a disposable-network test tool, not node or
//! runtime code.  It targets column 11, which is the pinned SDK's transaction
//! column, and keeps the original Blake2-256 key while optionally corrupting
//! the value.  The normal proof provider will then either fail to load the
//! retained body (`remove`) or build a proof that the runtime rejects against
//! its authoritative transaction root (`corrupt`).

use clap::{Parser, ValueEnum};
use kvdb::{DBTransaction, KeyValueDB};
use kvdb_rocksdb::{Database, DatabaseConfig};
use serde::Serialize;
use sp_crypto_hashing::blake2_256;
use std::{fs, path::PathBuf};

const SUBSTRATE_COLUMNS: u32 = 13;
const TRANSACTION_COLUMN: u32 = 11;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Action {
	Probe,
	Backup,
	Remove,
	Corrupt,
	Restore,
}

#[derive(Debug, Parser)]
struct Args {
	/// Path ending in `chains/<chain-id>/db/full`.
	#[clap(long)]
	database: PathBuf,
	/// Explicit acknowledgement that this is a stopped disposable node database.
	#[clap(long)]
	allow_dev_faults: bool,
	#[clap(long, value_enum)]
	action: Action,
	/// Indexed payload length used by `orbis_storage_proof_fault store`.
	#[clap(long, default_value_t = 2_048)]
	bytes: usize,
	/// Indexed payload fill byte used by `orbis_storage_proof_fault store`.
	#[clap(long, default_value_t = 0x5a)]
	fill: u8,
	/// Backup file written before remove/corrupt and read by restore.
	#[clap(long)]
	backup: PathBuf,
	/// Permit replacing an existing backup after its contents are verified.
	#[clap(long)]
	overwrite_backup: bool,
}

#[derive(Debug, Serialize)]
struct Output {
	status: &'static str,
	action: String,
	database: String,
	column: u32,
	key: String,
	original_bytes: usize,
	result_bytes: usize,
	present: bool,
	backup: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	if !args.allow_dev_faults {
		return Err("refusing database mutation without --allow-dev-faults".into());
	}
	if args.bytes == 0 || args.bytes > 65_536 {
		return Err("bytes must be 1..=65536".into());
	}
	if !args.database.ends_with("db/full") {
		return Err("database path must end in db/full".into());
	}

	let key = blake2_256(&vec![args.fill; args.bytes]);
	let config = DatabaseConfig::with_columns(SUBSTRATE_COLUMNS);
	let database = Database::open(&config, &args.database)
		.map_err(|error| format!("failed to open RocksDB (is the Orbis node stopped?): {error}"))?;
	let original = database.get(TRANSACTION_COLUMN, &key)?;
	if original.is_none() && !matches!(args.action, Action::Probe | Action::Restore) {
		return Err(format!("indexed payload {} not found", hex::encode(key)).into());
	}
	if let Some(value) = original.as_ref().filter(|_| !matches!(args.action, Action::Restore)) {
		if value.len() != args.bytes || blake2_256(value) != key {
			return Err(
				"indexed payload is already corrupt or does not match --bytes/--fill".into()
			);
		}
	}
	let original_bytes = original.as_ref().map_or(0, Vec::len);

	let result_bytes = match args.action {
		Action::Probe => original_bytes,
		Action::Backup => {
			let original = original.as_ref().expect("checked above");
			if args.backup.exists() && !args.overwrite_backup {
				return Err(
					"backup already exists; use --overwrite-backup after verifying it".into()
				);
			}
			fs::write(&args.backup, original)?;
			original.len()
		},
		Action::Remove => {
			let original = original.as_ref().expect("checked above");
			if args.backup.exists() && !args.overwrite_backup {
				return Err(
					"backup already exists; use --overwrite-backup after verifying it".into()
				);
			}
			fs::write(&args.backup, original)?;
			let mut transaction = DBTransaction::new();
			transaction.delete(TRANSACTION_COLUMN, &key);
			database.write(transaction)?;
			0
		},
		Action::Corrupt => {
			let original = original.as_ref().expect("checked above");
			if args.backup.exists() && !args.overwrite_backup {
				return Err(
					"backup already exists; use --overwrite-backup after verifying it".into()
				);
			}
			fs::write(&args.backup, original)?;
			let mut corrupted = original.to_vec();
			corrupted[0] ^= 0xff;
			let mut transaction = DBTransaction::new();
			transaction.put_vec(TRANSACTION_COLUMN, &key, corrupted.clone());
			database.write(transaction)?;
			corrupted.len()
		},
		Action::Restore => {
			let restored = fs::read(&args.backup)?;
			if restored.len() != args.bytes || blake2_256(&restored) != key {
				return Err("backup does not match the deterministic --bytes/--fill payload".into());
			}
			let mut transaction = DBTransaction::new();
			transaction.put_vec(TRANSACTION_COLUMN, &key, restored.clone());
			database.write(transaction)?;
			restored.len()
		},
	};
	let present = database.get(TRANSACTION_COLUMN, &key)?.is_some();

	let output = Output {
		status: "ok",
		action: format!("{:?}", args.action).to_lowercase(),
		database: args.database.display().to_string(),
		column: TRANSACTION_COLUMN,
		key: format!("0x{}", hex::encode(key)),
		original_bytes,
		result_bytes,
		present,
		backup: args.backup.display().to_string(),
	};
	println!("{}", serde_json::to_string_pretty(&output)?);
	Ok(())
}
