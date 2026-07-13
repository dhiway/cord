use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const ORBIS_GENESIS_FIXTURE: &str =
	"0x40519e2e0e894e9b68defd9df6697619116ea693d0864913c98b5c65cd4e511a";
pub const ORBIS_METADATA_HASH: &str =
	"0x8519557667f87eb7ee32cd109d40acfd48d9c53a7ed915fe1733b03573ab9ef9";
pub const ORBIS_DESCRIPTOR_CONTRACT_SHA256: &str =
	"19865403da04a79e045620c2cb1ec9b813c9af7b9b3ba2cb4b9ad2df399ce505";
pub const ORBIS_CHAIN_SPEC_SOURCE_SHA256: &str =
	"b67ec215a4ecea710357537ed304e7d228d3389eabd9eb80dc47dd5c329ae489";
pub const P0_RATIFICATION_PAYLOAD_SHA256: &str =
	"18a7fe95a300632121d0f448cb7bf3e1bc0e6776e254bc26bc1437a5f19c790a";
pub const ORBIS_SPEC_VERSION: u32 = 29;
pub const ORBIS_TRANSACTION_VERSION: u32 = 8;
pub const ORBIS_PARA_ID: u32 = 1006;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeErrorCode {
	PermissionDenied,
	PermissionRevoked,
	ConsentExpired,
	Replay,
	Cancelled,
	Timeout,
	UnsupportedRuntime,
	MetadataMismatch,
	DescriptorMismatch,
	InconsistentSnapshot,
	InvalidInput,
	NotAuthorized,
	NotFound,
	Expired,
	Conflict,
	CapacityExceeded,
	ProofInvalid,
	ContentUnavailable,
	ContentIntegrity,
	UnsupportedSurface,
	RuntimeRejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeError {
	pub version: u8,
	pub code: NativeErrorCode,
	pub message: String,
	pub retryable: bool,
	#[serde(default, skip_serializing_if = "Map::is_empty")]
	pub details: Map<String, Value>,
}

impl NativeError {
	pub fn new(code: NativeErrorCode, message: impl Into<String>) -> Self {
		Self { version: 1, code, message: message.into(), retryable: false, details: Map::new() }
	}

	pub fn retryable(mut self) -> Self {
		self.retryable = true;
		self
	}

	pub fn validate(&self) -> Result<(), Self> {
		if self.version != 1 || self.message.is_empty() || self.message.len() > 512 {
			return Err(Self::new(NativeErrorCode::InvalidInput, "invalid native error envelope"));
		}
		Ok(())
	}
}

impl fmt::Display for NativeError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "{:?}: {}", self.code, self.message)
	}
}

