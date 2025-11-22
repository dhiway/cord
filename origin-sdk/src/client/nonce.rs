use std::collections::HashMap;

use tokio::sync::Mutex;

/// Lightweight per-account nonce tracker (local only for now).
#[derive(Default)]
pub struct NonceManager {
	inner: Mutex<HashMap<[u8; 32], u64>>,
}

impl NonceManager {
	#[allow(dead_code)]
	pub fn new() -> Self {
		Self::default()
	}

	/// Allocate next nonce for account, fetching from chain if not cached.
	pub async fn allocate(
		&self,
		api: &subxt::OnlineClient<crate::client::OriginConfig>,
		account: &[u8; 32],
	) -> Result<u64, subxt::Error> {
		let mut guard = self.inner.lock().await;
		let entry = guard.entry(*account).or_insert_with(|| 0);
		if *entry == 0 {
			let remote = api.tx().account_nonce(&subxt::utils::AccountId32::from(*account)).await?;
			*entry = remote;
		}
		let current = *entry;
		*entry = entry.saturating_add(1);
		Ok(current)
	}

	#[allow(dead_code)]
	pub async fn next(&self, account: &[u8; 32]) -> u64 {
		let mut guard = self.inner.lock().await;
		let entry = guard.entry(*account).or_insert(0);
		let current = *entry;
		*entry = entry.saturating_add(1);
		current
	}

	#[allow(dead_code)]
	pub async fn set(&self, account: &[u8; 32], nonce: u64) {
		let mut guard = self.inner.lock().await;
		guard.insert(*account, nonce);
	}
}
