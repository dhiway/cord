use super::{auth, Query};
use crate::{
	error::{Error, Result},
	types::{identifier_value, RegistrySchema},
};
use codec::Decode;
use pallet_register::register::{LookupSpecView, RegistryInfoView};
use serde_json::Value as JsonValue;

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> RegisterQuery<'a> {
	fn client(&self) -> &'a Query<'a> {
		self.query
	}

	pub async fn registry_info(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<RegistryInfoView> {
		let value: Option<RegistryInfoView> =
			self.call_registry("registry_info", auth, registry_ss58).await?;
		value.ok_or_else(|| Error::NotFound("registry not found".into()))
	}

	pub async fn schema(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<RegistrySchema> {
		let view = self.registry_info(auth, registry_ss58).await?;
		Ok(RegistrySchema::from_view(&view))
	}

	pub async fn lookup_specs(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<Vec<LookupSpecView>> {
		let specs: Option<Vec<LookupSpecView>> =
			self.call_registry("lookup_specs_view", auth, registry_ss58).await?;
		specs.ok_or_else(|| Error::NotFound("lookup specs not found".into()))
	}

	pub async fn info_json(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<JsonValue> {
		self.call_json("info", auth, registry_ss58).await
	}

	pub async fn packet_json(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
		packet_ss58: &str,
		version: Option<u32>,
	) -> Result<JsonValue> {
		let args = self.build_args(auth, |builder| {
			builder
				.push("rtoken", identifier_value(registry_ss58)?)
				.push("ptoken", identifier_value(packet_ss58)?)
				.push("version", super::option_u32_value(version));
			Ok(())
		})?;
		self.client().call_json_raw("Register", "packet", args).await
	}

	async fn call_registry<T: Decode + 'static>(
		&self,
		function: &str,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<T> {
		let args = self.build_args(auth, |builder| {
			builder.push("registry", identifier_value(registry_ss58)?);
			Ok(())
		})?;
		self.client().call_typed("Register", function, args).await
	}

	async fn call_json(
		&self,
		function: &str,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
	) -> Result<JsonValue> {
		let args = self.build_args(auth, |builder| {
			builder.push("registry", identifier_value(registry_ss58)?);
			Ok(())
		})?;
		self.client().call_json_raw("Register", function, args).await
	}

	fn build_args<F>(&self, auth: &auth::ViewAuthorization, f: F) -> Result<scale_value::Value>
	where
		F: FnOnce(&mut super::ArgBuilder) -> Result<()>,
	{
		let mut builder = super::ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		f(&mut builder)?;
		Ok(builder.finish())
	}
}
