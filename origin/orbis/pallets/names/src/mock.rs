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

use crate as pallet_orbis_names;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub const KNOWN_SUBJECT: H256 = H256::repeat_byte(41);
pub const LIVE_ATTESTATION: H256 = H256::repeat_byte(42);

pub struct TestSubjectReferenceValidator;

impl pallet_orbis_names::SubjectReferenceValidator<H256> for TestSubjectReferenceValidator {
	fn contains(subject: &H256) -> bool {
		*subject == KNOWN_SUBJECT
	}
}

pub struct TestAttestationReferenceValidator;

impl pallet_orbis_names::AttestationReferenceValidator<H256> for TestAttestationReferenceValidator {
	fn is_live(attestation: &H256) -> bool {
		*attestation == LIVE_ATTESTATION
	}
}

type Block = frame_system::mocking::MockBlock<Test>;

#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(RuntimeCall, RuntimeEvent, RuntimeError, RuntimeOrigin, RuntimeTask)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;

	#[runtime::pallet_index(116)]
	pub type Names = pallet_orbis_names;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Hash = H256;
}

parameter_types! {
	pub const MinCommitmentAge: u64 = 2;
	pub const MaxCommitmentAge: u64 = 10;
	pub const RegistrationPeriod: u64 = 100;
	pub const MaxRenewalPeriod: u64 = 100;
	pub const MaxContentOperationReceiptLifetime: u64 = 20;
}

impl pallet_orbis_names::Config for Test {
	type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type SubjectId = H256;
	type SubjectReferenceValidator = TestSubjectReferenceValidator;
	type AttestationId = H256;
	type AttestationReferenceValidator = TestAttestationReferenceValidator;
	type ContentCommitment = H256;
	type ContentReferenceValidator = ();
	type MaxLabelLength = ConstU32<63>;
	type MaxSaltLength = ConstU32<64>;
	type MaxAddressLength = ConstU32<128>;
	type MaxTextKeyLength = ConstU32<32>;
	type MaxTextValueLength = ConstU32<256>;
	type MaxTextRecords = ConstU32<8>;
	type MaxControllers = ConstU32<4>;
	type MaxRegistrars = ConstU32<2>;
	type MaxBootstrapReservations = ConstU32<4>;
	type MaxNamesPerOwner = ConstU32<16>;
	type MaxChildrenPerName = ConstU32<16>;
	type MaxRootNames = ConstU32<32>;
	type MaxNameDepth = ConstU32<4>;
	type MaxCommitmentsPerAccount = ConstU32<8>;
	type MaxContentOperationReceipts = ConstU32<2>;
	type MaxContentOperationReceiptLifetime = MaxContentOperationReceiptLifetime;
	type MinCommitmentAge = MinCommitmentAge;
	type MaxCommitmentAge = MaxCommitmentAge;
	type RegistrationPeriod = RegistrationPeriod;
	type MaxRenewalPeriod = MaxRenewalPeriod;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	new_test_ext_with_names(Vec::new(), Vec::new())
}

pub fn new_test_ext_with_names(
	registrars: Vec<u64>,
	root_reservations: Vec<(pallet_orbis_names::LabelOf<Test>, Option<u64>)>,
) -> sp_io::TestExternalities {
	let storage = RuntimeGenesisConfig {
		system: Default::default(),
		names: pallet_orbis_names::GenesisConfig { registrars, root_reservations },
	}
		.build_storage()
		.expect("test genesis builds");
	let mut ext: sp_io::TestExternalities = storage.into();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn run_to(block: u64) {
	System::set_block_number(block);
}
