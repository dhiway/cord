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
use sp_core::H256;
use sp_runtime::{traits::BlakeTwo256, BuildStorage};
use std::{cell::RefCell, collections::BTreeSet};

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
	pub const MaxBucketNameLen: u32 = 32;
	pub const MaxObjectKeyLen: u32 = 64;
	pub const MaxControllers: u32 = 2;
	pub const MaxBucketsPerOwner: u32 = 3;
	pub const MaxObjectsPerBucket: u32 = 4;
	pub const MaxObjectVersions: u32 = 3;
}

thread_local! {
	static CANONICAL_CONTENT: RefCell<BTreeSet<[u8; 32]>> = RefCell::new(BTreeSet::new());
}

pub struct CanonicalContent;
impl crate::ContentHashValidator for CanonicalContent {
	fn exists(content_hash: &crate::ContentHash) -> bool {
		CANONICAL_CONTENT.with(|items| items.borrow().contains(content_hash))
	}
}

impl pallet_orbis_s3::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ContentValidator = CanonicalContent;
	type MaxBucketNameLen = MaxBucketNameLen;
	type MaxObjectKeyLen = MaxObjectKeyLen;
	type MaxControllers = MaxControllers;
	type MaxBucketsPerOwner = MaxBucketsPerOwner;
	type MaxObjectsPerBucket = MaxObjectsPerBucket;
	type MaxObjectVersions = MaxObjectVersions;
	type WeightInfo = ();
}

pub fn add_canonical(content_hash: [u8; 32]) {
	CANONICAL_CONTENT.with(|items| {
		items.borrow_mut().insert(content_hash);
	});
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	CANONICAL_CONTENT.with(|items| items.borrow_mut().clear());
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
