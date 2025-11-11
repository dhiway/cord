use crate::{
	error::Result,
	types::{identifier_value, PayloadMode, RegistrySchema},
};
use cord_primitives::registry::RegistryInfoView;
use subxt::dynamic::Value;

use super::Transactions;

impl<'a> Transactions<'a> {
	/// Build a `Register::create_packet` call from JSON attributes validated against the schema
	/// view.
	pub async fn packet_create_json(
		&self,
		registry_ss58: &str,
		attributes: serde_json::Value,
		schema_view: &RegistryInfoView,
	) -> Result<subxt::tx::DynamicPayload> {
		let schema = RegistrySchema::from_view(schema_view);
		let payload = schema.build_payload(&attributes, PayloadMode::Full)?;
		let args = Value::named_composite([
			("rtoken", identifier_value(registry_ss58)?),
			("attributes", payload.as_value()?),
		]);
		self.build("Register", "create_packet", args).await
	}

	/// Build a `Register::update_packet` call from JSON attribute patches.
	pub async fn packet_update_json(
		&self,
		registry_ss58: &str,
		packet_ss58: &str,
		attributes: serde_json::Value,
		schema_view: &RegistryInfoView,
	) -> Result<subxt::tx::DynamicPayload> {
		let schema = RegistrySchema::from_view(schema_view);
		let payload = schema.build_payload(&attributes, PayloadMode::Partial)?;
		let args = Value::named_composite([
			("rtoken", identifier_value(registry_ss58)?),
			("ptoken", identifier_value(packet_ss58)?),
			("attributes", payload.as_value()?),
		]);
		self.build("Register", "update_packet", args).await
	}

	pub async fn packet_revoke(
		&self,
		registry_ss58: &str,
		packet_ss58: &str,
	) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([
			("rtoken", identifier_value(registry_ss58)?),
			("ptoken", identifier_value(packet_ss58)?),
		]);
		self.build("Register", "revoke_packet", args).await
	}

	pub async fn packet_restore(
		&self,
		registry_ss58: &str,
		packet_ss58: &str,
	) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([
			("rtoken", identifier_value(registry_ss58)?),
			("ptoken", identifier_value(packet_ss58)?),
		]);
		self.build("Register", "restore_packet", args).await
	}

	pub async fn packet_remove(
		&self,
		registry_ss58: &str,
		packet_ss58: &str,
	) -> Result<subxt::tx::DynamicPayload> {
		let args = Value::named_composite([
			("rtoken", identifier_value(registry_ss58)?),
			("ptoken", identifier_value(packet_ss58)?),
		]);
		self.build("Register", "remove_packet", args).await
	}
}
