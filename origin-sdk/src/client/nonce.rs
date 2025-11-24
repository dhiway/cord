use std::{collections::HashMap, time::Instant};

use crate::types::error::OriginSdkError;
use tokio::sync::Mutex;

/// Strategy for nonce handling in concurrent submit pipelines.
#[derive(Clone, Copy, Debug)]
pub enum NonceStrategy {
	RpcPerTx,
	LocalCache,
}

impl Default for NonceStrategy {
	fn default() -> Self {
		NonceStrategy::RpcPerTx
	}
}

/// In-memory nonce state for a single account.
#[derive(Clone, Debug)]
pub struct NonceState {
	pub next: u64,
	pub last_refresh: Instant,
}

/// Thread-safe nonce tracker that can serve many concurrent submitters.
#[derive(Debug)]
pub struct NonceManager {
	strategy: NonceStrategy,
	refresh_after: std::time::Duration,
	state: Mutex<HashMap<[u8; 32], NonceState>>,
}

impl NonceManager {
	pub fn new(strategy: NonceStrategy, refresh_after: std::time::Duration) -> Self {
		Self { strategy, refresh_after, state: Mutex::new(HashMap::new()) }
	}

	/// Allocate the next nonce for an account, refreshing if the cache is stale.
	pub async fn allocate(
		&self,
		api: &subxt::OnlineClient<crate::client::OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		match self.strategy {
			NonceStrategy::RpcPerTx => self.fetch(api, account).await,
			NonceStrategy::LocalCache => self.allocate_cached(api, account).await,
		}
	}

	async fn allocate_cached(
		&self,
		api: &subxt::OnlineClient<crate::client::OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		let mut guard = self.state.lock().await;
		let entry = guard.entry(account.clone().into()).or_insert_with(|| NonceState {
			next: 0,
			last_refresh: Instant::now() - self.refresh_after,
		});

		if entry.next == 0 || entry.last_refresh.elapsed() >= self.refresh_after {
			let fresh = self.fetch(api, account).await?;
			entry.next = fresh;
			entry.last_refresh = Instant::now();
		}

		let nonce = entry.next;
		entry.next = entry.next.saturating_add(1);
		Ok(nonce)
	}

	async fn fetch(
		&self,
		api: &subxt::OnlineClient<crate::client::OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		api.tx()
			.account_nonce(account)
			.await
			.map_err(|e| OriginSdkError::Nonce(e.to_string()))
	}
}
