use super::{ArgBuilder, Query};
use crate::{
	api::runtime,
	error::{Error, Result},
	types::RegistrySchema,
};
use cord_primitives::view_api::{
	AuthorizationError, RegisterDetailsRequest, RegisterLookupSpecsRequest,
	RegisterPacketSnapshotRequest,
};
use scale_value::Value;

pub type RuntimeRegistryInfo = runtime::runtime_types::pallet_register::register::RegistryInfo;
pub type RuntimeLookupSpec = runtime::runtime_types::pallet_register::register::LookupSpec;
pub type RuntimeLookupSpecList =
	runtime::runtime_types::bounded_collections::bounded_vec::BoundedVec<RuntimeLookupSpec>;
pub type RuntimePacketSnapshot = runtime::runtime_types::pallet_register::packet::PacketSnapshot;

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> RegisterQuery<'a> {
	pub async fn details(&self, req: &RegisterDetailsRequest) -> Result<RuntimeRegistryInfo> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RuntimeRegistryInfo, AuthorizationError> =
			self.query.call_typed("Register", "details", args).await?;
		raw.map_err(|err| view_failure("register.details", err))
	}

	pub async fn schema(&self, req: &RegisterDetailsRequest) -> Result<RegistrySchema> {
		let info = self.details(req).await?;
		Ok(RegistrySchema::from_runtime(&info))
	}

	pub async fn lookup_specs(
		&self,
		req: &RegisterLookupSpecsRequest,
	) -> Result<RuntimeLookupSpecList> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RuntimeLookupSpecList, AuthorizationError> =
			self.query.call_typed("Register", "lookup_specs", args).await?;
		raw.map_err(|err| view_failure("register.lookup_specs", err))
	}

	pub async fn packet_snapshot(
		&self,
		req: &RegisterPacketSnapshotRequest,
	) -> Result<RuntimePacketSnapshot> {
		let args = self.packet_args(req)?;
		let raw: core::result::Result<RuntimePacketSnapshot, AuthorizationError> =
			self.query.call_typed("Register", "packet_snapshot", args).await?;
		raw.map_err(|err| view_failure("register.packet_snapshot", err))
	}

	fn base_args(
		&self,
		auth: &cord_primitives::view_api::AuthorizationRequest,
		registry: &cord_primitives::identifier::Ss58Identifier,
	) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("registry", super::identifier_struct_value(registry));
		Ok(builder.finish())
	}

	fn packet_args(&self, req: &RegisterPacketSnapshotRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("rtoken", super::identifier_struct_value(&req.registry));
		builder.push("ptoken", super::identifier_struct_value(&req.packet));
		builder.push("version", super::option_u32_value(req.version));
		Ok(builder.finish())
	}
}

fn view_failure(ctx: &str, err: AuthorizationError) -> Error {
	match err {
		AuthorizationError::NotFound => Error::NotFound(format!("{ctx}: not found")),
		AuthorizationError::Unauthorized => Error::Params(format!("{ctx}: unauthorized")),
		AuthorizationError::InvalidInput => Error::Params(format!("{ctx}: invalid input")),
		AuthorizationError::TooLarge => Error::Params(format!("{ctx}: result too large")),
		AuthorizationError::Internal => Error::ViewDecode(format!("{ctx}: internal error")),
	}
}
