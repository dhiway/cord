use super::{ArgBuilder, Query};
use crate::{
	error::{Error, Result},
	types::RegistrySchema,
};
use hex;
use origin_primitives::{
	identifier::Ss58Identifier,
	registry::{LookupSpecView, RegistryInfoView, RegistryStatus},
	view::{DevPacketSnapshot, PacketMetadataView},
	view_api::{
		AuthorizationError, AuthorizationRequest, RegisterDetailsRequest,
		RegisterListByDigestRequest, RegisterListByTokenRequest, RegisterLookupSpecsRequest,
		RegisterPacketSnapshotByTokenRequest, RegisterPacketSnapshotRequest,
	},
};
use scale_value::Value;

pub type PacketSnapshotView = DevPacketSnapshot<RegistryStatus>;

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
	pub(crate) supported: bool,
}

type LookupDigest = [u8; 32];

impl<'a> RegisterQuery<'a> {
	fn ensure_supported(&self) -> Result<()> {
		if self.supported {
			Ok(())
		} else {
			Err(Error::Params(
				"register queries require origin-hub flavor (not available on origin relay)".into(),
			))
		}
	}

	pub async fn details(&self, req: &RegisterDetailsRequest) -> Result<RegistryInfoView> {
		self.ensure_supported()?;
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RegistryInfoView, AuthorizationError> =
			self.query.call_result("Register", "details", args.clone()).await?;
		let info = match raw {
			Ok(view) => view,
			Err(err) => return Err(super::view_failure("register.details", err)),
		};
		if std::env::var("ORIGIN_RS_DEBUG_REGISTRY").is_ok() {
			let view = self.query.call_view_bytes("Register", "details", args).await?;
			println!("\n🔬 Raw Register::details SCALE (hex): 0x{}", hex::encode(&view.data));
		}
		Ok(info)
	}

	pub async fn schema(&self, req: &RegisterDetailsRequest) -> Result<RegistrySchema> {
		self.ensure_supported()?;
		let info = self.details(req).await?;
		Ok(RegistrySchema::from_view(&info))
	}

	pub async fn lookup_specs(
		&self,
		req: &RegisterLookupSpecsRequest,
	) -> Result<Vec<LookupSpecView>> {
		self.ensure_supported()?;
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<Vec<LookupSpecView>, AuthorizationError> =
			self.query.call_result("Register", "lookup_specs", args).await?;
		raw.map_err(|err| super::view_failure("register.lookup_specs", err))
	}

	pub async fn packet_snapshot(
		&self,
		req: &RegisterPacketSnapshotRequest,
	) -> Result<PacketSnapshotView> {
		self.ensure_supported()?;
		let args = self.packet_args(req)?;
		let raw: core::result::Result<PacketSnapshotView, AuthorizationError> =
			self.query.call_result("Register", "packet_snapshot", args).await?;
		raw.map_err(|err| super::view_failure("register.packet_snapshot", err))
	}

	pub async fn packet_snapshot_by_token(
		&self,
		req: &RegisterPacketSnapshotByTokenRequest,
	) -> Result<Option<PacketSnapshotView>> {
		self.ensure_supported()?;
		let args = self.packet_by_token_args(req)?;
		let raw: core::result::Result<Option<PacketSnapshotView>, AuthorizationError> =
			self.query.call_result("Register", "packet_snapshot_by_token", args).await?;
		match raw {
			Ok(Some(snapshot)) => Ok(Some(snapshot)),
			Ok(None) => Ok(None),
			Err(err) => Err(super::view_failure("register.packet_snapshot_by_token", err)),
		}
	}

	pub async fn packet_metadata(
		&self,
		auth: &AuthorizationRequest,
		registry: &Ss58Identifier,
		packet: &Ss58Identifier,
	) -> Result<PacketMetadataView> {
		self.ensure_supported()?;
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("rtoken", super::identifier_struct_value(registry));
		builder.push("ptoken", super::identifier_struct_value(packet));
		let args = builder.finish();
		let raw: core::result::Result<PacketMetadataView, AuthorizationError> =
			self.query.call_result("Register", "packet_metadata", args).await?;
		raw.map_err(|err| super::view_failure("register.packet_metadata", err))
	}

	pub async fn overview(
		&self,
		auth: &AuthorizationRequest,
		registry: &Ss58Identifier,
	) -> Result<(RegistryInfoView, Vec<LookupSpecView>)> {
		self.ensure_supported()?;
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(auth)?);
		builder.push("registry", super::identifier_struct_value(registry));
		let args = builder.finish();
		let raw: core::result::Result<(RegistryInfoView, Vec<LookupSpecView>), AuthorizationError> =
			self.query.call_result("Register", "overview", args).await?;
		raw.map_err(|err| super::view_failure("register.overview", err))
	}

	pub async fn list_by_token(
		&self,
		req: &RegisterListByTokenRequest,
	) -> Result<(Vec<PacketSnapshotView>, Option<Ss58Identifier>)> {
		self.ensure_supported()?;
		let args = self.list_by_token_args(req)?;
		let raw: core::result::Result<
			(Vec<PacketSnapshotView>, Option<Ss58Identifier>),
			AuthorizationError,
		> = self.query.call_result("Register", "list_by_token", args).await?;
		raw.map_err(|err| super::view_failure("register.list_by_token", err))
	}

	pub async fn list_by_digest(
		&self,
		req: &RegisterListByDigestRequest,
	) -> Result<(Vec<PacketSnapshotView>, Option<LookupDigest>)> {
		self.ensure_supported()?;
		let args = self.list_by_digest_args(req)?;
		let raw: core::result::Result<
			(Vec<PacketSnapshotView>, Option<LookupDigest>),
			AuthorizationError,
		> = self.query.call_result("Register", "list_by_digest", args).await?;
		raw.map_err(|err| super::view_failure("register.list_by_digest", err))
	}

	fn base_args(&self, auth: &AuthorizationRequest, registry: &Ss58Identifier) -> Result<Value> {
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

	fn packet_by_token_args(&self, req: &RegisterPacketSnapshotByTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("token", super::identifier_struct_value(&req.token));
		builder.push("version", super::option_u32_value(req.version));
		Ok(builder.finish())
	}

	fn list_by_token_args(&self, req: &RegisterListByTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("token_prefix", super::hex_arg(req.token_prefix.as_slice()));
		builder.push("version", super::option_u32_value(req.version));
		builder.push(
			"cursor",
			super::option_value(req.cursor.as_ref().map(super::identifier_struct_value)),
		);
		builder.push("limit", super::option_u32_value(req.limit));
		Ok(builder.finish())
	}

	fn list_by_digest_args(&self, req: &RegisterListByDigestRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("digest_prefix", super::hex_arg(req.digest_prefix.as_slice()));
		builder.push("version", super::option_u32_value(req.version));
		builder
			.push("cursor", super::option_value(req.cursor.map(|d| Value::from_bytes(d.to_vec()))));
		builder.push("limit", super::option_u32_value(req.limit));
		Ok(builder.finish())
	}
}
