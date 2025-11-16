use crate::{error::Result, flavors::ChainFlavor};
use codec::Decode;
use serde::{Deserialize, Serialize};
use sp_core::hashing::blake2_256;
use std::{fs, path::PathBuf, time::SystemTime};
use subxt::{
	backend::rpc::RpcClient,
	config::PolkadotConfig,
	ext::{
		subxt_core::client::RuntimeVersion as CoreRuntimeVersion,
		subxt_rpcs::methods::legacy::{LegacyRpcMethods, RuntimeVersion as LegacyRuntimeVersion},
	},
	Metadata,
};

const DEFAULT_DIR: &str = "metadata";
const INDEX_FILE: &str = "index.json";

#[derive(Debug, Serialize, Deserialize)]
struct Index {
	entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
	flavor: String,
	spec_version: u32,
	metadata_hash: String,
	path: String,
	fetched_at_unix: u64,
}

impl Index {
	fn load(path: &PathBuf) -> Result<Self> {
		if path.exists() {
			let data = fs::read(path).map_err(|e| crate::error::Error::Params(e.to_string()))?;
			Ok(serde_json::from_slice(&data)
				.map_err(|e| crate::error::Error::Params(e.to_string()))?)
		} else {
			Ok(Self { entries: Vec::new() })
		}
	}

	fn save(&self, path: &PathBuf) -> Result<()> {
		let data = serde_json::to_vec_pretty(self)
			.map_err(|e| crate::error::Error::Params(e.to_string()))?;
		fs::write(path, data).map_err(|e| crate::error::Error::Params(e.to_string()))?;
		Ok(())
	}

	fn upsert(&mut self, entry: Entry) {
		self.entries.retain(|e| !(e.flavor == entry.flavor && e.metadata_hash == entry.metadata_hash));
		self.entries.push(entry);
	}
}

fn metadata_dir_from_env() -> PathBuf {
	std::env::var("ORIGIN_RS_METADATA_DIR")
		.map(PathBuf::from)
		.unwrap_or_else(|_| PathBuf::from(DEFAULT_DIR))
}

/// Ensure the current chain's metadata is cached on disk and indexed.
pub async fn cache_metadata(
	flavor: ChainFlavor,
	runtime_version: &CoreRuntimeVersion,
	metadata_bytes: &[u8],
) -> Result<()> {
	let dir = metadata_dir_from_env();
	let (flavor_dir, latest_file) = match flavor {
		ChainFlavor::Origin => ("origin", "origin.scale"),
		_ => ("origin-hub", "origin-hub.scale"),
	};

	fs::create_dir_all(dir.join(flavor_dir))
		.map_err(|e| crate::error::Error::Params(e.to_string()))?;

	let hash_hex = format!("0x{}", hex::encode(blake2_256(metadata_bytes)));

	// fetch blob
	let blob = metadata_bytes;
	let hashed_path = dir.join(flavor_dir).join(format!("{}.scale", &hash_hex[2..]));
	fs::write(&hashed_path, blob).map_err(|e| crate::error::Error::Params(e.to_string()))?;
	fs::write(dir.join(latest_file), blob)
		.map_err(|e| crate::error::Error::Params(e.to_string()))?;

	// update index
	let idx_path = dir.join(INDEX_FILE);
	let mut index = Index::load(&idx_path)?;
	index.upsert(Entry {
		flavor: flavor_dir.to_string(),
		spec_version: runtime_version.spec_version,
		metadata_hash: hash_hex,
		path: hashed_path
			.strip_prefix(&dir)
			.unwrap_or(&hashed_path)
			.to_string_lossy()
			.into_owned(),
		fetched_at_unix: SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.map_err(|e| crate::error::Error::Params(e.to_string()))?
			.as_secs(),
	});
	index.save(&idx_path)?;

	Ok(())
}

/// Try to load metadata from cache by hash; fall back to live fetch.
pub async fn load_or_fetch(
	rpc: &RpcClient,
) -> Result<(subxt::utils::H256, CoreRuntimeVersion, Metadata, Vec<u8>)> {
	let dir = metadata_dir_from_env();
	let legacy = LegacyRpcMethods::<PolkadotConfig>::new(rpc.clone());

	let genesis_hash = legacy
		.chain_get_block_hash(Some(0u32.into()))
		.await
		.map_err(|e| crate::error::Error::Transport(e.to_string()))?
		.ok_or_else(|| crate::error::Error::NotFound("genesis hash".into()))?;

	let runtime_version_legacy = legacy
		.state_get_runtime_version(None)
		.await
		.map_err(|e| crate::error::Error::Transport(e.to_string()))?;
	let runtime_version = convert_runtime_version(&runtime_version_legacy);

	let raw_meta = legacy
		.state_get_metadata(None)
		.await
		.map_err(|e| crate::error::Error::Transport(e.to_string()))?
		.into_raw();

let hash_hex = format!("0x{}", hex::encode(blake2_256(&raw_meta)));

	for flavor_dir in ["origin", "origin-hub"] {
		let candidate = dir.join(flavor_dir).join(format!("{}.scale", &hash_hex[2..]));
		if let Ok(bytes) = fs::read(&candidate) {
			if let Ok(meta) = Metadata::decode(&mut &bytes[..]) {
				return Ok((genesis_hash, runtime_version.clone(), meta, bytes));
			}
		}
	}

	let meta = Metadata::decode(&mut &raw_meta[..])
		.map_err(|e| crate::error::Error::Codec(e.to_string()))?;

	Ok((genesis_hash, runtime_version, meta, raw_meta))
}
fn convert_runtime_version(
	rv: &LegacyRuntimeVersion,
) -> CoreRuntimeVersion {
	CoreRuntimeVersion {
		spec_version: rv.spec_version,
		transaction_version: rv.transaction_version,
	}
}
