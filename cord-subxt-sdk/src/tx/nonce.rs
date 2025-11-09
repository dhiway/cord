use crate::{
	error::{Error, Result},
	params::config::CordConfig,
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
pub async fn resolve_nonce(
	api: &subxt::OnlineClient<CordConfig>,
	who: &AccountId32,
	mode: NonceMode,
) -> Result<u64> {
	match mode {
		NonceMode::Manual(nonce) => Ok(nonce),
		NonceMode::Auto => api
			.rpc()
			.system_account_next_index(who)
			.await
			.map_err(Error::from)
			.map(|idx| idx as u64),
	}
}
