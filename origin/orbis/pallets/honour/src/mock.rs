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

use crate::{
	bench_helpers::{get_mock_ring_members, get_ring_config},
	Seconds,
};
use alloc::vec::Vec;
use core::{cell::RefCell, ops::Range};
use frame_support::{derive_impl, parameter_types};
use indiv_support::traits::{
	BatchProofItem, Context, ContextualAlias, Identifier, MembershipMultiProver, MembershipProver,
	RevisedContextualAlias, RevisionIndex, RingExponent, RingIndex, PEOPLE_IDENTIFIER,
};
use sp_core::{ConstU16, ConstU64, H256};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError,
};
use verifiable::{
	ring::bandersnatch::{BandersnatchSha512Ell2, BandersnatchVrfVerifiable},
	GenerateVerifiable,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub fn new_test_ext() -> sp_io::TestExternalities {
	use sp_runtime::BuildStorage;
	set_time(0);

	RuntimeGenesisConfig { system: Default::default() }
		.build_storage()
		.unwrap()
		.into()
}

parameter_types! {
	pub const RingExponentValue: RingExponent = RingExponent::R2e14;
	pub const PointFreezeDuration: Seconds = 7 * 24 * 3600; // 7 days
	pub const CallMortality: Seconds = 10 * 60; // 10 minutes
}

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Honour: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl crate::Config for Test {
	type WeightInfo = ();
	type MemberService = MockMemberService;
	type Clock = MockUnixTime;
	type PointFreezeDuration = PointFreezeDuration;
	type CallMortality = CallMortality;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelper;
}

/// Benchmark helper that proves against the in-process mock ring verified by
/// [`MockMemberService`], so the pallet's `extension_validate` benchmark needs no real runtime.
#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for BenchmarkHelper {
	fn set_time(now: Seconds) {
		set_time(now);
	}

	fn seed_and_create_proof(vote: &crate::VoteData, message: &[u8]) -> crate::RingProofOf<Test> {
		crate::bench_helpers::prove_vote::<Test>(vote, 0, message)
	}
}

/// Mock [`MembershipMultiProver`] for the honour pallet tests.
///
/// Wraps Bandersnatch ring-VRF verification against a fixed mock ring (the
/// 255 keyed members produced by [`get_mock_ring_members`]). Always reports
/// revision `0` and ignores `identifier`, since honour only ever queries the
/// hardcoded `PEOPLE_RING_COLLECTION`.
pub struct MockMemberService;

impl MembershipProver for MockMemberService {
	type Crypto = BandersnatchVrfVerifiable;

	fn verify_membership(
		_identifier: &Identifier,
		_proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		_ring_index: RingIndex,
		_context: Context,
		_msg: &[u8],
	) -> Result<RevisedContextualAlias, DispatchError> {
		unimplemented!("not used by honour tests")
	}

	fn verify_membership_at_rev(
		_identifier: &Identifier,
		_proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		_ring_index: RingIndex,
		_revision: RevisionIndex,
		_context: Context,
		_msg: &[u8],
	) -> Result<ContextualAlias, DispatchError> {
		unimplemented!("not used by honour tests")
	}

	fn verify_memberships_in_ring(
		_identifier: &Identifier,
		_ring_index: RingIndex,
		_items: &[BatchProofItem<<Self::Crypto as GenerateVerifiable>::Proof>],
	) -> Result<Vec<RevisedContextualAlias>, DispatchError> {
		unimplemented!("not used by honour tests")
	}

	fn verify_memberships_in_ring_at_rev(
		_identifier: &Identifier,
		_ring_index: RingIndex,
		_revision: RevisionIndex,
		_items: &[BatchProofItem<<Self::Crypto as GenerateVerifiable>::Proof>],
	) -> Result<Vec<ContextualAlias>, DispatchError> {
		unimplemented!("not used by honour tests")
	}

