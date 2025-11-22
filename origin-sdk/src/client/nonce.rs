use std::collections::HashMap;

use tokio::sync::Mutex;

/// Lightweight per-account nonce tracker (local only for now).
#[derive(Default)]
pub struct NonceManager {
	inner: Mutex<HashMap<[u8; 32], u64>>,
}

impl NonceManager {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn next(&self, account: &[u8; 32]) -> u64 {
		let mut guard = self.inner.lock().await;
		let entry = guard.entry(*account).or_insert(0);
		let current = *entry;
		*entry = entry.saturating_add(1);
		current
	}

	pub async fn set(&self, account: &[u8; 32], nonce: u64) {
		let mut guard = self.inner.lock().await;
		guard.insert(*account, nonce);
	}
}
