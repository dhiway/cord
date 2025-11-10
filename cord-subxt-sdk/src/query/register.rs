use super::{auth, Query};
use crate::{
	error::{Error, Result},
	types::{identifier_value, RegistrySchema},
};
use codec::Decode;
use cord_primitives::{
	registry::{LookupSpecView, RegistryInfoView, RegistryStatus},
	view::DevPacketSnapshot,
};

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

	pub async fn packet_snapshot(
		&self,
		auth: &auth::ViewAuthorization,
		registry_ss58: &str,
		packet_ss58: &str,
		version: Option<u32>,
	) -> Result<DevPacketSnapshot<RegistryStatus>> {
		let args = self.build_args(auth, |builder| {
			builder
				.push("rtoken", identifier_value(registry_ss58)?)
				.push("ptoken", identifier_value(packet_ss58)?)
				.push("version", super::option_u32_value(version));
			Ok(())
		})?;
		let result: Option<DevPacketSnapshot<RegistryStatus>> =
			self.client().call_typed("Register", "packet", args).await?;
		result.ok_or_else(|| Error::NotFound("register.packet returned none".into()))
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