impl std::error::Error for NativeError {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
	Identity,
	Attestation,
	Dotns,
	Storage,
	Content,
	Assets,
	Transaction,
	#[serde(other)]
	Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMethod {
	Read,
	Resolve,
	Fetch,
	Balance,
	Submit,
	#[serde(other)]
	Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Finality {
	Finalized,
	SubmitAndFinalize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkIdentity {
	pub genesis_hash: String,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub metadata_hash: String,
	pub descriptor_contract_sha256: String,
	pub chain_spec_source_sha256: String,
}

impl NetworkIdentity {
	pub fn p0_fixture() -> Self {
		Self {
			genesis_hash: ORBIS_GENESIS_FIXTURE.into(),
			spec_version: ORBIS_SPEC_VERSION,
			transaction_version: ORBIS_TRANSACTION_VERSION,
			metadata_hash: ORBIS_METADATA_HASH.into(),
			descriptor_contract_sha256: ORBIS_DESCRIPTOR_CONTRACT_SHA256.into(),
			chain_spec_source_sha256: ORBIS_CHAIN_SPEC_SOURCE_SHA256.into(),
		}
	}

	pub fn validate(&self) -> Result<(), NativeError> {
		if self.genesis_hash != ORBIS_GENESIS_FIXTURE
			|| self.chain_spec_source_sha256 != ORBIS_CHAIN_SPEC_SOURCE_SHA256
		{
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unrecognized P0 Orbis fixture identity",
			));
		}
		if self.spec_version != ORBIS_SPEC_VERSION
			|| self.transaction_version != ORBIS_TRANSACTION_VERSION
		{
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unsupported Orbis runtime version",
			));
		}
		if self.metadata_hash != ORBIS_METADATA_HASH {
			return Err(NativeError::new(
				NativeErrorCode::MetadataMismatch,
				"Orbis metadata hash mismatch",
			));
		}
		if self.descriptor_contract_sha256 != ORBIS_DESCRIPTOR_CONTRACT_SHA256 {
			return Err(NativeError::new(
				NativeErrorCode::DescriptorMismatch,
				"Orbis descriptor contract hash mismatch",
			));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Consent {
	pub scope: Vec<String>,
	pub expires_at: u64,
	pub nonce: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
	pub version: u8,
	pub request_id: String,
	pub application_id: String,
	pub capability: Capability,
	pub method: HostMethod,
	pub network: NetworkIdentity,
	pub finality: Finality,
	pub payload: Map<String, Value>,
	pub consent: Consent,
}

impl HostRequest {
	pub fn validate(&self) -> Result<(), NativeError> {
		if self.version != 1
			|| !bounded(&self.request_id, 16, 128)
			|| !bounded(&self.application_id, 1, 128)
		{
			return Err(invalid("invalid host request envelope"));
		}
		if self.consent.scope.is_empty()
			|| self.consent.expires_at == 0
			|| !bounded(&self.consent.nonce, 16, 128)
			|| self.consent.scope.iter().any(String::is_empty)
			|| has_duplicates(&self.consent.scope)
		{
			return Err(invalid("invalid consent envelope"));
		}
		self.network.validate()?;
		assert_no_contract_surface(&Value::Object(self.payload.clone()), "payload")?;
		validate_method_payload(self.capability, self.method, &self.payload)
	}

	pub fn scope_name(&self) -> Result<&'static str, NativeError> {
		method_contract(self.capability, self.method).map(|contract| contract.scope)
	}
}

pub fn decode_host_request(source: &str) -> Result<HostRequest, NativeError> {
	let request: HostRequest = serde_json::from_str(source)
		.map_err(|error| invalid(format!("invalid host request JSON: {error}")))?;
	request.validate()?;
	Ok(request)
}

struct MethodContract {
	scope: &'static str,
	fields: &'static [FieldContract],
}

struct FieldContract {
	name: &'static str,
	max: usize,
	kind: FieldKind,
}

#[derive(Clone, Copy)]
enum FieldKind {
	String,
	Hash32,
	Intent,
}

const IDENTITY_FIELDS: &[FieldContract] =
	&[FieldContract { name: "subject_id", max: 128, kind: FieldKind::String }];
const ATTESTATION_FIELDS: &[FieldContract] =
	&[FieldContract { name: "attestation_id", max: 128, kind: FieldKind::String }];
const DOTNS_FIELDS: &[FieldContract] =
	&[FieldContract { name: "name", max: 253, kind: FieldKind::String }];
const STORAGE_FIELDS: &[FieldContract] =
	&[FieldContract { name: "commitment", max: 66, kind: FieldKind::Hash32 }];
const CONTENT_FIELDS: &[FieldContract] =
	&[FieldContract { name: "cid", max: 128, kind: FieldKind::String }];
const ASSET_FIELDS: &[FieldContract] = &[
	FieldContract { name: "asset_id", max: 64, kind: FieldKind::String },
	FieldContract { name: "account", max: 128, kind: FieldKind::String },
];
const TRANSACTION_FIELDS: &[FieldContract] = &[
	FieldContract { name: "operation_id", max: 128, kind: FieldKind::String },
	FieldContract { name: "intent_id", max: 128, kind: FieldKind::Intent },
];

fn method_contract(
	capability: Capability,
	method: HostMethod,
) -> Result<MethodContract, NativeError> {
	let contract = match (capability, method) {
		(Capability::Identity, HostMethod::Read) => {
			MethodContract { scope: "identity:read", fields: IDENTITY_FIELDS }
		},
		(Capability::Attestation, HostMethod::Read) => {
			MethodContract { scope: "attestation:read", fields: ATTESTATION_FIELDS }
		},
		(Capability::Dotns, HostMethod::Resolve) => {
			MethodContract { scope: "dotns:resolve", fields: DOTNS_FIELDS }
		},
		(Capability::Storage, HostMethod::Read) => {
			MethodContract { scope: "storage:read", fields: STORAGE_FIELDS }
		},
		(Capability::Content, HostMethod::Fetch) => {
			MethodContract { scope: "content:fetch", fields: CONTENT_FIELDS }
		},
		(Capability::Assets, HostMethod::Balance) => {
			MethodContract { scope: "assets:balance", fields: ASSET_FIELDS }
		},
		(Capability::Transaction, HostMethod::Submit) => {
			MethodContract { scope: "transaction:submit", fields: TRANSACTION_FIELDS }
		},
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedSurface,
				"unsupported native product method",
			))
		},
	};
	Ok(contract)
}

