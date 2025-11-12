use super::{ArgBuilder, Query};
use crate::{
	error::{Error, Result},
	types::RegistrySchema,
};
use cord_primitives::{
	registry::{LookupSpecView, RegistryInfoView, RegistryStatus},
	view::DevPacketSnapshot,
	view_api::{RegisterInfoRequest, RegisterLookupSpecsRequest, RegisterPacketRequest, ViewError},
};
use scale_value::Value;

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> RegisterQuery<'a> {
	pub async fn registry_info(&self, req: &RegisterInfoRequest) -> Result<RegistryInfoView> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RegistryInfoView, ViewError> =
			self.query.call_typed("Register", "registry_info", args).await?;
		raw.map_err(|err| view_failure("register.registry_info", err))
	}

	pub async fn schema(&self, req: &RegisterInfoRequest) -> Result<RegistrySchema> {
		let view = self.registry_info(req).await?;
		Ok(RegistrySchema::from_view(&view))
	}

	pub async fn lookup_specs(
		&self,
		req: &RegisterLookupSpecsRequest,
	) -> Result<Vec<LookupSpecView>> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<Vec<LookupSpecView>, ViewError> =
			self.query.call_typed("Register", "lookup_specs_view", args).await?;
		raw.map_err(|err| view_failure("register.lookup_specs_view", err))
	}

	pub async fn packet_snapshot(
		&self,
		req: &RegisterPacketRequest,
	) -> Result<DevPacketSnapshot<RegistryStatus>> {
		let args = self.packet_args(req)?;
		let raw: core::result::Result<DevPacketSnapshot<RegistryStatus>, ViewError> =
			self.query.call_typed("Register", "packet", args).await?;
		raw.map_err(|err| view_failure("register.packet", err))
	}

	fn base_args(
		&self,
		auth: &cord_primitives::view_api::ViewRequestAuth,
		registry: &cord_primitives::identifier::Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(auth)?);
		builder.push("registry", super::identifier_struct_value(registry));
		Ok(builder.finish())
	}

	fn packet_args(&self, req: &RegisterPacketRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::view_auth_value(&req.auth)?);
		builder.push("rtoken", super::identifier_struct_value(&req.registry));
		builder.push("ptoken", super::identifier_struct_value(&req.packet));
		builder.push("version", super::option_u32_value(req.version));
		Ok(builder.finish())
	}
}

fn view_failure(ctx: &str, err: ViewError) -> Error {
	match err {
		ViewError::NotFound => Error::NotFound(format!("{ctx}: not found")),
		ViewError::AuthFailed | ViewError::PermissionDenied => {
			Error::Params(format!("{ctx}: {err:?}"))
		},
		ViewError::InvalidRequest | ViewError::InvalidContext | ViewError::Replay => {
			Error::Params(format!("{ctx}: {err:?}"))
		},
		ViewError::Expired => Error::Params(format!("{ctx}: authorization expired")),
	}
}
