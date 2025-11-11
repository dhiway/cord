use crate::{
	client::Client,
	error::{Error, Result},
};
use subxt::utils::AccountId32;

/// Controls how the SDK derives a nonce for outbound transactions.
#[derive(Clone, Copy)]
pub enum NonceMode {
	/// Query the node for the next nonce (default).
	Auto,
	/// Use a caller-specified nonce.
	Manual(u64),
}

impl Default for NonceMode {
	fn default() -> Self {
		NonceMode::Auto
	}
}

/// Resolve a nonce according to the requested strategy.
pub async fn resolve_nonce(client: &Client, who: &AccountId32, mode: NonceMode) -> Result<u64> {
	match mode {
		NonceMode::Manual(nonce) => Ok(nonce),
		NonceMode::Auto => client
			.legacy_methods()
			.system_account_next_index(who)
			.await
			.map_err(|e| Error::Transport(e.to_string()))
			.map(|idx| idx as u64),
	}
}

/// Tracks nonces for sequential extrinsic submissions without re-querying the node.
pub struct NonceTracker {
	account: AccountId32,
	next: Option<u64>,
	reserved: Option<u64>,
}

impl NonceTracker {
	pub fn new(account: AccountId32) -> Self {
		Self { account, next: None, reserved: None }
	}

	/// Reserve the next nonce for the tracked account, fetching from the node on first use.
	pub async fn reserve(&mut self, client: &Client) -> Result<u64> {
		if self.next.is_none() {
			let current = client
				.legacy_methods()
				.system_account_next_index(&self.account)
				.await
				.map_err(|e| Error::Transport(e.to_string()))? as u64;
			self.next = Some(current);
		}
		let value = self.next.expect("nonce initialized");
		self.reserved = Some(value);
		self.next = Some(value + 1);
		Ok(value)
	}

	/// Confirm the last reserved nonce was accepted.
	pub fn confirm(&mut self) {
		self.reserved = None;
	}

	/// Roll back the last reservation so the nonce can be retried.
	pub fn rollback(&mut self) {
		if let Some(value) = self.reserved.take() {
			self.next = Some(value);
		}
	}

	/// Expose the tracked account id.
	pub fn account(&self) -> &AccountId32 {
		&self.account
	}
}