fn validate_method_payload(
	capability: Capability,
	method: HostMethod,
	payload: &Map<String, Value>,
) -> Result<(), NativeError> {
	let contract = method_contract(capability, method)?;
	if payload.len() != contract.fields.len() {
		return Err(invalid("payload fields do not match the method contract"));
	}
	for field in contract.fields {
		let Some(value) = payload.get(field.name).and_then(Value::as_str) else {
			return Err(invalid(format!("invalid {}", field.name)));
		};
		let valid = match field.kind {
			FieldKind::String => bounded(value, 1, field.max),
			FieldKind::Intent => bounded(value, 16, field.max),
			FieldKind::Hash32 => is_prefixed_hash(value),
		};
		if !valid {
			return Err(invalid(format!("invalid {}", field.name)));
		}
	}
	Ok(())
}

pub fn assert_no_contract_surface(value: &Value, path: &str) -> Result<(), NativeError> {
	match value {
		Value::String(value) => {
			let normalized = normalize(value);
			if normalized.contains("rawscale")
				|| normalized.contains("contractabi")
				|| normalized.contains("contractaddress")
				|| normalized.contains("contractaddr")
				|| normalized.contains("revivecontract")
			{
				return Err(NativeError::new(
					NativeErrorCode::UnsupportedSurface,
					format!("contract-era value at {path}"),
				));
			}
		},
		Value::Array(items) => {
			for (index, item) in items.iter().enumerate() {
				assert_no_contract_surface(item, &format!("{path}[{index}]"))?;
			}
		},
		Value::Object(fields) => {
			for (key, child) in fields {
				let normalized = normalize(key);
				let forbidden = normalized == "scale"
					|| normalized == "rawscale"
					|| normalized == "scalebytes"
					|| normalized.contains("abi")
					|| (normalized.contains("contract")
						&& (normalized.contains("address")
							|| normalized.contains("addr")
							|| normalized.contains("deployment")));
				if forbidden {
					return Err(NativeError::new(
						NativeErrorCode::UnsupportedSurface,
						format!("forbidden product field {path}.{key}"),
					));
				}
				assert_no_contract_surface(child, &format!("{path}.{key}"))?;
			}
		},
		_ => {},
	}
	Ok(())
}