	fn ring_revision(_identifier: &Identifier, _ring_index: RingIndex) -> Option<RevisionIndex> {
		Some(0)
	}

	fn is_revision_valid(
		_identifier: &Identifier,
		_ring_index: RingIndex,
		revision: RevisionIndex,
	) -> bool {
		revision == 0
	}

	fn revision_source_time(
		_identifier: &Identifier,
		_ring_index: RingIndex,
		_revision: RevisionIndex,
	) -> Option<u64> {
		None
	}
}

impl MembershipMultiProver for MockMemberService {
	fn verify_membership_multi_context(
		_identifier: &Identifier,
		_proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		_ring_index: RingIndex,
		_contexts: &[Context],
		_msg: &[u8],
	) -> Result<Vec<RevisedContextualAlias>, DispatchError> {
		unimplemented!("not used by honour tests")
	}

	fn verify_membership_multi_context_at_rev(
		identifier: &Identifier,
		proof: &<Self::Crypto as GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		revision: RevisionIndex,
		contexts: &[Context],
		msg: &[u8],
	) -> Result<Vec<ContextualAlias>, DispatchError> {
		assert_eq!(
			identifier, PEOPLE_IDENTIFIER,
			"honour pallet must only query the people ring collection",
		);

		// The mock has a single ring (index `0`) built once at revision `0`, mirroring
		// `ring_revision`/`is_revision_valid`. Reject any other ring or revision the way a real
		// implementation would reject an unknown ring or non-retained revision.
		if ring_index != 0 {
			return Err(DispatchError::Other("mock unknown ring"));
		}
		if revision != 0 {
			return Err(DispatchError::Other("mock unknown revision"));
		}

		let config = get_ring_config();
		let root = build_ring_members();
		let context_slices: Vec<&[u8]> = contexts.iter().map(|c| &c[..]).collect();

		let aliases = BandersnatchVrfVerifiable::validate_multi_context(
			config,
			proof,
			&root,
			&context_slices,
			msg,
		)
		.map_err(|_| DispatchError::Other("mock invalid proof"))?;

		Ok(aliases
			.into_iter()
			.zip(contexts.iter().copied())
			.map(|(alias, context)| ContextualAlias { alias, context })
			.collect())
	}
}

fn build_ring_members() -> <BandersnatchVrfVerifiable as GenerateVerifiable>::Members {
	let config = get_ring_config();
	let mut intermediate = BandersnatchVrfVerifiable::start_members(config);
	BandersnatchVrfVerifiable::push_members(
		&mut intermediate,
		get_mock_ring_members::<BandersnatchVrfVerifiable>(),
		get_chunks_for_range,
	)
	.expect("Failed to push ring members");
	BandersnatchVrfVerifiable::finish_members(intermediate)
}

thread_local! {
	pub static MOCK_NOW: RefCell<Seconds> = const { RefCell::new(0) };
}

pub struct MockUnixTime;

impl frame_support::traits::UnixTime for MockUnixTime {
	fn now() -> core::time::Duration {
		core::time::Duration::from_secs(MOCK_NOW.with(|v| *v.borrow()))
	}
}

pub fn set_time(now: Seconds) {
	MOCK_NOW.with(|v| *v.borrow_mut() = now);
}

fn get_chunks_for_range(
	range: Range<usize>,
) -> Result<Vec<<BandersnatchVrfVerifiable as GenerateVerifiable>::StaticChunk>, ()> {
	use verifiable::ring::{ark_vrf::ring::SrsLookup, StaticChunk};

	let domain_size = RingExponentValue::get()
		.try_into()
		.expect("exponent should be compatible with capacity");
	let params =
		verifiable::ring::ring_verifier_builder_params::<BandersnatchSha512Ell2>(domain_size);
	(&params)
		.lookup(range)
		.map(|v| v.into_iter().map(StaticChunk::<BandersnatchSha512Ell2>).collect())
		.ok_or(())
}
