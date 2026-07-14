use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use super::version::{
	ORBIS_ACTIVATION_STATE, ORBIS_METADATA_HASH, ORBIS_PARA_ID, ORBIS_PRODUCTION_ACTIVATION_READY,
	ORBIS_SPEC_VERSION, ORBIS_TRANSACTION_VERSION,
};

pub use super::version::ORBIS_CANDIDATE_GENESIS_HEADER_HASH;
pub const ORBIS_DESCRIPTOR_CONTRACT_SHA256: &str =
	"ceeb91edb8cd277b12c933fdb789185acd824a2b3d68eccb7cd43a17979df609";
pub const ORBIS_CHAIN_SPEC_SOURCE_SHA256: &str =
	"7f0400a126b7e731e8bfab1a3fa8852b599619d112324fa411d72afa66f480bb";
pub const NATIVE_SDK_RATIFICATION_PAYLOAD_SHA256: &str =
	"27c6effe52c60fade8e9fe341ffe874cb6e286e97944c51c7f0532052361e9fe";

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NativeHostMethod(String);

impl NativeHostMethod {
	pub fn new(value: impl Into<String>) -> Self {
		Self(value.into())
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
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
	pub activation_state: NetworkActivationState,
	pub production_activation_ready: bool,
	pub access_mode: NetworkAccessMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkActivationState {
	CandidatePending,
	ProductionApproved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkAccessMode {
	Candidate,
	Production,
}

impl NetworkIdentity {
	pub fn orbis_candidate() -> Self {
		Self {
			genesis_hash: ORBIS_CANDIDATE_GENESIS_HEADER_HASH.into(),
			spec_version: ORBIS_SPEC_VERSION,
			transaction_version: ORBIS_TRANSACTION_VERSION,
			metadata_hash: ORBIS_METADATA_HASH.into(),
			descriptor_contract_sha256: ORBIS_DESCRIPTOR_CONTRACT_SHA256.into(),
			chain_spec_source_sha256: ORBIS_CHAIN_SPEC_SOURCE_SHA256.into(),
			activation_state: NetworkActivationState::CandidatePending,
			production_activation_ready: ORBIS_PRODUCTION_ACTIVATION_READY,
			access_mode: NetworkAccessMode::Candidate,
		}
	}

	pub fn orbis_production() -> Self {
		Self { access_mode: NetworkAccessMode::Production, ..Self::orbis_candidate() }
	}

	pub fn validate(&self) -> Result<(), NativeError> {
		if self.genesis_hash != ORBIS_CANDIDATE_GENESIS_HEADER_HASH
			|| self.chain_spec_source_sha256 != ORBIS_CHAIN_SPEC_SOURCE_SHA256
		{
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unrecognized Orbis clean-break prelaunch identity",
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
		let frozen_activation = match ORBIS_ACTIVATION_STATE {
			"candidate-pending" => NetworkActivationState::CandidatePending,
			"production-approved" => NetworkActivationState::ProductionApproved,
			_ => {
				return Err(NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"unsupported frozen Orbis activation state",
				))
			},
		};
		if self.activation_state != frozen_activation
			|| self.production_activation_ready != ORBIS_PRODUCTION_ACTIVATION_READY
			|| (frozen_activation == NetworkActivationState::CandidatePending
				&& ORBIS_PRODUCTION_ACTIVATION_READY)
			|| (frozen_activation == NetworkActivationState::ProductionApproved
				&& !ORBIS_PRODUCTION_ACTIVATION_READY)
		{
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"Orbis activation state does not match the signed SDK freeze",
			));
		}
		match self.access_mode {
			NetworkAccessMode::Candidate
				if self.activation_state == NetworkActivationState::CandidatePending
					&& !self.production_activation_ready => {},
			NetworkAccessMode::Production
				if self.activation_state == NetworkActivationState::ProductionApproved
					&& self.production_activation_ready => {},
			NetworkAccessMode::Candidate => {
				return Err(NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"candidate access is unavailable for the activated production network",
				));
			},
			NetworkAccessMode::Production => {
				return Err(NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"production access rejects the unsigned candidate/PENDING network",
				));
			},
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
	pub method: NativeHostMethod,
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
		validate_method_payload(self.capability, &self.method, self.finality, &self.payload)
	}

