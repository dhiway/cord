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

use std::{
	net::SocketAddr,
	os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use clap::Parser;
use origin_orbis_provider::{
	run_checkpoint_live_worker, run_checkpoint_quorum_worker, run_replication_worker, run_workers,
	serve_private_host_ipc, serve_provider_ingress, ApiConfig, FinalizedRuntimeAuthority,
	NodeProfile, ProviderService, WorkerConfig,
};
use sp_core::{crypto::AccountId32, ed25519, Pair as _};

#[derive(Debug, Parser)]
#[command(name = "origin-orbis-provider", about = "Native Orbis content-provider service")]
struct Cli {
	/// Orbis HTTP JSON-RPC endpoint. Every commit is checked at finalized head.
	#[arg(long, default_value = "http://127.0.0.1:9933")]
	orbis_rpc: String,
	/// Orbis native WebSocket RPC endpoint used by the account-serialized SDK finality lane.
	#[arg(long, default_value = "ws://127.0.0.1:9944")]
	orbis_native_rpc: String,
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
	/// Dedicated service-key-authenticated provider replication listener.
	#[arg(long, default_value = "127.0.0.1:8081")]
	peer_listen: SocketAddr,
	/// Private host-v2 Unix socket. Defaults beneath the provider data root.
	#[arg(long)]
	private_host_socket: Option<PathBuf>,
	/// Kernel UID permitted to connect to the private host-v2 socket. Defaults to the provider
	/// UID.
	#[arg(long)]
	private_host_uid: Option<u32>,
	/// Optional provider region label.
	#[arg(long)]
	region: Option<String>,
	/// Environment variable containing the bearer token. The token is never persisted.
	#[arg(long, default_value = "ORBIS_PROVIDER_BEARER_TOKEN")]
	bearer_token_env: String,
	/// Environment variable containing the Ed25519 secret URI for the registered service key.
	#[arg(long, default_value = "ORBIS_PROVIDER_SERVICE_SURI")]
	service_key_env: String,
	/// Environment variable containing the provider account secret URI used for Orbis extrinsics.
	#[arg(long, default_value = "ORBIS_PROVIDER_ACCOUNT_SURI")]
	account_key_env: String,
	/// Maximum decoded content bytes per commit.
	#[arg(long, default_value_t = 16 * 1024 * 1024)]
	max_content_bytes: usize,
	/// Target replication reconciliation interval in seconds.
	#[arg(long, default_value_t = 6)]
	replication_seconds: u64,
	/// Primary checkpoint quorum coordination interval in seconds.
	#[arg(long, default_value_t = 6)]
	checkpoint_quorum_seconds: u64,
	/// Checkpoint finality and exact publication reconciliation interval in seconds.
	#[arg(long, default_value_t = 6)]
	checkpoint_live_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let cli = Cli::parse();
	if cli.peer_listen == cli.listen {
		return Err("peer listener must be distinct from the public HTTP listener".into());
	}
	let provider = decode_account(&cli.provider)?;
	let bearer = std::env::var(&cli.bearer_token_env)
		.map_err(|_| format!("{} must contain a non-empty bearer token", cli.bearer_token_env))?;
	if bearer.len() < 32 {
		return Err("provider bearer token must contain at least 32 bytes".into());
	}
	let suri = std::env::var(&cli.service_key_env)
		.map_err(|_| format!("{} must contain the service-key secret URI", cli.service_key_env))?;
	let service_key = ed25519::Pair::from_string(&suri, None)
		.map_err(|error| format!("invalid service-key secret URI: {error:?}"))?;
	let account_suri = std::env::var(&cli.account_key_env).map_err(|_| {
		format!("{} must contain the provider account secret URI", cli.account_key_env)
	})?;
	let provider_bytes: &[u8] = provider.as_ref();
	let local_provider: [u8; 32] =
		provider_bytes.try_into().map_err(|_| "provider must be exactly 32 bytes")?;
	let profile = NodeProfile {
		provider: format!("0x{}", hex::encode(provider_bytes)),
		endpoint: cli.public_endpoint,
		service_key: format!("0x{}", hex::encode(service_key.public().0)),
		region: cli.region,
	};
	let authority = Arc::new(FinalizedRuntimeAuthority::connect(
		&cli.orbis_rpc,
		provider,
		service_key.public().0,
	)?);
	let service = Arc::new(ProviderService::open(
		&cli.data_path,
		profile,
		cli.capacity_bytes,
		authority,
		service_key,
	)?);
	let private_host_socket = cli
		.private_host_socket
		.unwrap_or_else(|| cli.data_path.join("origin-host-v2.sock"));
	let (private_host_listener, private_host_socket_guard) =
		bind_private_host_socket(&private_host_socket)?;
	let private_host_uid = cli.private_host_uid.unwrap_or(private_host_socket_guard.owner);
	let _private_host_socket_guard = private_host_socket_guard;
	let api = ApiConfig {
		listen: cli.listen,
		bearer_token_hash: *blake3::hash(bearer.as_bytes()).as_bytes(),
		max_content_bytes: cli.max_content_bytes,
		max_json_bytes: 64 * 1024,
	};
	let workers = WorkerConfig::default();
	// Bind before entering either lifecycle select so an unavailable peer endpoint is fatal.
	let peer_listener = tokio::net::TcpListener::bind(cli.peer_listen).await?;
	println!("origin-orbis-provider listening on {}", api.listen);
	println!("origin-orbis-provider peer ingress listening on {}", cli.peer_listen);
	println!(
		"origin-orbis-provider private host listening on {} for uid {}",
		private_host_socket.display(),
		private_host_uid
	);
	tokio::select! {
		result = serve_provider_ingress(api, peer_listener, service.clone(), local_provider) => result?,
		result = serve_private_host_ipc(private_host_listener, private_host_uid, service.clone()) => result?,
		_ = run_workers(service.clone(), workers) => {},
		_ = run_replication_worker(service.clone(), local_provider, Duration::from_secs(cli.replication_seconds.max(1))) => {},
		result = run_checkpoint_quorum_worker(service.clone(), local_provider, Duration::from_secs(cli.checkpoint_quorum_seconds.max(1))) => result?,
		result = run_checkpoint_live_worker(service.clone(), local_provider, cli.orbis_native_rpc, account_suri, Duration::from_secs(cli.checkpoint_live_seconds.max(1))) => result?,
		_ = tokio::signal::ctrl_c() => {},
	}
	Ok(())
}

#[derive(Debug)]
struct PrivateHostSocketGuard {
	path: PathBuf,
	device: u64,
	inode: u64,
	owner: u32,
}

impl Drop for PrivateHostSocketGuard {
	fn drop(&mut self) {
		let Ok(metadata) = std::fs::symlink_metadata(&self.path) else { return };
		if metadata.file_type().is_socket() &&
			metadata.dev() == self.device &&
			metadata.ino() == self.inode &&
			metadata.uid() == self.owner
		{
			let _ = std::fs::remove_file(&self.path);
		}
	}
}

fn bind_private_host_socket(
	path: &Path,
) -> std::io::Result<(tokio::net::UnixListener, PrivateHostSocketGuard)> {
	match std::fs::symlink_metadata(path) {
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
		Err(error) => return Err(error),
		Ok(_) => {
			return Err(std::io::Error::new(
				std::io::ErrorKind::AlreadyExists,
				"private host socket path already exists",
			));
		},
	}
	let listener = tokio::net::UnixListener::bind(path)?;
	let metadata = std::fs::symlink_metadata(path)?;
	if !metadata.file_type().is_socket() {
		return Err(std::io::Error::new(
			std::io::ErrorKind::InvalidData,
			"private host bind did not create a Unix socket",
		));
	}
	let guard = PrivateHostSocketGuard {
		path: path.to_path_buf(),
		device: metadata.dev(),
		inode: metadata.ino(),
		owner: metadata.uid(),
	};
	std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
	let secured = std::fs::symlink_metadata(path)?;
	if !secured.file_type().is_socket() ||
		secured.dev() != guard.device ||
		secured.ino() != guard.inode ||
		secured.uid() != guard.owner ||
		secured.permissions().mode() & 0o777 != 0o600
	{
		return Err(std::io::Error::new(
			std::io::ErrorKind::PermissionDenied,
			"private host socket identity changed while securing it",
		));
	}
	Ok((listener, guard))
}

fn decode_account(value: &str) -> Result<AccountId32, Box<dyn std::error::Error>> {
	let bytes = hex::decode(value.strip_prefix("0x").unwrap_or(value))?;
	let bytes: [u8; 32] = bytes.try_into().map_err(|_| "provider must be exactly 32 bytes")?;
	Ok(AccountId32::new(bytes))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn private_host_bind_refuses_preexisting_paths_without_unlinking_them() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		std::fs::write(&path, b"owned by another process").unwrap();
		assert_eq!(
			bind_private_host_socket(&path).unwrap_err().kind(),
			std::io::ErrorKind::AlreadyExists
		);
		assert_eq!(std::fs::read(path).unwrap(), b"owned by another process");
	}

	#[tokio::test]
	async fn private_host_cleanup_preserves_a_replacement_path() {
		let root = tempfile::tempdir().unwrap();
		let path = root.path().join("origin-host-v2.sock");
		let (listener, guard) = bind_private_host_socket(&path).unwrap();
		assert_eq!(std::fs::symlink_metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
		std::fs::remove_file(&path).unwrap();
		std::fs::write(&path, b"replacement").unwrap();
		drop(listener);
		drop(guard);
		assert_eq!(std::fs::read(path).unwrap(), b"replacement");
	}
}
