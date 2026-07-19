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

use crate as pallet_orbis_s3;
use frame_support::{derive_impl, parameter_types};
use pallet_orbis_storage_control_primitives::{
	CanonicalStorageControl, Commitment, CommitmentState,
};
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, BuildStorage};
use std::{
	cell::RefCell,
	collections::{BTreeMap, BTreeSet},
};

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		S3: pallet_orbis_s3,
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
	pub const MaxBucketNameLen: u32 = 63;
	pub const MaxObjectKeyLen: u32 = 1024;
	pub const MaxControllers: u32 = 2;
	pub const MaxBucketsPerOwner: u32 = 3;
	pub const MaxObjectsPerBucket: u32 = 4;
	pub const MaxObjectVersions: u32 = 64;
}

thread_local! {
	static CANONICAL_CONTENT: RefCell<BTreeMap<Commitment, (CommitmentState, Commitment)>> = RefCell::new(BTreeMap::new());
	static DRIVE_REFERENCES: RefCell<BTreeSet<Commitment>> = RefCell::new(BTreeSet::new());
	static DELETION_REQUIREMENTS: RefCell<BTreeMap<Commitment, BTreeSet<u64>>> = RefCell::new(BTreeMap::new());
	static DELETION_ACKNOWLEDGEMENTS: RefCell<BTreeMap<Commitment, BTreeSet<u64>>> = RefCell::new(BTreeMap::new());
}

pub struct CanonicalContent;
impl CanonicalStorageControl for CanonicalContent {
	fn manifest_state(content_hash: &Commitment) -> CommitmentState {
		CANONICAL_CONTENT.with(|items| {
			items
				.borrow()
				.get(content_hash)
				.map(|item| item.0)
				.unwrap_or(CommitmentState::Missing)
		})
	}
	fn provider_commitment_matches(content_hash: &Commitment, provider: &Commitment) -> bool {
		CANONICAL_CONTENT
			.with(|items| items.borrow().get(content_hash).is_some_and(|item| &item.1 == provider))
	}
	fn is_drive_referenced(manifest: &Commitment) -> bool {
		DRIVE_REFERENCES.with(|items| items.borrow().contains(manifest))
	}
	fn deletion_evidence_satisfied(manifest: &Commitment) -> bool {
		DELETION_REQUIREMENTS.with(|requirements| {
			DELETION_ACKNOWLEDGEMENTS.with(|acknowledgements| {
				let requirements = requirements.borrow();
				let acknowledgements = acknowledgements.borrow();
				let Some(required) = requirements.get(manifest) else { return false };
				!required.is_empty()
					&& acknowledgements.get(manifest).is_some_and(|acked| {
						required.iter().all(|provider| acked.contains(provider))
					})
			})
		})
	}
}

impl pallet_orbis_s3::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type StorageControl = CanonicalContent;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = CanonicalContent;
	type MaxBucketNameLen = MaxBucketNameLen;
	type MaxObjectKeyLen = MaxObjectKeyLen;
	type MaxControllers = MaxControllers;
	type MaxBucketsPerOwner = MaxBucketsPerOwner;
	type MaxObjectsPerBucket = MaxObjectsPerBucket;
	type MaxObjectVersions = MaxObjectVersions;
	type WeightInfo = ();
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper for CanonicalContent {
	fn make_publishable(manifest: Commitment, provider: Commitment) {
		set_state(manifest, CommitmentState::Publishable, provider);
	}

	fn make_deletion_satisfied(manifest: Commitment) {
		set_deletion_evidence(manifest);
	}
}

pub fn add_canonical(content_hash: [u8; 32]) {
	CANONICAL_CONTENT.with(|items| {
		items
			.borrow_mut()
			.insert(content_hash, (CommitmentState::Publishable, content_hash));
	});
}

pub fn set_state(content_hash: Commitment, state: CommitmentState, provider: Commitment) {
	CANONICAL_CONTENT.with(|items| {
		items.borrow_mut().insert(content_hash, (state, provider));
	});
}

pub fn set_drive_referenced(manifest: Commitment, referenced: bool) {
	DRIVE_REFERENCES.with(|items| {
		if referenced {
			items.borrow_mut().insert(manifest);
		} else {
			items.borrow_mut().remove(&manifest);
		}
	});
}

pub fn set_deletion_evidence(manifest: Commitment) {
	set_deletion_requirements(manifest, &[1, 2, 3]);
	set_deletion_acknowledgements(manifest, &[1, 2, 3]);
}

pub fn set_deletion_requirements(manifest: Commitment, providers: &[u64]) {
	DELETION_REQUIREMENTS.with(|requirements| {
		requirements.borrow_mut().insert(manifest, providers.iter().copied().collect());
	});
}

pub fn set_deletion_acknowledgements(manifest: Commitment, providers: &[u64]) {
	DELETION_ACKNOWLEDGEMENTS.with(|acknowledgements| {
		acknowledgements
			.borrow_mut()
			.insert(manifest, providers.iter().copied().collect());
	});
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	CANONICAL_CONTENT.with(|items| items.borrow_mut().clear());
	DRIVE_REFERENCES.with(|items| items.borrow_mut().clear());
	DELETION_REQUIREMENTS.with(|items| items.borrow_mut().clear());
	DELETION_ACKNOWLEDGEMENTS.with(|items| items.borrow_mut().clear());
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
