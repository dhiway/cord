use crate::{
	client::Client,
	error::{Error, Result},
	types,
};
use codec::Decode;
use cord_primitives::identifier::Ss58Identifier;
use subxt::{dynamic, utils::AccountId32};

/// Convenience helpers for state queries that the SDK exposes frequently.
pub struct State<'a> {
	pub(crate) client: &'a Client,
}

/// Simplified account info type matching `frame_system::AccountInfo`.
#[derive(Clone, Debug, Decode)]
pub struct AccountInfo {
	pub nonce: u32,
	pub consumers: u32,
	pub providers: u32,
	pub sufficients: u32,
	pub data: AccountData,
}

#[derive(Clone, Debug, Decode)]
pub struct AccountData {
	pub free: u128,
	pub reserved: u128,
	pub frozen: u128,
	pub flags: u128,
}

impl<'a> State<'a> {
	/// Fetch `frame_system::AccountInfo` for the supplied SS58 address.
	pub async fn account_info(&self, account_ss58: &str) -> Result<AccountInfo> {
		let account = types::ss58_to_account32(account_ss58)?;
		let addr =
			dynamic::storage("System", "Account", vec![subxt::dynamic::Value::from_bytes(account)]);
		let snapshot = self.client.api.storage().at_latest().await.map_err(Error::from)?;
		let Some(raw) = snapshot.fetch(&addr).await.map_err(Error::from)? else {
			return Err(Error::NotFound(format!("account {account_ss58} not found")));
		};
		let cursor = raw.encoded();
		AccountInfo::decode(&mut &cursor[..]).map_err(|e| Error::Codec(e.to_string()))
	}

	/// Return the block hash for the provided block number (if available).
	pub async fn block_hash(&self, number: u32) -> Result<Option<subxt::utils::H256>> {
		let hash = self
			.client
			.legacy_methods()
			.chain_get_block_hash(Some(number.into()))
			.await
			.map_err(|e| Error::Transport(e.to_string()))?;
		Ok(hash.map(Into::into))
	}

	/// Lookup the entity token bound to an account directly from storage.
	pub async fn entity_token_of_account(
		&self,
		account: &AccountId32,
	) -> Result<Option<Ss58Identifier>> {
		let key = dynamic::Value::from_bytes(account);
		let addr = dynamic::storage("Entity", "Ss58OfActiveAccounts", vec![key]);
		let snapshot = self.client.api.storage().at_latest().await.map_err(Error::from)?;
		let Some(raw) = snapshot.fetch(&addr).await.map_err(Error::from)? else {
			return Ok(None);
		};
		let mut cursor = raw.encoded();
		let token = Ss58Identifier::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
		Ok(Some(token))
	}
}