	pub fn scope_name(&self) -> Result<String, NativeError> {
		method_contract(self.capability, &self.method).map(|contract| contract.scope)
	}
}

pub fn decode_host_request(source: &str) -> Result<HostRequest, NativeError> {
	let request: HostRequest = serde_json::from_str(source)
		.map_err(|error| invalid(format!("invalid host request JSON: {error}")))?;
	request.validate()?;
	Ok(request)
}

struct MethodContract {
	scope: String,
	fields: Vec<String>,
	finality: Finality,
}

fn method_contract(
	capability: Capability,
	method: &NativeHostMethod,
) -> Result<MethodContract, NativeError> {
	let capability_name = match capability {
		Capability::Identity => "identity",
		Capability::Attestation => "attestation",
		Capability::Dotns => "dotns",
		Capability::Storage => "storage",
		Capability::Content => "content",
		Capability::Assets => "assets",
		Capability::Transaction => "transaction",
		Capability::Unsupported => return Err(unsupported_method()),
	};
	let route_contract: Value =
		serde_json::from_str(include_str!("../../../docs/sdk/native-route-contract.json"))
			.map_err(|_| {
				NativeError::new(NativeErrorCode::DescriptorMismatch, "invalid route contract")
			})?;
	if route_contract["network"]["metadata_hash"].as_str() != Some(ORBIS_METADATA_HASH) {
		return Err(NativeError::new(
			NativeErrorCode::MetadataMismatch,
			"route contract metadata binding mismatch",
		));
	}
	let Some(entry) = route_contract["routes"].as_array().and_then(|routes| {
		routes.iter().find(|entry| {
			entry["capability"].as_str() == Some(capability_name)
				&& entry["method"].as_str() == Some(method.as_str())
		})
	}) else {
		return Err(unsupported_method());
	};
	let finality = match entry["finality"].as_str() {
		Some("finalized") => Finality::Finalized,
		Some("submit-and-finalize") => Finality::SubmitAndFinalize,
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::DescriptorMismatch,
				"invalid descriptor finality",
			))
		},
	};
	let fields = entry["parameters"]
		.as_array()
		.ok_or_else(|| {
			NativeError::new(NativeErrorCode::DescriptorMismatch, "invalid descriptor fields")
		})?
		.iter()
		.map(|field| {
			field["name"].as_str().map(str::to_owned).ok_or_else(|| {
				NativeError::new(NativeErrorCode::DescriptorMismatch, "invalid descriptor field")
			})
		})
		.collect::<Result<Vec<_>, _>>()?;
	Ok(MethodContract { scope: format!("{capability_name}:{}", method.as_str()), fields, finality })
}

pub fn validate_method_scope(scope: &str) -> Result<(), NativeError> {
	let (capability, method) = scope.split_once(':').ok_or_else(unsupported_method)?;
	if method.is_empty() || method.contains(':') {
		return Err(unsupported_method());
	}
	let capability = match capability {
		"identity" => Capability::Identity,
		"attestation" => Capability::Attestation,
		"dotns" => Capability::Dotns,
		"storage" => Capability::Storage,
		"content" => Capability::Content,
		"assets" => Capability::Assets,
		"transaction" => Capability::Transaction,
		_ => return Err(unsupported_method()),
	};
	method_contract(capability, &NativeHostMethod::new(method)).map(|_| ())
}

fn validate_method_payload(
	capability: Capability,
	method: &NativeHostMethod,
	finality: Finality,
	payload: &Map<String, Value>,
) -> Result<(), NativeError> {
	let contract = method_contract(capability, method)?;
	if finality != contract.finality
		|| payload.len() != contract.fields.len()
		|| contract.fields.iter().any(|field| !payload.contains_key(field))
	{
		return Err(invalid("payload fields do not match the method contract"));
	}
	Ok(())
}

