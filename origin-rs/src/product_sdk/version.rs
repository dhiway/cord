//! Version coherence contract for the first supported clean-break Origin/Orbis SDK surface.

pub const NATIVE_SDK_CONTRACT_VERSION: u32 = 1;
pub const NATIVE_SDK_RELEASE: &str = "origin-orbis-native-v1";
pub const NATIVE_SDK_PACKAGE_VERSION: &str = "0.9.9";

pub const ORIGIN_SPEC_VERSION: u32 = 9901;
pub const ORIGIN_TRANSACTION_VERSION: u32 = 2;
pub const ORIGIN_ACTIVATION_STATE: &str = "candidate-pending";
pub const ORIGIN_PRODUCTION_ACTIVATION_READY: bool = false;
pub const ORBIS_PARA_ID: u32 = 1006;
pub const ORBIS_SPEC_VERSION: u32 = 29;
pub const ORBIS_TRANSACTION_VERSION: u32 = 8;
pub const ORBIS_METADATA_HASH: &str =
	"0xa11fc57ceabd72676b4f1f6dec860de0c8f52f9d2e9496ea36366d1ba47cd391";
pub const ORBIS_COMPACT_WASM_SHA256: &str =
	"5c8fc14ff06c0a85dcc3ccd95027c3b899df6fa7a691b83fb696fc278fa7ce04";
pub const ORBIS_CANDIDATE_GENESIS_HEADER_HASH: &str =
	"0x066f97db4ab6a5e5d44ee66c3b469f82d178817650fa6f52634ea2ebead6c6e3";
pub const ORBIS_CANDIDATE_GENESIS_STATE_ROOT: &str =
	"0x55e81b8c3aada6227214d467cefad014198e84d4e3fa15c8021e31dcc40fe0dc";
pub const ORBIS_CANDIDATE_GENESIS_IDENTITY_SHA256: &str =
	"8b4986d5d6c7b4e92a29b53424bedb75303dbd80c77137f996d40ae334a4b59f";
pub const ORBIS_ACTIVATION_STATE: &str = "candidate-pending";
pub const ORBIS_PRODUCTION_ACTIVATION_READY: bool = false;

pub const IDENTITY_PERSONHOOD_RUNTIME_API_VERSION: u32 = 1;
pub const ATTESTATION_RUNTIME_API_VERSION: u32 = 1;
pub const DOTNS_RUNTIME_API_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_RUNTIME_API_VERSION: u32 = 4;
pub const DRIVE_RUNTIME_API_VERSION: u32 = 1;
pub const S3_RUNTIME_API_VERSION: u32 = 1;

pub const ATTESTATION_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const DOTNS_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_STORAGE_SCHEMA_VERSION: u32 = 5;
pub const DRIVE_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const S3_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const TRANSACTION_STORAGE_SCHEMA_VERSION: u32 = 8;
pub const RESOURCES_STORAGE_SCHEMA_VERSION: u32 = 1;

pub const DOTNS_LABEL_POLICY_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_PROTOCOL_VERSION: u32 = 4;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checked_in_version_matrix_matches_rust_contract() {
		let matrix: serde_json::Value =
			serde_json::from_str(include_str!("../../../docs/sdk/native-version-matrix.json"))
				.expect("native version matrix is valid JSON");
		assert_eq!(matrix["contract_version"], NATIVE_SDK_CONTRACT_VERSION);
		assert_eq!(matrix["release"], NATIVE_SDK_RELEASE);
		assert_eq!(matrix["sdk_release"], NATIVE_SDK_PACKAGE_VERSION);
		assert_eq!(matrix["networks"]["origin"]["spec_version"], ORIGIN_SPEC_VERSION);
		assert_eq!(matrix["networks"]["origin"]["transaction_version"], ORIGIN_TRANSACTION_VERSION);
		assert_eq!(matrix["networks"]["origin"]["activation_state"], ORIGIN_ACTIVATION_STATE);
		assert_eq!(
			matrix["networks"]["origin"]["production_activation_ready"],
			ORIGIN_PRODUCTION_ACTIVATION_READY
		);
		assert_eq!(matrix["networks"]["orbis"]["para_id"], ORBIS_PARA_ID);
		assert_eq!(matrix["networks"]["orbis"]["spec_version"], ORBIS_SPEC_VERSION);
		assert_eq!(matrix["networks"]["orbis"]["transaction_version"], ORBIS_TRANSACTION_VERSION);
		assert_eq!(matrix["networks"]["orbis"]["metadata_hash"], ORBIS_METADATA_HASH);
		assert_eq!(matrix["networks"]["orbis"]["activation_state"], ORBIS_ACTIVATION_STATE);
		assert_eq!(
			matrix["networks"]["orbis"]["production_activation_ready"],
			ORBIS_PRODUCTION_ACTIVATION_READY
		);
		assert_eq!(
			matrix["native_runtime_apis"]["identity_personhood"]["version"],
			IDENTITY_PERSONHOOD_RUNTIME_API_VERSION
		);
		assert_eq!(
			matrix["native_runtime_apis"]["storage_provider"]["version"],
			STORAGE_PROVIDER_RUNTIME_API_VERSION
		);
		assert_eq!(
			matrix["native_storage_schemas"]["storage_provider"]["version"],
			STORAGE_PROVIDER_STORAGE_SCHEMA_VERSION
		);
		assert_eq!(
			matrix["service_protocols"]["storage_provider"]["version"],
			STORAGE_PROVIDER_PROTOCOL_VERSION
		);
	}
}
