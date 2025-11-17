//! High-level dynamic facades built on [`OriginClient`].
//!
//! These are thin helpers that compose dynamic calls/storage using human-readable
//! pallet/call names and `scale_value::value!` for arguments. They remain
//! runtime-upgrade-safe because they rely on runtime metadata fetched at runtime.

use crate::{origin_client::OriginClient, Error};
use scale_value::{value, Value};
use subxt::tx::TxProgress;

/// Dynamic Entity API (writes via dynamic calls).
pub struct DynamicEntityApi {
	client: OriginClient,
}

impl DynamicEntityApi {
	pub fn new(client: OriginClient) -> Self {
		Self { client }
	}

	/// Set an attribute on an entity.
	///
	/// `entity` and `key` are provided as strings; `val` is a raw string value.
	/// For richer payloads, build your own `Value` vector and call `submit_dynamic_call`.
	pub async fn set_attribute<S: subxt::tx::Signer<crate::params::config::OriginConfig> + Clone + Send + Sync>(
		&self,
		entity: &str,
		key: &str,
		val: &str,
		signer: &S,
	) -> Result<TxProgress<crate::params::config::OriginConfig, subxt::OnlineClient<crate::params::config::OriginConfig>>, Error> {
		let args = vec![
			value!(entity.to_string()),
			value!(key.to_string()),
			value!(val.to_string()),
		];
		self.client.submit_dynamic_call("Entity", "set_attribute", args, signer).await
	}

	/// Call Entity.details view (pass composite args e.g., value!({ "auth": ..., "token": ... })).
	pub async fn details(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Entity", "details", args).await
	}

	pub async fn attribute_history(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Entity", "attribute_history", args).await
	}

	pub async fn linked_accounts(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Entity", "linked_accounts", args).await
	}
}

/// Dynamic Register API.
pub struct DynamicRegisterApi {
	client: OriginClient,
}

impl DynamicRegisterApi {
	pub fn new(client: OriginClient) -> Self {
		Self { client }
	}

	pub async fn add_attribute<S: subxt::tx::Signer<crate::params::config::OriginConfig> + Clone + Send + Sync>(
		&self,
		register: &str,
		key: &str,
		val: &str,
		signer: &S,
	) -> Result<TxProgress<crate::params::config::OriginConfig, subxt::OnlineClient<crate::params::config::OriginConfig>>, Error> {
		let args = vec![
			value!(register.to_string()),
			value!(key.to_string()),
			value!(val.to_string()),
		];
		self.client.submit_dynamic_call("Register", "add_attribute", args, signer).await
	}

	pub async fn details(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Register", "details", args).await
	}

	pub async fn packet_snapshot(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Register", "packet_snapshot", args).await
	}
}

/// Dynamic Token API.
pub struct DynamicTokenApi {
	client: OriginClient,
}

impl DynamicTokenApi {
	pub fn new(client: OriginClient) -> Self {
		Self { client }
	}

	pub async fn mint<S: subxt::tx::Signer<crate::params::config::OriginConfig> + Clone + Send + Sync>(
		&self,
		token_id: &str,
		amount: u128,
		signer: &S,
	) -> Result<TxProgress<crate::params::config::OriginConfig, subxt::OnlineClient<crate::params::config::OriginConfig>>, Error> {
		let args = vec![value!(token_id.to_string()), value!(amount)];
		self.client.submit_dynamic_call("Token", "mint", args, signer).await
	}

	pub async fn timeline(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Token", "timeline", args).await
	}

	pub async fn resolve_identifier(&self, args: Value) -> Result<Value<u32>, Error> {
		self.client.call_view("Token", "resolve_identifier", args).await
	}
}

/// Bundle of dynamic APIs.
pub struct DynamicApis {
	client: OriginClient,
}

impl DynamicApis {
	pub fn new(client: OriginClient) -> Self {
		Self { client }
	}

	pub fn entity(&self) -> DynamicEntityApi {
		DynamicEntityApi::new(self.client.clone())
	}

	pub fn register(&self) -> DynamicRegisterApi {
		DynamicRegisterApi::new(self.client.clone())
	}

	pub fn token(&self) -> DynamicTokenApi {
		DynamicTokenApi::new(self.client.clone())
	}
}
