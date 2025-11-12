use super::{ArgBuilder, Query};
use crate::{
	error::{Error, Result},
	types::RegistrySchema,
};
use cord_primitives::{
	registry::{LookupSpecView, RegistryInfoView, RegistryStatus},
	view::DevPacketSnapshot,
	view_api::{
		AuthorizationError, RegisterDetailsRequest, RegisterLookupSpecsRequest,
		RegisterPacketSnapshotRequest,
	},
};
use scale_value::Value;

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> RegisterQuery<'a> {
	pub async fn details(&self, req: &RegisterDetailsRequest) -> Result<RegistryInfoView> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RegistryInfoView, AuthorizationError> =
			self.query.call_typed("Register", "details", args).await?;
		raw.map_err(|err| view_failure("register.details", err))
	}

	pub async fn schema(&self, req: &RegisterDetailsRequest) -> Result<RegistrySchema> {
		let view = self.details(req).await?;
		Ok(RegistrySchema::from_view(&view))
	}

	pub async fn lookup_specs(
		&self,
		req: &RegisterLookupSpecsRequest,
	) -> Result<Vec<LookupSpecView>> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<Vec<LookupSpecView>, AuthorizationError> =
			self.query.call_typed("Register", "lookup_specs", args).await?;
		raw.map_err(|err| view_failure("register.lookup_specs", err))
	}

	pub async fn packet_snapshot(
		&self,
		req: &RegisterPacketSnapshotRequest,
	) -> Result<DevPacketSnapshot<RegistryStatus>> {
		let args = self.packet_args(req)?;
		let raw: core::result::Result<DevPacketSnapshot<RegistryStatus>, AuthorizationError> =
			self.query.call_typed("Register", "packet_snapshot_dev", args).await?;
		raw.map_err(|err| view_failure("register.packet_snapshot_dev", err))
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
