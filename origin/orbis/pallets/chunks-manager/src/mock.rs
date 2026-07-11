// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{derive_impl, traits::ConstU32};
use frame_system::EnsureRoot;
use scale_info::TypeInfo;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		ChunksManager: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

/// Test `Chunk` type. Wraps a `u32` for tests only.
#[derive(
	Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct MockChunk(pub u32);

impl verifiable::DecodeUnchecked for MockChunk {}

impl From<u32> for MockChunk {
	fn from(v: u32) -> Self {
		Self(v)
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<MockChunk> for BenchHelper {
	fn chunk_page() -> Vec<MockChunk> {
		(0..256).map(MockChunk).collect()
	}
}

impl crate::Config for Test {
	type WeightInfo = ();
	type Chunk = MockChunk;
	type PageSize = ConstU32<256>;
	type ManagerOrigin = EnsureRoot<Self::AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchHelper;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = RuntimeGenesisConfig::default().build_storage().unwrap();
	sp_io::TestExternalities::from(storage)
}
