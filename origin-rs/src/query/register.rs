use super::{ArgBuilder, Query};
use crate::{
	api::runtime,
	error::{Error, Result},
	runtime_helpers::{
		attribute_optional, bounded_bytes_vec, bounded_iter, element_type_to_sdk, identifier_bytes,
	},
	types::RegistrySchema,
};
use origin_primitives::{
	identifier::Ss58Identifier,
	registry::{
		LookupSpecView, RegistryAttributeView, RegistryInfoView, RegistryKind, RegistryStatus,
	},
	view::{
		dev_packet_snapshot_from, AttributeValueView, DevPacketSnapshot, ElementView,
		PacketStateView,
	},
	view_api::{
		AuthorizationError, RegisterDetailsRequest, RegisterLookupSpecsRequest,
		RegisterPacketSnapshotByTokenRequest, RegisterPacketSnapshotRequest,
	},
};
use scale_value::Value;

pub type PacketSnapshotView = DevPacketSnapshot<RegistryStatus>;

#[derive(scale_decode::DecodeAsType)]
struct PacketSnapshotRecord {
	state: PacketStateRecord,
	registry_status: RegistryStatus,
}

#[derive(scale_decode::DecodeAsType)]
struct PacketStateRecord {
	registry: RuntimeIdentifier,
	controller: RuntimeIdentifier,
	status: RuntimePacketStatus,
	version: u32,
	attributes_hash: [u8; 32],
	attributes: RuntimeAttributes,
}

/// Entry point for register-specific view helpers.
pub struct RegisterQuery<'a> {
	pub(crate) query: &'a Query<'a>,
}

impl<'a> RegisterQuery<'a> {
	pub async fn details(&self, req: &RegisterDetailsRequest) -> Result<RegistryInfoView> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<RuntimeRegistryInfo, AuthorizationError> =
			self.query.call_result("Register", "details", args.clone()).await?;
		let info = match raw {
			Ok(runtime_info) => runtime_info,
			Err(err) => return Err(super::view_failure("register.details", err)),
		};
		if std::env::var("ORIGIN_RS_DEBUG_REGISTRY").is_ok() {
			let view = self.query.call_view_bytes("Register", "details", args).await?;
			println!("\n🔬 Raw Register::details SCALE (hex): 0x{}", hex::encode(&view.data));
		}
		Ok(registry_view_from_runtime(&info)?)
	}

	pub async fn schema(&self, req: &RegisterDetailsRequest) -> Result<RegistrySchema> {
		let info = self.details(req).await?;
		Ok(RegistrySchema::from_view(&info))
	}

	pub async fn lookup_specs(
		&self,
		req: &RegisterLookupSpecsRequest,
	) -> Result<Vec<LookupSpecView>> {
		let args = self.base_args(&req.auth, &req.registry)?;
		let raw: core::result::Result<Vec<LookupSpecView>, AuthorizationError> =
			self.query.call_result("Register", "lookup_specs", args).await?;
		raw.map_err(|err| super::view_failure("register.lookup_specs", err))
	}

	pub async fn packet_snapshot(
		&self,
		req: &RegisterPacketSnapshotRequest,
	) -> Result<PacketSnapshotView> {
		let args = self.packet_args(req)?;
		let raw: core::result::Result<PacketSnapshotRecord, AuthorizationError> =
			self.query.call_result("Register", "packet_snapshot", args).await?;
		match raw {
			Ok(record) => Ok(dev_packet_snapshot_from(
				&packet_state_view_from_record(&record.state)?,
				record.registry_status,
			)),
			Err(err) => Err(super::view_failure("register.packet_snapshot", err)),
		}
	}

	pub async fn packet_snapshot_by_token(
		&self,
		req: &RegisterPacketSnapshotByTokenRequest,
	) -> Result<Option<PacketSnapshotView>> {
		let args = self.packet_by_token_args(req)?;
		let raw: core::result::Result<Option<PacketSnapshotRecord>, AuthorizationError> =
			self.query.call_result("Register", "packet_snapshot_by_token", args).await?;
		match raw {
			Ok(Some(record)) => {
				let state = packet_state_view_from_record(&record.state)?;
				Ok(Some(dev_packet_snapshot_from(&state, record.registry_status)))
			},
			Ok(None) => Ok(None),
			Err(err) => Err(super::view_failure("register.packet_snapshot_by_token", err)),
		}
	}

	fn base_args(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		registry: &origin_primitives::identifier::Ss58Identifier,
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

	fn packet_by_token_args(&self, req: &RegisterPacketSnapshotByTokenRequest) -> Result<Value> {
		let mut builder = ArgBuilder::default();
		builder.push("auth", super::authorization_value(&req.auth)?);
		builder.push("token", super::identifier_struct_value(&req.token));
		builder.push("version", super::option_u32_value(req.version));
		Ok(builder.finish())
	}
}

type RuntimeRegistryInfo = runtime::runtime_types::pallet_register::register::RegistryInfo;
type RuntimeAttributeSpec = runtime::runtime_types::pallet_register::register::AttributeSpec;
type RuntimeLookupSpec = runtime::runtime_types::pallet_register::register::LookupSpec;
type RuntimeElement = runtime::runtime_types::origin_primitives::element::Elum;
type RuntimeIdentifier = runtime::runtime_types::origin_primitives::identifier::Ss58Identifier;
type RuntimeRegistryKind = runtime::runtime_types::oridin_primitives::registry::RegistryKind;
type RuntimeRegistryStatus = runtime::runtime_types::origin_primitives::registry::RegistryStatus;
type RuntimePacketStatus = runtime::runtime_types::origin_primitives::packet::PacketStatus;
type RuntimeAttributes = runtime::runtime_types::origin_primitives::packet::Attributes;

