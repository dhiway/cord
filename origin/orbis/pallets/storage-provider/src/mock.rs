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

use crate::{
	self as pallet_orbis_storage_provider, OrganizationRefOf, ProviderAuthority,
	ProviderAuthorityError,
};
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use sp_core::{ed25519, H256};
use sp_runtime::{traits::BlakeTwo256, BuildStorage};
use std::cell::RefCell;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		StorageProvider: pallet_orbis_storage_provider,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = u64;
	type Block = frame_system::mocking::MockBlock<Self>;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
}

thread_local! {
	static AUTHORITY_FAILURE: RefCell<Option<ProviderAuthorityError>> = const { RefCell::new(None) };
	static TARGETED_AUTHORITY_FAILURE: RefCell<Option<(u64, ProviderAuthorityError)>> = const { RefCell::new(None) };
}

pub fn set_finalized(block: u64) {
	crate::GovernedFinalizedCheckpoint::<Test>::put(block);
}
pub fn set_authority_failure(error: Option<ProviderAuthorityError>) {
	AUTHORITY_FAILURE.with(|value| *value.borrow_mut() = error);
}

pub fn set_provider_authority_failure(provider: u64, error: Option<ProviderAuthorityError>) {
	TARGETED_AUTHORITY_FAILURE.with(|value| {
		*value.borrow_mut() = error.map(|error| (provider, error));
	});
}

pub struct Authority;
pub struct Context;
impl crate::CheckpointContextProvider<H256, u64> for Context {
	fn genesis_hash() -> H256 {
		H256::repeat_byte(0x11)
	}
	fn spec_version() -> u32 {
		1
	}
	fn transaction_version() -> u32 {
		1
	}
	fn metadata_hash() -> H256 {
		H256::repeat_byte(0x22)
	}
	fn block_hash(block: u64) -> H256 {
		H256::from_low_u64_be(block)
	}
}
impl ProviderAuthority<u64, OrganizationRefOf<Test>, ed25519::Public, u64> for Authority {
	fn validate(
		provider: &u64,
		organization: &OrganizationRefOf<Test>,
		service_key: &ed25519::Public,
		finalized_at: u64,
	) -> Result<(), ProviderAuthorityError> {
		if let Some(error) = AUTHORITY_FAILURE.with(|value| *value.borrow()) {
			return Err(error);
		}
		if let Some((target, error)) = TARGETED_AUTHORITY_FAILURE.with(|value| *value.borrow()) {
			if target == *provider {
				return Err(error);
			}
		}
		if organization.entity_id.as_slice() == b"unknown" {
			return Err(ProviderAuthorityError::OrganizationUnknown);
		}
		if organization.attestation_id == H256::zero() {
			return Err(ProviderAuthorityError::AttestationInvalid);
		}
		if finalized_at < organization.valid_from || finalized_at > organization.valid_until {
			return Err(ProviderAuthorityError::AttestationExpired);
		}
		if organization.schema_id == H256::zero()
			|| organization.sla_commitment == H256::zero()
			|| organization.sla_version == 0
		{
			return Err(ProviderAuthorityError::SlaInvalid);
		}
		let _ = service_key;
		Ok(())
	}
}

parameter_types! {
	pub const MaxEndpointBytes: u32 = 64;
	pub const MaxEntityIdBytes: u32 = 64;
	pub const MaxAgreements: u32 = 4;
	pub const MaxDutiesPerBlock: u32 = 8;
	pub const MaxChallengeBacklog: u32 = 8;
	pub const MaxCapacityReleasesPerBlock: u32 = 8;
	pub const MaxProofNodes: u32 = 16;
	pub const MaxEvidence: u32 = 8;
	pub const CheckpointCadence: u64 = 100;
	pub const CheckpointGrace: u64 = 20;
	pub const MaxCheckpointAge: u64 = 128;
	pub const EvidenceWindow: u64 = 10;
	pub const MaxHostDelegationLifetime: u64 = 128;
}

impl pallet_orbis_storage_provider::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AdminOrigin = frame_system::EnsureRoot<u64>;
	type MaxEndpointBytes = MaxEndpointBytes;
	type MaxEntityIdBytes = MaxEntityIdBytes;
	type MaxProviders = ConstU32<16>;
	type MaxBuckets = ConstU32<16>;
	type MaxBucketGrants = ConstU32<4>;
	type MaxHostDelegationsPerBucket = ConstU32<2>;
	type MaxCapabilityProductIdBytes = ConstU32<128>;
	type MaxCapabilityMethods = ConstU32<64>;
	type MaxCapabilityCidBytes = ConstU32<128>;
	type MaxHostDelegationLifetime = MaxHostDelegationLifetime;
	type MaxReplicas = ConstU32<4>;
	type MaxAssignedProviders = ConstU32<5>;
	type MaxProviderAgreements = ConstU32<16>;
	type MaxBucketAgreements = MaxAgreements;
	type MaxDutiesPerBlock = MaxDutiesPerBlock;
	type MaxChallengeBacklog = MaxChallengeBacklog;
	type MaxCapacityReleasesPerBlock = MaxCapacityReleasesPerBlock;
	type MaxChallengesPerBlock = ConstU32<4>;
	type MaxReconciliationRecords = ConstU32<4>;
	type MaxProofNodes = MaxProofNodes;
	type MaxEvidencePerProvider = MaxEvidence;
	type MaxOrganizationHistory = ConstU32<4>;
	type CheckpointCadence = CheckpointCadence;
	type CheckpointGrace = CheckpointGrace;
	type MaxCheckpointAge = MaxCheckpointAge;
	type EvidenceWindow = EvidenceWindow;
	type ProviderAuthority = Authority;
	type CheckpointContext = Context;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = Authority;
	type WeightInfo = crate::weights::SubstrateWeight<Test>;
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for Authority {
	fn organization(
		_: &u64,
		_: &ed25519::Public,
		rotation_predecessor: Option<H256>,
	) -> OrganizationRefOf<Test> {
		crate::ProviderOrganizationRefV1 {
			entity_id: b"benchmark".to_vec().try_into().unwrap(),
			attestation_id: H256::repeat_byte(1),
			schema_id: H256::repeat_byte(2),
			sla_commitment: H256::repeat_byte(3),
			sla_version: 1,
			valid_from: 0,
			valid_until: u64::MAX,
			rotation_predecessor,
		}
	}

	fn set_finalized_block(block: u64) {
		set_finalized(block);
	}

	fn invalidate_authority(provider: &u64, _: &OrganizationRefOf<Test>) {
		set_provider_authority_failure(*provider, Some(ProviderAuthorityError::AttestationInvalid));
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		System::set_block_number(1);
		set_finalized(1);
		set_authority_failure(None);
		set_provider_authority_failure(0, None);
	});
	ext
}
