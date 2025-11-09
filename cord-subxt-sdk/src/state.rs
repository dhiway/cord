use crate::{
	error::{Error, Result},
	params::config::CordConfig,
	types,
};
use codec::Decode;
use subxt::dynamic;

/// Convenience helpers for state queries that the SDK exposes frequently.
pub struct State<'a> {
	pub(crate) api: &'a subxt::OnlineClient<CordConfig>,
}

/// Simplified account info type matching `frame_system::AccountInfo`.
pub type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;

impl<'a> State<'a> {
	/// Fetch `frame_system::AccountInfo` for the supplied SS58 address.
	pub async fn account_info(&self, account_ss58: &str) -> Result<AccountInfo> {
		let account = types::ss58_to_account32(account_ss58)?;
		let addr =
			dynamic::storage("System", "Account", vec![subxt::dynamic::Value::from_bytes(account)]);
		let snapshot = self.api.storage().at_latest().await.map_err(Error::from)?;
		let Some(raw) = snapshot.fetch(&addr).await.map_err(Error::from)? else {
			return Err(Error::NotFound(format!("account {account_ss58} not found")));
		};
		let mut cursor = raw.encoded();
		AccountInfo::decode(&mut &cursor[..]).map_err(|e| Error::Codec(e.to_string()))
	}

	/// Return the block hash for the provided block number (if available).
	pub async fn block_hash(&self, number: u32) -> Result<Option<subxt::utils::H256>> {
		self.api.rpc().block_hash(Some(number.into())).await.map_err(Error::from)
	}
}
