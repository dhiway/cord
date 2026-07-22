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

use crate as pallet_orbis_drive;
use frame_support::{derive_impl, parameter_types};
use pallet_orbis_storage_control_primitives::{
	CanonicalStorageControl, Commitment, CommitmentState,
};
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, BuildStorage};
use std::{cell::RefCell, collections::BTreeMap};

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Drive: pallet_orbis_drive,
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

parameter_types! {
	pub const MaxDriveNameBytes: u32 = 256;
	pub const MaxDrivesPerOwner: u32 = 3;
	pub const MaxControllersPerDrive: u32 = 2;
}

thread_local! {
	static STATES: RefCell<BTreeMap<Commitment, CommitmentState>> = RefCell::new(BTreeMap::new());
	static PROVIDERS: RefCell<BTreeMap<Commitment, Commitment>> = RefCell::new(BTreeMap::new());
}

pub struct StorageControl;
impl CanonicalStorageControl for StorageControl {
	fn manifest_state(manifest: &Commitment) -> CommitmentState {
		STATES.with(|states| {
			states.borrow().get(manifest).copied().unwrap_or(CommitmentState::Missing)
		})
	}

	fn provider_commitment_matches(manifest: &Commitment, provider: &Commitment) -> bool {
		PROVIDERS.with(|providers| providers.borrow().get(manifest) == Some(provider))
	}
}

impl pallet_orbis_drive::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type StorageControl = StorageControl;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = StorageControl;
	type MaxDriveNameBytes = MaxDriveNameBytes;
	type MaxDrivesPerOwner = MaxDrivesPerOwner;
	type MaxControllersPerDrive = MaxControllersPerDrive;
	type WeightInfo = ();
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper for StorageControl {
	fn make_publishable(manifest: Commitment, provider: Commitment) {
		set_commitment(manifest, CommitmentState::Publishable, provider);
	}
}

pub fn set_commitment(manifest: Commitment, state: CommitmentState, provider: Commitment) {
	STATES.with(|states| states.borrow_mut().insert(manifest, state));
	PROVIDERS.with(|providers| providers.borrow_mut().insert(manifest, provider));
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	STATES.with(|states| states.borrow_mut().clear());
	PROVIDERS.with(|providers| providers.borrow_mut().clear());
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
