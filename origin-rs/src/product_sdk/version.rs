// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

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
	"0x50c8958f0171889a4b01093a5dd5272faf8802018f24a4ac22b231d37b2adc45";
pub const ORBIS_COMPACT_WASM_SHA256: &str =
	"19662465cef9ea3cde0c7c10865cc9e68cf3afb723e6184de8a0029de82b1f2e";
pub const ORBIS_CANDIDATE_GENESIS_HEADER_HASH: &str =
	"0x2584c9d420dc8160b85deaf958d776886366d5beecc2ee7b1236293d20ac70fc";
pub const ORBIS_CANDIDATE_GENESIS_STATE_ROOT: &str =
	"0xe778ef91e9e1419f77a17a687245fc903335ecdb8dd3cf450df86637d529a53c";
pub const ORBIS_CANDIDATE_GENESIS_IDENTITY_SHA256: &str =
	"e0cfdc509e90b58c7c36a9cc522eab9013f281c076c100ec3f09fc9e24c074de";
pub const ORBIS_ACTIVATION_STATE: &str = "candidate-pending";
pub const ORBIS_PRODUCTION_ACTIVATION_READY: bool = false;

pub const ATTESTATION_RUNTIME_API_VERSION: u32 = 1;
pub const NAMES_RUNTIME_API_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_RUNTIME_API_VERSION: u32 = 11;
pub const DRIVE_RUNTIME_API_VERSION: u32 = 2;
pub const S3_RUNTIME_API_VERSION: u32 = 3;

pub const ATTESTATION_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const NAMES_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_STORAGE_SCHEMA_VERSION: u32 = 2;
pub const DRIVE_STORAGE_SCHEMA_VERSION: u32 = 1;
pub const S3_STORAGE_SCHEMA_VERSION: u32 = 2;
pub const RESOURCES_STORAGE_SCHEMA_VERSION: u32 = 1;

pub const NAMES_LABEL_POLICY_VERSION: u32 = 1;
pub const STORAGE_PROVIDER_PROTOCOL_VERSION: u32 = 6;

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
			matrix["native_runtime_apis"]["storage_provider"]["version"],
			STORAGE_PROVIDER_RUNTIME_API_VERSION
		);
		assert_eq!(matrix["native_runtime_apis"]["drive"]["version"], DRIVE_RUNTIME_API_VERSION);
		assert_eq!(matrix["native_runtime_apis"]["s3"]["version"], S3_RUNTIME_API_VERSION);
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