fn unsupported_method() -> NativeError {
	NativeError::new(NativeErrorCode::UnsupportedSurface, "unsupported native product method")
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
	pub release: String,
	pub first_supported_native_sdk: bool,
	pub runtime: DescriptorRuntime,
	pub fixture_identity: DescriptorFixtureIdentity,
	pub network_activation: DescriptorNetworkActivation,
	pub ratification_payload_sha256: String,
	pub sources: BTreeMap<String, DescriptorSource>,
	pub signed_extension_surfaces: BTreeMap<String, Vec<String>>,
	pub native_host_contract: DescriptorNativeHostContract,
	pub descriptor_provenance: DescriptorProvenance,
	pub production_papi_descriptor_generated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorProvenance {
	pub runtime_metadata_binding: String,
	pub method_inventory: String,
	pub drift_validation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorNativeHostContract {
	pub version: u8,
	pub method_count: usize,
	pub page_limit: u32,
	pub payload_validation: String,
	pub methods: Vec<DescriptorNativeMethod>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorNativeMethod {
	pub capability: String,
	pub method: String,
	pub finality: String,
	pub payload_fields: Vec<String>,
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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DescriptorNetworkActivation {
	pub state: String,
	pub production_activation_ready: bool,
	pub source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorFixtureIdentity {
	pub status: String,
	pub genesis_identity: String,
	pub genesis_state_root: String,
	pub candidate_identity_source: String,
	pub candidate_identity_sha256: String,
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
		|| descriptor.kind != "cord-native-host-contract-manifest"
		|| descriptor.release != "origin-orbis-native-v1"
		|| !descriptor.first_supported_native_sdk
		|| descriptor.runtime.name != "orbis"
		|| descriptor.runtime.para_id != ORBIS_PARA_ID
		|| descriptor.runtime.spec_version != ORBIS_SPEC_VERSION
		|| descriptor.runtime.transaction_version != ORBIS_TRANSACTION_VERSION
		|| descriptor.runtime.metadata_hash != ORBIS_METADATA_HASH
		|| descriptor.fixture_identity.status
			!= "deterministic-clean-break-candidate-not-production-approved"
		|| descriptor.fixture_identity.genesis_identity != ORBIS_CANDIDATE_GENESIS_HEADER_HASH
		|| descriptor.fixture_identity.genesis_state_root
			!= super::version::ORBIS_CANDIDATE_GENESIS_STATE_ROOT
		|| descriptor.fixture_identity.candidate_identity_source
			!= "docs/genesis/orbis-candidate-genesis-identity.json"
		|| descriptor.fixture_identity.candidate_identity_sha256
			!= super::version::ORBIS_CANDIDATE_GENESIS_IDENTITY_SHA256
		|| descriptor.fixture_identity.chain_spec_source != "origin/orbis/node/src/chain_spec.rs"
		|| descriptor.fixture_identity.chain_spec_source_sha256 != ORBIS_CHAIN_SPEC_SOURCE_SHA256
		|| descriptor.network_activation.state != ORBIS_ACTIVATION_STATE
		|| descriptor.network_activation.production_activation_ready
			!= ORBIS_PRODUCTION_ACTIVATION_READY
		|| descriptor.network_activation.source
			!= "docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json"
		|| descriptor.ratification_payload_sha256 != NATIVE_SDK_RATIFICATION_PAYLOAD_SHA256
		|| descriptor.native_host_contract.version != 1
		|| descriptor.native_host_contract.method_count
			!= descriptor.native_host_contract.methods.len()
		|| descriptor.native_host_contract.methods.is_empty()
		|| descriptor.native_host_contract.page_limit != 100
		|| descriptor.native_host_contract.payload_validation
			!= "closed-shape-plus-core-native-types-v1"
		|| descriptor.descriptor_provenance.runtime_metadata_binding
			!= "reproduced-rfc78-wasm-metadata-hash"
		|| descriptor.descriptor_provenance.method_inventory
			!= "authoritative-typed-native-route-contract"
		|| descriptor.descriptor_provenance.drift_validation
			!= "metadata-hash-pallet-call-index-runtime-api-and-rust-typescript-route-harness"
		|| descriptor.production_papi_descriptor_generated
		|| descriptor.sources.is_empty()
	{
		return Err(NativeError::new(
			NativeErrorCode::DescriptorMismatch,
			"Orbis native SDK descriptor contract drift",
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