pub fn assert_composite_snapshot<'a>(hashes: &'a [&'a str]) -> Result<&'a str, NativeError> {
	let Some(first) = hashes.first() else {
		return Err(NativeError::new(
			NativeErrorCode::InconsistentSnapshot,
			"composite read did not pin a finalized hash",
		));
	};
	if hashes.iter().any(|hash| hash != first) {
		return Err(NativeError::new(
			NativeErrorCode::InconsistentSnapshot,
			"composite read crossed finalized hashes",
		));
	}
	Ok(first)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLifecycleState {
	Draft,
	Authorized,
	Submitted,
	Included,
	Finalized,
	Rejected,
	Expired,
	Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLifecycle {
	pub version: u8,
	pub intent_id: String,
	pub state: NativeLifecycleState,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub block_hash: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub extrinsic_hash: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub error: Option<NativeError>,
}

impl NativeLifecycle {
	pub fn validate(&self) -> Result<(), NativeError> {
		if self.version != 1
			|| !bounded(&self.intent_id, 16, 128)
			|| self.block_hash.as_deref().is_some_and(|hash| !is_prefixed_hash(hash))
			|| self.extrinsic_hash.as_deref().is_some_and(|hash| !is_prefixed_hash(hash))
		{
			return Err(invalid("invalid native lifecycle envelope"));
		}
		if let Some(error) = &self.error {
			error.validate()?;
		}
		match self.state {
			NativeLifecycleState::Draft | NativeLifecycleState::Authorized
				if self.block_hash.is_some()
					|| self.extrinsic_hash.is_some()
					|| self.error.is_some() =>
			{
				Err(invalid("pre-submit lifecycle state contains terminal evidence"))
			},
			NativeLifecycleState::Included if self.block_hash.is_none() => {
				Err(invalid("included lifecycle requires a block hash"))
			},
			NativeLifecycleState::Finalized
				if self.block_hash.is_none()
					|| self.extrinsic_hash.is_none()
					|| self.error.is_some() =>
			{
				Err(invalid("finalized lifecycle evidence is incomplete"))
			},
			NativeLifecycleState::Rejected | NativeLifecycleState::Expired
				if self.error.is_none() =>
			{
				Err(invalid("terminal failure lifecycle requires an error"))
			},
			NativeLifecycleState::Cancelled
				if self.error.as_ref().map(|error| error.code)
					!= Some(NativeErrorCode::Cancelled) =>
			{
				Err(invalid("cancelled lifecycle requires a cancelled error"))
			},
			_ => Ok(()),
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorContract {
	pub contract_version: u8,
	pub kind: String,
	pub runtime: DescriptorRuntime,
	pub fixture_identity: DescriptorFixtureIdentity,
	pub ratification_payload_sha256: String,
	pub sources: BTreeMap<String, DescriptorSource>,
	pub signed_extension_surfaces: BTreeMap<String, Vec<String>>,
	pub production_papi_descriptor_generated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorRuntime {
	pub name: String,
	pub para_id: u32,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub metadata_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorFixtureIdentity {
	pub status: String,
	pub genesis_identity: String,
	pub chain_spec_source: String,
	pub chain_spec_source_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorSource {
	pub path: String,
	pub sha256: String,
}

pub fn validate_descriptor_contract(descriptor: &DescriptorContract) -> Result<(), NativeError> {
	if descriptor.contract_version != 1
		|| descriptor.kind != "papi-bootstrap-descriptor-contract"
		|| descriptor.runtime.name != "orbis"
		|| descriptor.runtime.para_id != ORBIS_PARA_ID
		|| descriptor.runtime.spec_version != ORBIS_SPEC_VERSION
		|| descriptor.runtime.transaction_version != ORBIS_TRANSACTION_VERSION
		|| descriptor.runtime.metadata_hash != ORBIS_METADATA_HASH
		|| descriptor.fixture_identity.status != "unfinalized-p0-fixture-not-production-genesis"
		|| descriptor.fixture_identity.genesis_identity != ORBIS_GENESIS_FIXTURE
		|| descriptor.fixture_identity.chain_spec_source != "origin/orbis/node/src/chain_spec.rs"
		|| descriptor.fixture_identity.chain_spec_source_sha256 != ORBIS_CHAIN_SPEC_SOURCE_SHA256
		|| descriptor.ratification_payload_sha256 != P0_RATIFICATION_PAYLOAD_SHA256
		|| descriptor.production_papi_descriptor_generated
		|| descriptor.sources.is_empty()
	{
		return Err(NativeError::new(
			NativeErrorCode::DescriptorMismatch,
			"P0 Orbis descriptor contract drift",
		));
	}
	for surface in ["normal", "authorized", "ethereum", "meta_inner"] {
		if descriptor
			.signed_extension_surfaces
			.get(surface)
			.is_none_or(|extensions| extensions.is_empty())
		{
			return Err(NativeError::new(
				NativeErrorCode::DescriptorMismatch,
				"descriptor signed-extension surface is incomplete",
			));
		}
	}
	Ok(())
}

fn invalid(message: impl Into<String>) -> NativeError {
	NativeError::new(NativeErrorCode::InvalidInput, message)
}

fn bounded(value: &str, min: usize, max: usize) -> bool {
	(min..=max).contains(&value.len())
}

fn has_duplicates(values: &[String]) -> bool {
	let mut sorted = values.to_vec();
	sorted.sort();
	sorted.windows(2).any(|pair| pair[0] == pair[1])
}

fn is_prefixed_hash(value: &str) -> bool {
	value.len() == 66
		&& value.starts_with("0x")
		&& value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn normalize(value: &str) -> String {
	value
		.chars()
		.filter(char::is_ascii_alphanumeric)
		.flat_map(char::to_lowercase)
		.collect()
}