fn registry_view_from_runtime(info: &RuntimeRegistryInfo) -> Result<RegistryInfoView> {
	let attributes = bounded_iter(&info.attributes)
		.map(attribute_view_from_runtime)
		.collect::<Vec<_>>();
	let lookup_specs = bounded_iter(&info.lookup_specs)
		.map(lookup_spec_from_runtime)
		.collect::<Vec<_>>();
	Ok(RegistryInfoView {
		info: element_view_from_runtime(&info.info)?,
		maintainer: identifier_from_runtime(&info.maintainer)?,
		attributes,
		token_spec: lookup_spec_from_runtime(&info.token_spec),
		lookup_specs,
		kind: registry_kind_from_runtime(&info.kind),
		status: registry_status_from_runtime(&info.status),
	})
}

fn attribute_view_from_runtime(spec: &RuntimeAttributeSpec) -> RegistryAttributeView {
	RegistryAttributeView {
		key: bounded_bytes_vec(&spec.key),
		kind: element_type_to_sdk(&spec.kind),
		optional: attribute_optional(&spec.flags),
	}
}

fn lookup_spec_from_runtime(spec: &RuntimeLookupSpec) -> LookupSpecView {
	use runtime::runtime_types::pallet_register::register::LookupSpec as RuntimeLookup;
	match spec {
		RuntimeLookup::Single(attr) => LookupSpecView::Single(bounded_bytes_vec(attr)),
		RuntimeLookup::Combo(list) => {
			let keys = bounded_iter(list).map(|attr| bounded_bytes_vec(attr)).collect::<Vec<_>>();
			LookupSpecView::Combo(keys)
		},
	}
}

fn element_view_from_runtime(element: &RuntimeElement) -> Result<ElementView> {
	use runtime::runtime_types::origin_primitives::element::Elum as RuntimeElementEnum;
	let view = match element {
		RuntimeElementEnum::None => ElementView::None,
		RuntimeElementEnum::Raw(bytes) => ElementView::Raw(bounded_bytes_vec(bytes)),
		RuntimeElementEnum::Bool(flag) => ElementView::Bool(*flag != 0),
		RuntimeElementEnum::U64(bytes) => ElementView::U64(u64::from_le_bytes(*bytes)),
		RuntimeElementEnum::U128(bytes) => ElementView::U128(u128::from_le_bytes(*bytes)),
		RuntimeElementEnum::Hash(digest) => ElementView::Hash(*digest),
		RuntimeElementEnum::Token(identifier) =>
			ElementView::Token(identifier_from_runtime(identifier)?),
		RuntimeElementEnum::CID(bytes) => ElementView::Cid(bounded_bytes_vec(bytes)),
	};
	Ok(view)
}

fn identifier_from_runtime(identifier: &RuntimeIdentifier) -> Result<Ss58Identifier> {
	let raw = identifier_bytes(identifier).to_vec();
	Ss58Identifier::try_from(raw)
		.map_err(|e| Error::ViewDecode(format!("invalid registry identifier: {e:?}")))
}

fn registry_kind_from_runtime(kind: &RuntimeRegistryKind) -> RegistryKind {
	use runtime::runtime_types::origin_primitives::registry::RegistryKind as RuntimeKindEnum;
	match kind {
		RuntimeKindEnum::Raw => RegistryKind::Raw,
		RuntimeKindEnum::Token => RegistryKind::Token,
		RuntimeKindEnum::Hash => RegistryKind::Hash,
	}
}

fn registry_status_from_runtime(status: &RuntimeRegistryStatus) -> RegistryStatus {
	use runtime::runtime_types::origin_primitives::registry::RegistryStatus as RuntimeStatusEnum;
	match status {
		RuntimeStatusEnum::Active => RegistryStatus::Active,
		RuntimeStatusEnum::Revoked => RegistryStatus::Revoked,
		RuntimeStatusEnum::Deleted => RegistryStatus::Deleted,
	}
}

fn packet_state_view_from_record(record: &PacketStateRecord) -> Result<PacketStateView> {
	let attributes = packet_attributes_from_runtime(&record.attributes)?;
	Ok(PacketStateView {
		registry: identifier_from_runtime(&record.registry)?,
		controller: identifier_from_runtime(&record.controller)?,
		status: packet_status_from_runtime(&record.status),
		version: record.version,
		attributes_hash: record.attributes_hash.to_vec(),
		attributes,
	})
}

fn packet_status_from_runtime(
	status: &RuntimePacketStatus,
) -> origin_primitives::packet::PacketStatus {
	use runtime::runtime_types::origin_primitives::packet::PacketStatus as RuntimePacketStatusEnum;
	match status {
		RuntimePacketStatusEnum::Active => origin_primitives::packet::PacketStatus::Active,
		RuntimePacketStatusEnum::Revoked => origin_primitives::packet::PacketStatus::Revoked,
		RuntimePacketStatusEnum::Deleted => origin_primitives::packet::PacketStatus::Deleted,
	}
}

fn packet_attributes_from_runtime(attrs: &RuntimeAttributes) -> Result<Vec<AttributeValueView>> {
	attrs
		.0
		 .0
		.iter()
		.map(|(key, value)| {
			Ok(AttributeValueView {
				key: bounded_bytes_vec(key),
				value: element_view_from_runtime(value)?,
			})
		})
		.collect()
}
