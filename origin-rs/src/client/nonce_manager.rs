use crate::{error::Error, params::config::OriginConfig};
use std::{collections::HashMap, time::Instant};
use subxt::{utils::AccountId32, OnlineClient};
use tokio::sync::Mutex;

/// Strategy for nonce handling in concurrent submit pipelines.
#[derive(Clone, Copy, Debug)]
pub enum NonceStrategy {
	RpcPerTx,
	LocalCache,
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
		client: &OnlineClient<OriginConfig>,
		account: &AccountId32,
	) -> Result<u64, Error> {
		match self.strategy {
			NonceStrategy::RpcPerTx => self.fetch(client, account).await,
			NonceStrategy::LocalCache => self.allocate_cached(client, account).await,
		}
	}

	pub async fn refresh(
		&self,
		client: &OnlineClient<OriginConfig>,
		account: &AccountId32,
	) -> Result<u64, Error> {
		let fresh = self.fetch(client, account).await?;
		let mut state = self.state.lock().await;
		state.insert(account.0, NonceState { next: fresh + 1, last_refresh: Instant::now() });
		Ok(fresh)
	}

	/// Return the set of accounts currently cached.
	pub async fn accounts(&self) -> Vec<AccountId32> {
		let guard = self.state.lock().await;
		guard.keys().map(|k| AccountId32(*k)).collect()
	}

	async fn allocate_cached(
		&self,
		client: &OnlineClient<OriginConfig>,
		account: &AccountId32,
	) -> Result<u64, Error> {
		let mut guard = self.state.lock().await;
		let entry = guard.entry(account.0).or_insert_with(|| NonceState {
			next: 0,
			last_refresh: Instant::now() - self.refresh_after,
		});

		if entry.next == 0 || entry.last_refresh.elapsed() >= self.refresh_after {
			let fresh = self.fetch(client, account).await?;
			entry.next = fresh;
			entry.last_refresh = Instant::now();
		}

		let nonce = entry.next;
		entry.next = entry.next.saturating_add(1);
		Ok(nonce)
	}

	async fn fetch(
		&self,
		client: &OnlineClient<OriginConfig>,
		account: &AccountId32,
	) -> Result<u64, Error> {
		client.tx().account_nonce(account).await.map_err(Error::from)
	}
}
