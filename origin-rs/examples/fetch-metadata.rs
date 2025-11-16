use anyhow::{anyhow, Result};
use hex::ToHex;
use oc::{ChainFlavor, Client};
use serde::{Deserialize, Serialize};
use std::{
	fs,
	path::{Path, PathBuf},
	time::SystemTime,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Flavor {
	Origin,
	OriginHub,
}

#[derive(Debug, Serialize, Deserialize)]
struct Index {
	entries: Vec<IndexEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexEntry {
	flavor: Flavor,
	spec_version: u32,
	metadata_hash: String,
	path: String,
	fetched_at_unix: u64,
	node: String,
}

impl Index {
	fn load(path: &Path) -> Result<Self> {
		if path.exists() {
			let data = fs::read(path)?;
			Ok(serde_json::from_slice(&data)?)
		} else {
			Ok(Self { entries: Vec::new() })
		}
	}

	fn add_entry(&mut self, entry: IndexEntry) {
		self.entries.retain(|e| !(e.flavor == entry.flavor && e.metadata_hash == entry.metadata_hash));
		self.entries.push(entry);
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	env_logger::init();
	let args: Vec<String> = std::env::args().collect();
	let opts = Options::parse(&args)?;

	let flavor = match opts.flavor.as_str() {
		"origin" => ChainFlavor::Origin,
		"origin-hub" | "hub" => ChainFlavor::OriginHub,
		other => return Err(anyhow!("invalid --flavor {other} (use origin|origin-hub)")),
	};

	let client = Client::connect(&opts.node, flavor).await?;
	let version = client.runtime_version().await?;
	let hash = client.metadata_hash().await?;
	let hash_hex = hash.encode_hex::<String>();

	let base_dir = PathBuf::from(opts.out_dir);
	let subdir = base_dir.join(match flavor {
		ChainFlavor::Origin => "origin",
		_ => "origin-hub",
	});
	fs::create_dir_all(&subdir)?;

	// Write hashed snapshot
	let blob = client.fetch_metadata_blob().await?;
	let hashed_path = subdir.join(format!("{hash_hex}.scale"));
	fs::write(&hashed_path, &blob)?;

	// Update flat latest pointers for build.rs compatibility
	let latest_name = match flavor {
		ChainFlavor::Origin => "origin.scale",
		_ => "origin-hub.scale",
	};
	let latest_path = base_dir.join(latest_name);
	fs::write(&latest_path, &blob)?;

	// Update index
	let index_path = base_dir.join("index.json");
	let mut index = Index::load(&index_path)?;
	index.add_entry(IndexEntry {
		flavor: match flavor {
			ChainFlavor::Origin => Flavor::Origin,
			_ => Flavor::OriginHub,
		},
		spec_version: version.spec_version,
		metadata_hash: format!("0x{hash_hex}"),
		path: hashed_path
			.strip_prefix(&base_dir)
			.unwrap_or(&hashed_path)
			.to_string_lossy()
			.into_owned(),
		fetched_at_unix: now_unix()?,
		node: opts.node.clone(),
	});
	let pretty = serde_json::to_string_pretty(&index)?;
	fs::write(&index_path, pretty)?;

	println!(
		"✅ saved metadata for {:#?} → {} (spec={})",
		flavor,
		hashed_path.display(),
		version.spec_version,
	);
	Ok(())
}

fn now_unix() -> Result<u64> {
	Ok(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs())
}

#[derive(Debug)]
struct Options {
	node: String,
	flavor: String,
	out_dir: String,
}

impl Options {
	fn parse(args: &[String]) -> Result<Self> {
		let mut node = "ws://127.0.0.1:9944".to_string();
		let mut flavor = "origin-hub".to_string();
		let mut out_dir = "metadata".to_string();
		let mut iter = args.iter().skip(1);
		while let Some(arg) = iter.next() {
			match arg.as_str() {
				"--node" | "-n" => node = take(&mut iter, arg)?,
				"--flavor" | "-f" => flavor = take(&mut iter, arg)?,
				"--out" | "-o" => out_dir = take(&mut iter, arg)?,
				_ => {},
			}
		}
		Ok(Self { node, flavor, out_dir })
	}
}

fn take<'a>(iter: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<String> {
	iter.next()
		.cloned()
		.ok_or_else(|| anyhow!("{flag} expects a value"))
}
