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

//! Origin Orbis native content-provider companion process.

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use origin_orbis_provider::{
	run_workers, serve, ApiConfig, DiskStore, FinalizedRuntimeAuthority, JsonlCheckpointOutbox,
	NodeProfile, ProviderService, WorkerConfig,
};
use sp_core::{crypto::AccountId32, sr25519, Pair as _};

#[derive(Debug, Parser)]
#[command(name = "origin-orbis-provider", about = "Native Orbis content-provider service")]
struct Cli {
	/// Orbis HTTP JSON-RPC endpoint. Every commit is checked at finalized head.
	#[arg(long, default_value = "http://127.0.0.1:9933")]
	orbis_rpc: String,
	/// Provider AccountId32 as 0x-prefixed hex.
	#[arg(long)]
	provider: String,
	/// Public provider HTTP endpoint recorded on Orbis.
	#[arg(long)]
	public_endpoint: String,
	/// Filesystem root for blobs and the crash-safe index.
	#[arg(long)]
	data_path: PathBuf,
	/// Local storage capacity. Must match the operator-approved provider record.
	#[arg(long)]
	capacity_bytes: u64,
	/// HTTP listener.
	#[arg(long, default_value = "127.0.0.1:8080")]
	listen: SocketAddr,
	/// Optional provider region label.
	#[arg(long)]
	region: Option<String>,
	/// Environment variable containing the bearer token. The token is never persisted.
	#[arg(long, default_value = "ORBIS_PROVIDER_BEARER_TOKEN")]
	bearer_token_env: String,
	/// Environment variable containing the sr25519 secret URI for the registered service key.
	#[arg(long, default_value = "ORBIS_PROVIDER_SERVICE_SURI")]
	service_key_env: String,
	/// Maximum decoded content bytes per commit.
	#[arg(long, default_value_t = 16 * 1024 * 1024)]
	max_content_bytes: usize,
	/// Signed checkpoint interval in seconds.
	#[arg(long, default_value_t = 60)]
	checkpoint_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();
	let provider = decode_account(&cli.provider)?;
	let bearer = std::env::var(&cli.bearer_token_env)
		.map_err(|_| format!("{} must contain a non-empty bearer token", cli.bearer_token_env))?;
	if bearer.len() < 32 {
		return Err("provider bearer token must contain at least 32 bytes".into());
	}
	let suri = std::env::var(&cli.service_key_env)
		.map_err(|_| format!("{} must contain the service-key secret URI", cli.service_key_env))?;
	let service_key = sr25519::Pair::from_string(&suri, None)
		.map_err(|error| format!("invalid service-key secret URI: {error:?}"))?;
	let provider_bytes: &[u8] = provider.as_ref();
	let profile = NodeProfile {
		provider: format!("0x{}", hex::encode(provider_bytes)),
		endpoint: cli.public_endpoint,
		service_key: format!("0x{}", hex::encode(service_key.public().0)),
		region: cli.region,
	};
	let outbox_path = cli.data_path.join("provider-submissions-v3.jsonl");
	let store = Arc::new(DiskStore::open(&cli.data_path, profile, cli.capacity_bytes)?);
	let authority = Arc::new(FinalizedRuntimeAuthority::connect(
		&cli.orbis_rpc,
		provider,
		service_key.public().0,
	)?);
	let submitter = Arc::new(JsonlCheckpointOutbox::new(outbox_path));
	let service = Arc::new(ProviderService::new(store, authority, service_key, submitter));
	let api = ApiConfig {
		listen: cli.listen,
		bearer_token_hash: *blake3::hash(bearer.as_bytes()).as_bytes(),
		max_content_bytes: cli.max_content_bytes,
		max_json_bytes: 64 * 1024,
	};
	let workers = WorkerConfig {
		checkpoint_interval: Duration::from_secs(cli.checkpoint_seconds.max(1)),
		..Default::default()
	};
	println!("origin-orbis-provider listening on {}", api.listen);
	tokio::select! {
		result = serve(api, service.clone()) => result?,
		_ = run_workers(service, workers) => {},
		_ = tokio::signal::ctrl_c() => {},
	}
	Ok(())
}

fn decode_account(value: &str) -> Result<AccountId32, Box<dyn std::error::Error>> {
	let bytes = hex::decode(value.strip_prefix("0x").unwrap_or(value))?;
	let bytes: [u8; 32] = bytes.try_into().map_err(|_| "provider must be exactly 32 bytes")?;
	Ok(AccountId32::new(bytes))
}
