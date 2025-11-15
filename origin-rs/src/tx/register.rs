use crate::{
	error::Result,
	types::{
		element_json_to_dynamic, identifier_value, info_element_from_value, RegistryBlueprint,
	},
};
use origin_primitives::registry::RegistryPermissions;
use subxt::dynamic::Value;

use super::Transactions;

impl<'a> Transactions<'a> {
	/// Build a `Register::create_registry` call from developer JSON.
	pub async fn register_create_registry_json(
		&self,
		spec: serde_json::Value,
	) -> Result<subxt::tx::DynamicPayload> {
		let blueprint = RegistryBlueprint::from_json(spec)?;
		let args = blueprint.to_call_args()?;
		self.build("Register", "create_registry", args).await
	}

	/// Build a `Register::update_registry_info` call from JSON input.
	pub async fn register_update_info_json(
		&self,
		registry_ss58: &str,
		info: serde_json::Value,
	) -> Result<subxt::tx::DynamicPayload> {
		let registry = identifier_value(registry_ss58)?;
		let element = info_element_from_value(info)?;
		let info_value = element_json_to_dynamic(&element)?;
		let args = Value::named_composite([("registry", registry), ("info", info_value)]);
		self.build("Register", "update_registry_info", args).await
	}

	/// Convenience helper when the caller wants to update the info blob from a raw UTF-8 string.
	pub async fn register_update_info_text(
		&self,
		registry_ss58: &str,
		text: &str,
	) -> Result<subxt::tx::DynamicPayload> {
		let json = serde_json::Value::String(text.to_string());
		self.register_update_info_json(registry_ss58, json).await
	}

	pub async fn register_set_delegate(
		&self,
		registry_ss58: &str,
		delegate_ss58: &str,
		roles: Vec<RegistryPermissions>,
	) -> Result<subxt::tx::DynamicPayload> {
		let delegate = crate::types::ss58_to_account32(delegate_ss58)?;
		let args = Value::named_composite([
			("registry", identifier_value(registry_ss58)?),
			("delegate", account_value(&delegate)),
			("roles", permissions_value(roles)),
		]);
		self.build("Register", "set_delegate_permissions", args).await
	}

	pub async fn register_remove_delegate(
		&self,
		registry_ss58: &str,
		delegate_token_ss58: &str,
	) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([
			("registry", identifier_value(registry_ss58)?),
			("delegate", identifier_value(delegate_token_ss58)?),
		]);
		self.build("Register", "remove_delegate_permissions", args).await
	}

	pub async fn register_revoke(&self, registry_ss58: &str) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([("registry", identifier_value(registry_ss58)?)]);
		self.build("Register", "revoke_registry", args).await
	}

	pub async fn register_restore(&self, registry_ss58: &str) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([("registry", identifier_value(registry_ss58)?)]);
		self.build("Register", "restore_registry", args).await
	}

	pub async fn register_delete(&self, registry_ss58: &str) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([("registry", identifier_value(registry_ss58)?)]);
		self.build("Register", "delete_registry", args).await
	}
}

fn account_value(account: &subxt::utils::AccountId32) -> Value {
	crate::types::bytes_value(account.as_ref())
}

fn permissions_value(roles: Vec<RegistryPermissions>) -> Value {
	Value::unnamed_composite(roles.into_iter().map(|role| Value::u128(role.bits() as u128)))
}
