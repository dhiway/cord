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

//! Benchmarking for the members pallet.

use super::*;
use alloc::vec;
use core::{
	marker::{Send, Sync},
	time::Duration,
};
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
	pallet_prelude::{BoundedVec, Get, TransactionSource},
	traits::{Authorize, EnsureOrigin},
};
use frame_system::RawOrigin as SystemOrigin;
use indiv_support::traits::{
	AppendOnlyMembers, OnRingRootChange, RingExponent, RingIndex, RingMode, RingStatus,
};
use sp_runtime::traits::AppendZerosInput;
use verifiable::ring::{ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2, StaticChunk};

const RI_ZERO: RingIndex = 0;
const SEED: u32 = 0;

/// Test identifier for benchmarking.
const BENCH_IDENTIFIER: Identifier = [1u8; 32];

type SecretOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Secret;
type BandersnatchChunk = StaticChunk<BandersnatchSha512Ell2>;

/// Helper trait for benchmarking the members pallet.
pub trait BenchmarkHelper<Chunk> {
	/// Initialize chunks for crypto operations.
	fn initialize_chunks(ring_size: RingExponent) -> Vec<Chunk>;
	/// Set the current unix time to the given value.
	fn set_time(now: Duration);
	/// Bump the clock past genesis so `T::Clock::now()` doesn't return 0 and trip
	/// the timestamp pallet's "called at genesis" warning during bench setup.
	fn set_valid_time();
}

#[cfg(feature = "std")]
impl BenchmarkHelper<()> for () {
	fn initialize_chunks(_ring_size: RingExponent) -> Vec<()> {
		vec![]
	}
	fn set_time(_now: Duration) {
		// No-op for unit type implementation
	}
	fn set_valid_time() {
		// No-op for unit type implementation
	}
}

#[cfg(feature = "std")]
impl BenchmarkHelper<BandersnatchChunk> for () {
	fn initialize_chunks(ring_size: RingExponent) -> Vec<BandersnatchChunk> {
		use indiv_support::genesis::ring_verifier_builder_params;
		use verifiable::ring::RingDomainSize;
		let domain: RingDomainSize = ring_size.try_into().unwrap();
		ring_verifier_builder_params(domain)
	}
	fn set_time(_now: Duration) {
		// No-op for unit type implementation
	}
	fn set_valid_time() {
		// No-op for unit type implementation
	}
}

/// Generate a new member key pair from seed.
fn new_member_from<T: Config + Send + Sync>(i: u32, seed: u32) -> (SecretOf<T>, MemberOf<T>) {
	let mut entropy = &(i, seed).encode()[..];
	let mut entropy = AppendZerosInput::new(&mut entropy);
	let secret = T::Crypto::new_secret(Decode::decode(&mut entropy).unwrap());
	let public = T::Crypto::member_from_secret(&secret);
	(secret, public)
}

/// Generate a full ring's worth of members for benchmarks.
fn generate_members_for_ring<T: Config + Send + Sync>(
	seed: u32,
	max_ring_size: u32,
) -> Vec<(SecretOf<T>, MemberOf<T>)> {
	(0..max_ring_size).map(|i| new_member_from::<T>(i, seed)).collect::<Vec<_>>()
}

/// Generate members in a given range.
fn generate_members<T: Config + Send + Sync>(
	seed: u32,
	start: u32,
	end: u32,
) -> Vec<(SecretOf<T>, MemberOf<T>)> {
	(start..end).map(|i| new_member_from::<T>(i, seed)).collect::<Vec<_>>()
}

/// Create a test collection and add members to it.
fn setup_collection<T: Config + Send + Sync>(
	identifier: Identifier,
	onboarding_size: u32,
	ring_exponent: RingExponent,
	ring_mode: RingMode,
) -> T::Location {
	let owner: T::Location =
		Decode::decode(&mut &[0u8; 32][..]).expect("Location should be decodable from bytes");
	pallet::Pallet::<T>::create_collection(
		owner.clone(),
		&identifier,
		onboarding_size,
		ring_mode,
		ring_exponent,
		None,
	)
	.expect("Failed to create collection");

	// Initialize chunks for ring building operations.
	pallet::Pallet::<T>::initialize_chunks(ring_exponent);

	owner
}

/// Add members to an existing collection.
fn add_members_to_collection<T: Config + Send + Sync>(
	identifier: &Identifier,
	members: &[(SecretOf<T>, MemberOf<T>)],
) {
	let member_keys: Vec<MemberOf<T>> = members.iter().map(|(_, m)| m.clone()).collect();
	pallet::Pallet::<T>::add_members(identifier, member_keys).expect("Failed to add members");
}

/// Set up state for a `build_ring` benchmark with a specific ring exponent.
///
/// For multi-page rings (`ring_capacity > keys_per_page`), positions `included` at the page
/// boundary to capture the worst-case 2-page read. For single-page rings, fills to capacity
/// and leaves `n` members unbuilt.
fn setup_build_ring_bench<T: Config + Send + Sync>(
	ring_exponent: RingExponent,
	n: u32,
) -> Result<(Identifier, u32, u32), frame_benchmarking::BenchmarkError> {
	let identifier = BENCH_IDENTIFIER;
	let keys_per_page = T::MaxFlexibleRingExponent::get().ring_capacity();
	let ring_capacity = ring_exponent.ring_capacity();

	// Cap at ring capacity — a ring cannot hold more members than its exponent allows.
	let ring_size: u32 = (keys_per_page + n).min(ring_capacity);

	let pre_build = if ring_size > keys_per_page {
		// Multi-page: position `included` at the last slot of the first page so the
		// benchmarked call crosses the page boundary.
		keys_per_page - 1
	} else {
		// Single-page: leave exactly `n` members unbuilt.
		ring_size - n
	};

	setup_collection::<T>(identifier, 1, ring_exponent, RingMode::AppendOnly);

	let members = generate_members::<T>(SEED, 0, ring_size);
	add_members_to_collection::<T>(&identifier, &members);

	// Onboard all members — may need two calls if members span 2 queue pages.
	assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
	if RingKeysStatus::<T>::get(identifier, RI_ZERO).total < ring_size {
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
	}
	assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).total, ring_size);

	assert_ok!(Pallet::<T>::build_ring(&identifier, RI_ZERO, pre_build));
	assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).included, pre_build);

	Ok((identifier, pre_build, ring_size))
}

/// Worst-case setup for `ensure_can_delete_ring_page`: the deletion queue
/// already contains the targeted (identifier, ring_index, page_index) entry so
/// `RingDeletionQueue::contains_key` succeeds.
fn setup_ensure_can_delete_ring_page<T: Config + Send + Sync>() -> Call<T>
where
	<T as frame_system::Config>::RuntimeCall: From<Call<T>>,
{
	T::BenchmarkHelper::set_valid_time();

	let identifier = BENCH_IDENTIFIER;
	RingDeletionQueue::<T>::insert((identifier, RI_ZERO, 0u32), ());

	Call::<T>::delete_ring_page_authorized { identifier, ring_index: RI_ZERO, page_index: 0u32 }
}

/// Worst-case setup for `ensure_can_remove_suspended_keys`: the ring has
/// pending suspensions, the collection is not suspended, and `RingsState` is
/// back to append-only — so all three early-returns in
/// `should_remove_suspended_keys` miss and `PendingSuspensions::decode_len`
/// fires.
fn setup_ensure_can_remove_suspended_keys<T: Config + Send + Sync>() -> Call<T>
where
	<T as frame_system::Config>::RuntimeCall: From<Call<T>>,
{
	T::BenchmarkHelper::set_valid_time();

	let identifier = BENCH_IDENTIFIER;
	let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
	setup_collection::<T>(
		identifier,
		ring_size,
		T::MaxFlexibleRingExponent::get(),
		RingMode::Flexible,
	);

	let members = generate_members_for_ring::<T>(SEED, ring_size);
	add_members_to_collection::<T>(&identifier, &members);
	assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
	let to_include = pallet::Pallet::<T>::should_build_ring(&identifier, RI_ZERO, ring_size)
		.expect("should_build_ring");
	assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));

	// Mark all members suspended; end the session so RingsState is append-only
	// again while leaving entries in PendingSuspensions.
	assert_ok!(pallet::Pallet::<T>::start_removal_session(&identifier));
	let suspensions: Vec<MemberOf<T>> = members.iter().map(|(_, m)| m.clone()).collect();
	assert_ok!(pallet::Pallet::<T>::remove_members(&identifier, &suspensions));
	assert_ok!(pallet::Pallet::<T>::end_removal_session(&identifier));

	// The ring was built once, so its current revision is `Some(0)`.
	Call::<T>::remove_suspended_keys_authorized {
		identifier,
		ring_index: RI_ZERO,
		revision: Some(0),
		discriminator: 0,
	}
}

/// Worst-case setup for `ensure_can_merge_queue_pages`: the queue holds two
/// pages whose lengths sum to exactly `OnboardingQueuePageSize`, so
/// `should_merge_queue_pages` returns `Merge` after both `OnboardingQueue::get`
/// reads decode at full size.
fn setup_ensure_can_merge_queue_pages<T: Config + Send + Sync>() -> Call<T>
where
	<T as frame_system::Config>::RuntimeCall: From<Call<T>>,
{
	T::BenchmarkHelper::set_valid_time();

	let identifier = BENCH_IDENTIFIER;
	let queue_page_size: u32 = <T as Config>::OnboardingQueuePageSize::get();
	let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
	setup_collection::<T>(
		identifier,
		ring_size,
		T::MaxFlexibleRingExponent::get(),
		RingMode::Flexible,
	);

	// Two pages: page 0 full, page 1 holds one member, then drop one from page 0
	// so the sum equals `OnboardingQueuePageSize` (the largest mergeable layout).
	let members = generate_members::<T>(SEED, 0, queue_page_size + 1);
	add_members_to_collection::<T>(&identifier, &members);
	OnboardingQueue::<T>::mutate(identifier, 0, |keys| {
		keys.pop();
	});

	let (initial_head, new_head) = (0u32, 1u32);
	Call::<T>::merge_queue_pages_authorized { identifier, initial_head, new_head }
}

/// Worst-case setup for `ensure_can_onboard_members`: the queue spans two pages
/// (head=0, tail=1) with page 0 partial and page 1 full, so the dispatch reads
/// both `OnboardingQueue::decode_len` slots. `should_onboard_members` walks past
/// every early-return.
fn setup_ensure_can_onboard_members<T: Config + Send + Sync>() -> Call<T>
where
	<T as frame_system::Config>::RuntimeCall: From<Call<T>>,
{
	T::BenchmarkHelper::set_valid_time();

	let identifier = BENCH_IDENTIFIER;
	let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
	setup_collection::<T>(
		identifier,
		ring_size,
		T::MaxFlexibleRingExponent::get(),
		RingMode::Flexible,
	);

	// Onboard + build a full ring so CurrentRingIndex advances to 1.
	let members = generate_members::<T>(SEED, 0, ring_size);
	add_members_to_collection::<T>(&identifier, &members);
	assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
	let to_build = pallet::Pallet::<T>::should_build_ring(&identifier, RI_ZERO, ring_size)
		.expect("should_build_ring");
	assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_build));

	// Add a partial page of new members → fills queue page 0.
	let members2 = generate_members::<T>(SEED, ring_size, ring_size + ring_size / 2);
	add_members_to_collection::<T>(&identifier, &members2);

	// Force subsequent adds to flow into page 1 (head=0, tail=1).
	QueuePageIndices::<T>::insert(identifier, (0u32, 1u32));

	// Fill page 1.
	let queue_page_size: u32 = <T as Config>::OnboardingQueuePageSize::get();
	let members3 = generate_members::<T>(
		SEED,
		ring_size + ring_size / 2,
		ring_size + ring_size / 2 + queue_page_size,
	);
	add_members_to_collection::<T>(&identifier, &members3);

	let current_ring_index = CurrentRingIndex::<T>::get(identifier);
	let (head, tail) = QueuePageIndices::<T>::get(identifier);

	// Verify every worst-case condition holds so each branch in the authorize
	// hook actually fires. If any of these panics the bench setup is wrong.
	let collection_info = Collections::<T>::get(identifier).expect("collection must exist");
	let max_ring_size = collection_info.ring_size.ring_capacity();
	let ring_status = RingKeysStatus::<T>::get(identifier, current_ring_index);
	let open_slots = max_ring_size.saturating_sub(ring_status.total);
	let first_page_len = OnboardingQueue::<T>::decode_len(identifier, head).unwrap_or(0) as u32;

	assert!(head != tail, "head != tail is required for the second-page decode_len read to fire");
	assert!(
		first_page_len < open_slots,
		"first_page_len ({first_page_len}) < open_slots ({open_slots}) is required for the second-page read to fire"
	);
	assert!(
		!SuspendedCollections::<T>::contains_key(identifier),
		"collection must not be suspended for should_onboard_members to reach its full body"
	);
	assert!(
		RingsState::<T>::get(identifier).append_only(),
		"RingsState must be append_only for should_onboard_members to reach its full body"
	);
	assert!(
		!PendingSuspensions::<T>::contains_key(identifier, current_ring_index),
		"no pending suspensions allowed for should_onboard_members to reach its full body"
	);
	assert!(
		Pallet::<T>::can_onboard_members(&identifier, current_ring_index, head),
		"can_onboard_members must succeed so the authorize hook reaches the success path"
	);

	let first_member = OnboardingQueue::<T>::get(identifier, head).first().cloned();
	Call::<T>::onboard_members_authorized {
		identifier,
		ring_index: current_ring_index,
		head,
		first_member,
		discriminator: 0,
	}
}

fn setup_ensure_can_build_ring<T: Config + Send + Sync>(ring_exponent: RingExponent) -> Call<T>
where
	<T as frame_system::Config>::RuntimeCall: From<Call<T>>,
{
	T::BenchmarkHelper::set_valid_time();

	let identifier = BENCH_IDENTIFIER;
	setup_collection::<T>(identifier, 1, ring_exponent, RingMode::AppendOnly);

	// Onboard and build a first member so `Root` is populated at this exponent.
	let (_s0, m0) = new_member_from::<T>(0, SEED);
	pallet::Pallet::<T>::add_members(&identifier, vec![m0]).expect("add member");
	assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
	assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, 1));
	assert!(Root::<T>::get(identifier, RI_ZERO).is_some());

	// One more unbuilt member so `should_build_ring` returns Some(1).
	let (_s1, m1) = new_member_from::<T>(1, SEED);
	pallet::Pallet::<T>::add_members(&identifier, vec![m1]).expect("add member");
	assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));

	// The current revision after one successful build is `Some(0)`.
	Call::<T>::build_ring_authorized {
		identifier,
		ring_index: RI_ZERO,
		ring_exponent,
		revision: Some(0),
		to_include: 1,
		discriminator: 0,
	}
}

#[benchmarks(
	where <T as frame_system::Config>::RuntimeCall: From<Call<T>>,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn set_onboarding_size() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		// Setup collection
		let identifier = BENCH_IDENTIFIER;
		let onboarding_size = T::MaxFlexibleRingExponent::get().ring_capacity();
		setup_collection::<T>(
			identifier,
			onboarding_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		let new_onboarding_size = 1u32;

		let origin =
			T::ManagerOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, identifier, new_onboarding_size);

		assert_eq!(OnboardingSize::<T>::get(identifier), new_onboarding_size);

		Ok(())
	}

	#[benchmark]
	fn merge_rings() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection
		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Generate enough members for two full rings
		let members = generate_members::<T>(SEED, 0, ring_size * 2);
		add_members_to_collection::<T>(&identifier, &members);

		// Onboard and build first ring
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).total, ring_size);

		// Onboard and build second ring
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		assert_eq!(RingKeysStatus::<T>::get(identifier, 1).total, ring_size);

		assert_ok!(pallet::Pallet::<T>::build_ring(
			&identifier,
			RI_ZERO,
			T::MaxFlexibleRingExponent::get().ring_capacity()
		));
		assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).included, ring_size);

		assert_ok!(pallet::Pallet::<T>::build_ring(
			&identifier,
			1,
			T::MaxFlexibleRingExponent::get().ring_capacity()
		));
		assert_eq!(RingKeysStatus::<T>::get(identifier, 1).included, ring_size);

		// Suspend and remove more than half of the people in both rings
		assert_ok!(pallet::Pallet::<T>::start_removal_session(&identifier));
		let suspensions_ring_0: Vec<MemberOf<T>> =
			(1..ring_size / 2 + 3).map(|i| members[i as usize].1.clone()).collect();
		let suspensions_ring_1: Vec<MemberOf<T>> = (ring_size + 1..ring_size * 3 / 2 + 3)
			.map(|i| members[i as usize].1.clone())
			.collect();
		assert_ok!(pallet::Pallet::<T>::remove_members(&identifier, &suspensions_ring_0));
		assert_ok!(pallet::Pallet::<T>::remove_members(&identifier, &suspensions_ring_1));
		assert_ok!(pallet::Pallet::<T>::end_removal_session(&identifier));

		assert!(PendingSuspensions::<T>::get(identifier, RI_ZERO).len() > (ring_size / 2) as usize);
		assert!(PendingSuspensions::<T>::get(identifier, 1).len() > (ring_size / 2) as usize);

		pallet::Pallet::<T>::remove_suspended_keys(&identifier, RI_ZERO);
		pallet::Pallet::<T>::remove_suspended_keys(&identifier, 1);

		assert!(RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len() < (ring_size / 2) as usize);
		assert!(RingKeys::<T>::get((&identifier, 1u32, 0u32)).len() < (ring_size / 2) as usize);

		let keys_left_len = RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len() +
			RingKeys::<T>::get((&identifier, 1u32, 0u32)).len();

		// The current ring has to have a higher index than the ones being merged
		CurrentRingIndex::<T>::insert(identifier, 14);

		let caller: T::AccountId = account("caller", 0, SEED);

		// Drive the OnRingRootChange impl onto its worst-case branch
		<T::OnRingRootChange as OnRingRootChange<MembersOf<T>>>::bench_setup_worst_case();

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller), identifier, RI_ZERO, 1);

		assert_eq!(RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len(), keys_left_len);
		assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).total, keys_left_len as u32);
		assert!(Root::<T>::get(identifier, RI_ZERO).is_some());
		assert!(Root::<T>::get(identifier, 1).is_none());

		Ok(())
	}

	#[benchmark]
	fn should_build_ring(
		n: Linear<1, { T::RingBuildingMemberLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		// Generate a ring only up to the `RingBuildingMemberLimit` limit.
		let ring_size: u32 = T::RingBuildingMemberLimit::get();

		// Create collection with AppendOnly mode and R2e14 for larger rings
		setup_collection::<T>(
			identifier,
			ring_size,
			RingExponent::max_ring_exponent(),
			RingMode::AppendOnly,
		);

		// Generate members for the ring
		let members = generate_members::<T>(SEED, 0, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));

		// No ring built but people onboarded successfully
		assert!(Root::<T>::get(identifier, RI_ZERO).is_none());
		assert_eq!(RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len(), ring_size as usize);
		assert_eq!(
			RingKeysStatus::<T>::get(identifier, RI_ZERO),
			RingStatus { total: ring_size, included: 0, immutable_since: None }
		);

		#[block]
		{
			let _ = Pallet::<T>::should_build_ring(&identifier, RI_ZERO, n);
		}

		Ok(())
	}

	#[benchmark]
	fn build_ring_r2e9(
		n: Linear<1, { T::RingBuildingMemberLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let (identifier, pre_build, ring_size) =
			setup_build_ring_bench::<T>(RingExponent::R2e9, n)?;

		// Drive the OnRingRootChange impl onto its worst-case branch
		<T::OnRingRootChange as OnRingRootChange<MembersOf<T>>>::bench_setup_worst_case();

		#[block]
		{
			assert_ok!(Pallet::<T>::build_ring(&identifier, RI_ZERO, n));
		}

		let status = RingKeysStatus::<T>::get(identifier, RI_ZERO);
		assert_eq!(status.total, ring_size);
		assert_eq!(status.included, pre_build + n);

		Ok(())
	}

	#[benchmark]
	fn build_ring_r2e10(
		n: Linear<1, { T::RingBuildingMemberLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let (identifier, pre_build, ring_size) =
			setup_build_ring_bench::<T>(RingExponent::R2e10, n)?;

		// Drive the OnRingRootChange impl onto its worst-case branch
		<T::OnRingRootChange as OnRingRootChange<MembersOf<T>>>::bench_setup_worst_case();

		#[block]
		{
			assert_ok!(Pallet::<T>::build_ring(&identifier, RI_ZERO, n));
		}

		let status = RingKeysStatus::<T>::get(identifier, RI_ZERO);
		assert_eq!(status.total, ring_size);
		assert_eq!(status.included, pre_build + n);

		Ok(())
	}

	#[benchmark]
	fn build_ring_r2e14(
		n: Linear<1, { T::RingBuildingMemberLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let (identifier, pre_build, ring_size) =
			setup_build_ring_bench::<T>(RingExponent::R2e14, n)?;

		// Drive the OnRingRootChange impl onto its worst-case branch
		<T::OnRingRootChange as OnRingRootChange<MembersOf<T>>>::bench_setup_worst_case();

		#[block]
		{
			assert_ok!(Pallet::<T>::build_ring(&identifier, RI_ZERO, n));
		}

		let status = RingKeysStatus::<T>::get(identifier, RI_ZERO);
		assert_eq!(status.total, ring_size);
		assert_eq!(status.included, pre_build + n);

		Ok(())
	}

	#[benchmark]
	fn ensure_can_build_ring() -> Result<(), BenchmarkError> {
		let call = setup_ensure_can_build_ring::<T>(T::MaxFlexibleRingExponent::get());
		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}
		Ok(())
	}

	#[benchmark]
	fn ensure_can_delete_ring_page() -> Result<(), BenchmarkError> {
		let call = setup_ensure_can_delete_ring_page::<T>();
		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}
		Ok(())
	}

	#[benchmark]
	fn ensure_can_remove_suspended_keys() -> Result<(), BenchmarkError> {
		let call = setup_ensure_can_remove_suspended_keys::<T>();
		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}
		Ok(())
	}

	#[benchmark]
	fn ensure_can_merge_queue_pages() -> Result<(), BenchmarkError> {
		let call = setup_ensure_can_merge_queue_pages::<T>();
		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}
		Ok(())
	}

	#[benchmark]
	fn ensure_can_onboard_members() -> Result<(), BenchmarkError> {
		let call = setup_ensure_can_onboard_members::<T>();
		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}
		Ok(())
	}

	#[benchmark]
	fn onboard_members() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection
		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// One full ring exists
		let members = generate_members::<T>(SEED, 0, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		let to_include = pallet::Pallet::<T>::should_build_ring(
			&identifier,
			RI_ZERO,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
		)
		.unwrap();
		assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));
		assert_eq!(RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len(), ring_size as usize);
		assert_eq!(
			RingKeysStatus::<T>::get(identifier, RI_ZERO),
			RingStatus { total: ring_size, included: ring_size, immutable_since: None }
		);

		assert_eq!(QueuePageIndices::<T>::get(identifier), (0, 0));
		assert!(OnboardingQueue::<T>::get(identifier, 0).is_empty());

		// 1st onboarding page with fewer people than open slots
		let members2 = generate_members::<T>(SEED, ring_size, ring_size + ring_size / 2);
		add_members_to_collection::<T>(&identifier, &members2);
		assert_eq!(OnboardingQueue::<T>::get(identifier, 0).len(), (ring_size as u8 / 2) as usize);

		// To stop adding keys to the first page and start filling the next one
		QueuePageIndices::<T>::insert(identifier, (0, 1));
		assert!(OnboardingQueue::<T>::get(identifier, 1).is_empty());

		// 2nd onboarding page full
		let queue_page_size: u32 = <T as Config>::OnboardingQueuePageSize::get();
		let members3 = generate_members::<T>(
			SEED,
			ring_size + ring_size / 2,
			ring_size + ring_size / 2 + queue_page_size,
		);
		add_members_to_collection::<T>(&identifier, &members3);

		assert_eq!(QueuePageIndices::<T>::get(identifier), (0, 1));
		assert_eq!(OnboardingQueue::<T>::get(identifier, 0).len(), (ring_size / 2) as usize);
		assert!(OnboardingQueue::<T>::get(identifier, 1).is_full());

		assert_eq!(RingKeys::<T>::get((&identifier, 1u32, 0u32)).len(), 0);

		#[block]
		{
			assert_ok!(Pallet::<T>::onboard_members(&identifier, false));
		}

		assert_eq!(RingKeys::<T>::get((&identifier, 1u32, 0u32)).len(), ring_size as usize);
		assert_eq!(
			RingKeysStatus::<T>::get(identifier, 1),
			RingStatus { total: ring_size, included: 0, immutable_since: None }
		);

		Ok(())
	}

	#[benchmark]
	fn pending_suspensions_iteration() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let max_ring_size = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection
		setup_collection::<T>(
			identifier,
			max_ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Generate people and build a ring
		let members = generate_members_for_ring::<T>(SEED, max_ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		let to_include =
			pallet::Pallet::<T>::should_build_ring(&identifier, RI_ZERO, max_ring_size).unwrap();
		assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));

		// Suspend all people in the ring
		assert_ok!(pallet::Pallet::<T>::start_removal_session(&identifier));
		let suspensions: Vec<MemberOf<T>> = members.iter().map(|(_, m)| m.clone()).collect();
		assert_ok!(pallet::Pallet::<T>::remove_members(&identifier, &suspensions));
		assert_ok!(pallet::Pallet::<T>::end_removal_session(&identifier));

		// To make sure they are indeed pending suspension
		assert_eq!(PendingSuspensions::<T>::get(identifier, RI_ZERO).len(), max_ring_size as usize);

		#[block]
		{
			assert!(PendingSuspensions::<T>::iter_prefix(identifier).next().is_some());
		}

		Ok(())
	}

	#[benchmark]
	fn remove_suspended_keys(
		n: Linear<1, { T::MaxFlexibleRingExponent::get().ring_capacity() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection
		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Generate people and build a ring
		let members = generate_members_for_ring::<T>(SEED, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		let to_include =
			pallet::Pallet::<T>::should_build_ring(&identifier, RI_ZERO, ring_size).unwrap();
		assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));

		// For later verification
		let initial_root = Root::<T>::get(identifier, RI_ZERO).unwrap();

		// The value `n` represents the number of retained keys after suspensions, as this is what
		// scales the number of writes in the `Members` map; all keys are iterated during removal.
		// Suspend 'T::MaxFlexibleRingExponent::get().ring_capacity() - n' number of people in the
		// ring
		assert_ok!(pallet::Pallet::<T>::start_removal_session(&identifier));
		let members_retained = n;
		let suspension_count = ring_size - members_retained;
		let suspensions: Vec<MemberOf<T>> =
			members.iter().take(suspension_count as usize).map(|(_, m)| m.clone()).collect();
		assert_ok!(pallet::Pallet::<T>::remove_members(&identifier, &suspensions));
		assert_ok!(pallet::Pallet::<T>::end_removal_session(&identifier));

		// To make sure they are indeed pending suspension
		assert_eq!(
			PendingSuspensions::<T>::get(identifier, RI_ZERO).len(),
			suspension_count as usize
		);

		#[block]
		{
			pallet::Pallet::<T>::remove_suspended_keys(&identifier, RI_ZERO);
		}

		// Pending suspensions are cleared for the ring
		assert!(PendingSuspensions::<T>::get(identifier, RI_ZERO).is_empty());

		// Ring data becomes modified
		assert_eq!(
			RingKeysStatus::<T>::get(identifier, RI_ZERO),
			RingStatus { included: 0, total: members_retained, immutable_since: None }
		);
		assert_eq!(
			RingKeys::<T>::get((&identifier, RI_ZERO, 0u32)).len(),
			members_retained as usize
		);
		assert_ne!(
			Root::<T>::get(identifier, RI_ZERO).unwrap().intermediate,
			initial_root.intermediate
		);

		Ok(())
	}

	#[benchmark]
	fn merge_queue_pages() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let queue_page_size: u32 = <T as Config>::OnboardingQueuePageSize::get();
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection - use max ring size as onboarding size (must be <= MaxRingSize)
		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Two pages exists: first is full, the second contains one member
		let members = generate_members::<T>(SEED, 0, queue_page_size + 1);
		add_members_to_collection::<T>(&identifier, &members);

		assert_eq!(QueuePageIndices::<T>::get(identifier), (0, 1));
		assert!(OnboardingQueue::<T>::get(identifier, 0).is_full());
		assert_eq!(OnboardingQueue::<T>::get(identifier, 1).len(), 1);

		// One key is removed from the first page
		OnboardingQueue::<T>::mutate(identifier, 0, |keys| {
			keys.pop();
		});
		assert_eq!(OnboardingQueue::<T>::get(identifier, 0).len(), queue_page_size as usize - 1);

		// Attempt to merge pages succeeds
		let QueueMergeAction::Merge { initial_head, new_head, first_key_page, second_key_page } =
			pallet::Pallet::<T>::should_merge_queue_pages(&identifier)
		else {
			panic!("should be mergeable")
		};

		#[block]
		{
			pallet::Pallet::<T>::merge_queue_pages(
				&identifier,
				initial_head,
				new_head,
				first_key_page,
				second_key_page,
			)?;
		}

		// The queue pages have changed
		assert_eq!(QueuePageIndices::<T>::get(identifier), (1, 1));
		assert!(OnboardingQueue::<T>::get(identifier, 0).is_empty());
		assert!(OnboardingQueue::<T>::get(identifier, 1).is_full());

		Ok(())
	}

	#[benchmark]
	fn build_rings_base(
		n: Linear<1, { BUILD_MAX_ENTRIES as RingIndex }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let mut identifier = BENCH_IDENTIFIER;

		// Prepare to simulate reading of `BUILD_MAX_ENTRIES` stale rings
		for i in 0..n {
			identifier[..4].copy_from_slice(&i.to_le_bytes());
			StaleRings::<T>::insert(identifier, i as RingIndex, ());
		}

		#[block]
		{
			let _stale_rings_capped: Vec<_> =
				StaleRings::<T>::iter_keys().take(BUILD_MAX_ENTRIES).collect();
		}

		Ok(())
	}

	// ============================================================================
	// Collection Deletion Benchmarks
	// ============================================================================

	/// Benchmark for enqueue_ring_deletion_authorized - enqueuing a ring for deletion as part of
	/// collection deletion. Parameter `p` is the number of pages in the ring.
	#[benchmark]
	fn enqueue_ring_deletion_authorized(
		p: Linear<1, { Pallet::<T>::ring_pages_absolute_upper_limit() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		// Create collection
		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Generate and add members
		let members = generate_members_for_ring::<T>(SEED, ring_size);
		add_members_to_collection::<T>(&identifier, &members);

		// Onboard and build a ring
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		let to_include = pallet::Pallet::<T>::should_build_ring(
			&identifier,
			RI_ZERO,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
		)
		.unwrap();
		assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));

		// Add additional pages to RingKeys to simulate multi-page rings
		let page_size = RingCapacityFromExponent::<T>::get();
		for page_idx in 1..p {
			let synthetic_members: BoundedVec<MemberOf<T>, RingCapacityFromExponent<T>> = members
				.iter()
				.take(page_size as usize)
				.map(|(_, m)| m.clone())
				.collect::<Vec<_>>()
				.try_into()
				.unwrap();
			RingKeys::<T>::insert((&identifier, RI_ZERO, page_idx), synthetic_members);
		}

		// Update ring status to reflect all pages
		let total_members = p * page_size;
		RingKeysStatus::<T>::insert(
			identifier,
			RI_ZERO,
			RingStatus { total: total_members, included: total_members, immutable_since: None },
		);

		// Move to suspended state
		let collection_info = Collections::<T>::take(identifier).unwrap();
		SuspendedCollections::<T>::insert(identifier, collection_info);

		// Drive the OnRingRootChange impl onto its worst-case branch
		<T::OnRingRootChange as OnRingRootChange<MembersOf<T>>>::bench_setup_worst_case();

		#[extrinsic_call]
		enqueue_ring_deletion_authorized(SystemOrigin::Authorized, identifier, RI_ZERO);

		// Verify ring was cleaned up
		assert!(Root::<T>::get(identifier, RI_ZERO).is_none());
		assert_eq!(RingKeysStatus::<T>::get(identifier, RI_ZERO).total, 0);

		Ok(())
	}

	/// Benchmark for delete_onboarding_queue_page_authorized - deleting an onboarding queue page
	/// as part of collection deletion. Parameter `n` is the number of members in the page.
	#[benchmark]
	fn delete_onboarding_queue_page_authorized(
		n: Linear<0, { T::MaxFlexibleRingExponent::get().ring_capacity() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;

		// Create collection
		setup_collection::<T>(
			identifier,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Generate and add members (they go to onboarding queue)
		let member_list = generate_members::<T>(SEED, 0, n);
		add_members_to_collection::<T>(&identifier, &member_list);

		// Don't onboard - leave them in the queue
		let queue_page = OnboardingQueue::<T>::get(identifier, 0u32);
		assert_eq!(queue_page.len(), n as usize);

		// Move to suspended state
		let collection_info = Collections::<T>::take(identifier).unwrap();
		SuspendedCollections::<T>::insert(identifier, collection_info);

		#[extrinsic_call]
		delete_onboarding_queue_page_authorized(SystemOrigin::Authorized, identifier, 0u32);

		// Verify queue page was deleted
		assert!(OnboardingQueue::<T>::get(identifier, 0u32).is_empty());

		Ok(())
	}

	#[benchmark]
	fn finalize_collection_deletion_authorized() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;

		// Create collection
		let _owner = setup_collection::<T>(
			identifier,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Get the collection info and owner before we modify things
		let collection_info = Collections::<T>::take(identifier).unwrap();
		let owner = collection_info.owner.clone();

		// Place the target identifier at index 0 so `retain` shifts every other
		// entry. The remaining slots are filled with synthetic identifiers up
		// to `MaxCollections`.
		let max_collections = T::MaxCollections::get();
		let mut all_ids: Vec<Identifier> = Vec::with_capacity(max_collections as usize);
		all_ids.push(identifier);
		all_ids.extend((0..max_collections - 1).map(|i| {
			let mut id = [0u8; 32];
			id[0..4].copy_from_slice(&i.to_le_bytes());
			id
		}));
		let identifiers: BoundedVec<Identifier, T::MaxCollections> = all_ids.try_into().unwrap();
		IdentifiersOf::<T>::insert(&owner, identifiers);

		// Move to suspended state
		SuspendedCollections::<T>::insert(identifier, collection_info);

		#[extrinsic_call]
		finalize_collection_deletion_authorized(SystemOrigin::Authorized, identifier);

		// Verify collection was fully deleted
		assert!(SuspendedCollections::<T>::get(identifier).is_none());
		// Verify identifier was removed from owner's list
		assert_eq!(IdentifiersOf::<T>::get(&owner).unwrap().len(), (max_collections - 1) as usize);

		Ok(())
	}

	/// Benchmark for delete_ring_page_authorized - processing a ring keys page from the
	/// deletion queue. Parameter `n` is the number of members in the page.
	#[benchmark]
	fn delete_ring_page_authorized(
		n: Linear<1, { T::MaxFlexibleRingExponent::get().ring_capacity() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;

		// Create collection with small onboarding size
		setup_collection::<T>(identifier, 1, T::MaxFlexibleRingExponent::get(), RingMode::Flexible);

		// Generate members for the page we want to delete
		let member_list = generate_members::<T>(SEED, 0, n);

		// Add members directly to Members storage
		for (i, (_, member)) in member_list.iter().enumerate() {
			Members::<T>::insert(
				identifier,
				member,
				RingPosition::Included {
					ring_index: RI_ZERO,
					ring_page: 0,
					ring_position: i as u32,
				},
			);
		}

		// Create a ring keys page with the members
		let ring_keys: BoundedVec<MemberOf<T>, RingCapacityFromExponent<T>> = member_list
			.iter()
			.map(|(_, m)| m.clone())
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		RingKeys::<T>::insert((identifier, RI_ZERO, 0u32), ring_keys);

		// Enqueue the ring page for deletion
		RingDeletionQueue::<T>::insert((identifier, RI_ZERO, 0u32), ());

		// Verify members exist
		for (_, member) in member_list.iter() {
			assert!(Members::<T>::get(identifier, member).is_some());
		}

		#[extrinsic_call]
		_(SystemOrigin::Authorized, identifier, RI_ZERO, 0u32);

		// Verify members were deleted
		for (_, member) in member_list.iter() {
			assert!(Members::<T>::get(identifier, member).is_none());
		}

		Ok(())
	}

	/// Benchmark for ensure_can_enqueue_ring_deletion - validates that a ring can be
	/// enqueued for deletion.
	#[benchmark]
	fn ensure_can_enqueue_ring_deletion() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		let members = generate_members_for_ring::<T>(SEED, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));
		let to_include =
			pallet::Pallet::<T>::should_build_ring(&identifier, RI_ZERO, ring_size).unwrap();
		assert_ok!(pallet::Pallet::<T>::build_ring(&identifier, RI_ZERO, to_include));

		// Suspend the collection
		let collection_info = Collections::<T>::take(identifier).unwrap();
		SuspendedCollections::<T>::insert(identifier, collection_info);

		let call = Call::<T>::enqueue_ring_deletion_authorized { identifier, ring_index: RI_ZERO };

		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}

		Ok(())
	}

	/// Benchmark for ensure_can_delete_onboarding_queue_page - validates that an
	/// onboarding queue page can be deleted.
	#[benchmark]
	fn ensure_can_delete_onboarding_queue_page() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();
		let identifier = BENCH_IDENTIFIER;

		setup_collection::<T>(
			identifier,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Add members to create a queue page
		let member_list = generate_members::<T>(SEED, 0, 1);
		add_members_to_collection::<T>(&identifier, &member_list);

		// Suspend the collection
		let collection_info = Collections::<T>::take(identifier).unwrap();
		SuspendedCollections::<T>::insert(identifier, collection_info);

		let call =
			Call::<T>::delete_onboarding_queue_page_authorized { identifier, page_index: 0u32 };

		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}

		Ok(())
	}

	/// Benchmark for ensure_can_finalize_collection_deletion - validates that a
	/// collection deletion can be finalized.
	#[benchmark]
	fn ensure_can_finalize_collection_deletion() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();
		let identifier = BENCH_IDENTIFIER;

		setup_collection::<T>(
			identifier,
			T::MaxFlexibleRingExponent::get().ring_capacity(),
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		// Suspend the collection
		let collection_info = Collections::<T>::take(identifier).unwrap();
		SuspendedCollections::<T>::insert(identifier, collection_info);

		let call = Call::<T>::finalize_collection_deletion_authorized { identifier };

		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}

		Ok(())
	}

	/// `self_include` page-removal branch: a single-member page goes empty after
	/// the call, exercising `OnboardingQueue::remove` and the head-advance write.
	#[benchmark]
	fn self_include_remove_page() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
		let self_inclusion_delay = 3600u64;

		let owner: T::Location =
			Decode::decode(&mut &[0u8; 32][..]).expect("Location should be decodable from bytes");
		pallet::Pallet::<T>::create_collection(
			owner,
			&identifier,
			ring_size,
			RingMode::Flexible,
			T::MaxFlexibleRingExponent::get(),
			Some(self_inclusion_delay),
		)
		.expect("Failed to create collection");
		pallet::Pallet::<T>::initialize_chunks(T::MaxFlexibleRingExponent::get());

		let (_secret, member) = new_member_from::<T>(0, SEED);
		pallet::Pallet::<T>::add_members(&identifier, vec![member.clone()])
			.expect("Failed to add member");

		assert!(Members::<T>::get(identifier, &member).is_some());

		#[extrinsic_call]
		self_include(Origin::SelfInclude(member.clone()), identifier, member.clone(), 0);

		// Verify member is now included.
		let position = Members::<T>::get(identifier, &member).expect("member should exist");
		assert!(matches!(position, RingPosition::Included { .. }));

		Ok(())
	}

	/// `self_include` page-kept branch: target sits at the front of an `n`-member
	/// page so `Vec::remove` shifts the full `n - 1` tail (the dominant linear cost).
	#[benchmark]
	fn self_include_keep_page(
		n: Linear<2, { T::OnboardingQueuePageSize::get() }>,
	) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
		let self_inclusion_delay = 3600u64;

		let owner: T::Location =
			Decode::decode(&mut &[0u8; 32][..]).expect("Location should be decodable from bytes");
		pallet::Pallet::<T>::create_collection(
			owner,
			&identifier,
			ring_size,
			RingMode::Flexible,
			T::MaxFlexibleRingExponent::get(),
			Some(self_inclusion_delay),
		)
		.expect("Failed to create collection");
		pallet::Pallet::<T>::initialize_chunks(T::MaxFlexibleRingExponent::get());

		let members: Vec<MemberOf<T>> = (0..n).map(|i| new_member_from::<T>(i, SEED).1).collect();
		pallet::Pallet::<T>::add_members(&identifier, members.clone())
			.expect("Failed to add members");

		// dispatch does two linear-in-n operations. Scan with worst case target = last idx
		// and Vec::remove() where worst case taget = first idx.
		// computation_shift > computation_compare, so we target first
		let target = members.first().expect("non-empty").clone();
		assert!(Members::<T>::get(identifier, &target).is_some());

		#[extrinsic_call]
		self_include(Origin::SelfInclude(target.clone()), identifier, target.clone(), 0);

		// Verify member is now included and the page survived with `n - 1` keys.
		let position = Members::<T>::get(identifier, &target).expect("member should exist");
		assert!(matches!(position, RingPosition::Included { .. }));
		assert_eq!(OnboardingQueue::<T>::get(identifier, 0).len() as u32, n - 1);

		Ok(())
	}

	/// Benchmark for the `AsMember` transaction extension validation.
	/// Instantiates the extension and runs it via `test_run` with the proper call.
	#[benchmark]
	fn validate_self_include() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		use crate::extension::{AsMember, AsMemberInfo};
		use sp_runtime::traits::DispatchTransaction;

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();
		let self_inclusion_delay = 3600u64;

		// Set the clock to a value large enough so that saturating_sub on
		// queued_at doesn't clamp to 0 (genesis timestamp is typically 0).
		T::BenchmarkHelper::set_time(Duration::from_secs(self_inclusion_delay * 2 + 1));

		let owner: T::Location =
			Decode::decode(&mut &[0u8; 32][..]).expect("Location should be decodable from bytes");
		pallet::Pallet::<T>::create_collection(
			owner,
			&identifier,
			ring_size,
			RingMode::Flexible,
			T::MaxFlexibleRingExponent::get(),
			Some(self_inclusion_delay),
		)
		.expect("Failed to create collection");

		// Add a single member.
		let (secret, member) = new_member_from::<T>(0, SEED);
		pallet::Pallet::<T>::add_members(&identifier, vec![member.clone()])
			.expect("Failed to add member");

		let now = T::Clock::now().as_secs();
		let call_valid_at = now;

		// Set queued_at to the past so the delay has elapsed.
		Members::<T>::mutate(identifier, &member, |maybe_record| {
			if let Some(position) = maybe_record {
				*position = RingPosition::Onboarding {
					queue_page: 0,
					queued_at: now.saturating_sub(self_inclusion_delay + 1),
				};
			}
		});

		// Build the call and sign the inherited implication. For a single extension,
		// `test_run` passes `TxBaseImplication((extension_version, &call))` where
		// extension_version = 0.
		let call = Call::<T>::self_include { identifier, member: member.clone(), call_valid_at };
		let runtime_call: <T as frame_system::Config>::RuntimeCall = call.into();
		let msg = (0u8, &runtime_call).using_encoded(sp_io::hashing::blake2_256);
		let signature = T::Crypto::sign(&secret, &msg[..]).expect("signing should not fail");

		let tx_ext = AsMember::<T>::new(Some(AsMemberInfo::SelfInclude(signature)));
		let origin = frame_system::RawOrigin::None;
		let len = runtime_call.encode().len();

		#[block]
		{
			tx_ext
				.test_run(origin.into(), &runtime_call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}
	/// Benchmark for ensure_can_clean_up_old_roots - validates old roots for cleanup.
	#[benchmark]
	fn ensure_can_clean_up_old_roots() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;

		// Create collection
		setup_collection::<T>(identifier, 1, T::MaxFlexibleRingExponent::get(), RingMode::Flexible);

		for i in 0..=1 {
			let member = new_member_from::<T>(i, 0);
			Pallet::<T>::add_members(&identifier, vec![member.1]).expect("Failed to add members");
			Pallet::<T>::onboard_members(&identifier, false).unwrap();
			Pallet::<T>::build_ring(&identifier, RI_ZERO, 1).unwrap();
		}

		// Verify old roots were inserted
		assert_eq!(OldRoots::<T>::iter_prefix((identifier, RI_ZERO)).count(), 1);

		// Set time to after expiration: old roots were archived at current time,
		// so we need to advance past (current_time + retention_duration)
		let retention = T::OldRootRetentionDuration::get();
		let current_time = T::Clock::now().as_secs();
		T::BenchmarkHelper::set_time(Duration::from_secs(current_time + retention + 1));

		let call = Call::<T>::clean_up_old_roots_authorized {
			identifier,
			ring_index: RI_ZERO,
			limit: 100,
		};

		#[block]
		{
			call.authorize(TransactionSource::Local).unwrap().unwrap();
		}

		Ok(())
	}

	/// Benchmark for clean_up_old_roots_authorized - removes expired old roots.
	/// Parameter `n` is the number of old roots to remove.
	#[benchmark]
	fn clean_up_old_roots_authorized(n: Linear<1, 100>) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;

		// Create collection
		setup_collection::<T>(identifier, 1, T::MaxFlexibleRingExponent::get(), RingMode::Flexible);

		for i in 0..=n {
			let member = new_member_from::<T>(i, 0);
			Pallet::<T>::add_members(&identifier, vec![member.1]).expect("Failed to add members");
			Pallet::<T>::onboard_members(&identifier, false).unwrap();
			Pallet::<T>::build_ring(&identifier, RI_ZERO, 1).unwrap();
		}

		// Verify old roots were inserted
		assert_eq!(OldRoots::<T>::iter_prefix((identifier, RI_ZERO)).count(), n as usize);

		// Set time to after expiration: old roots were archived at current time,
		// so we need to advance past (current_time + retention_duration)
		let retention = T::OldRootRetentionDuration::get();
		let current_time = T::Clock::now().as_secs();
		T::BenchmarkHelper::set_time(Duration::from_secs(current_time + retention + 1));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, identifier, RI_ZERO, n);

		// Verify all old roots were removed
		assert_eq!(OldRoots::<T>::iter_prefix((identifier, RI_ZERO)).count(), 0);

		Ok(())
	}

	/// Benchmark for ensure_can_mark_ring_stale - validates that a ring can be marked stale.
	#[benchmark]
	fn ensure_can_mark_ring_stale() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		let members = generate_members_for_ring::<T>(SEED, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));

		// Ring has total > included (members onboarded but ring not built).
		let ring_status = RingKeysStatus::<T>::get(identifier, RI_ZERO);
		assert!(ring_status.total > ring_status.included);

		// Remove StaleRings entry that onboarding inserted.
		StaleRings::<T>::remove(identifier, RI_ZERO);

		let call = Call::<T>::mark_ring_stale_authorized { identifier, ring_index: RI_ZERO };

		#[block]
		{
			call.authorize(TransactionSource::External).unwrap().unwrap();
		}

		Ok(())
	}

	/// Benchmark for mark_ring_stale_authorized - marks a ring as stale.
	#[benchmark]
	fn mark_ring_stale_authorized() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_valid_time();

		let identifier = BENCH_IDENTIFIER;
		let ring_size: u32 = T::MaxFlexibleRingExponent::get().ring_capacity();

		setup_collection::<T>(
			identifier,
			ring_size,
			T::MaxFlexibleRingExponent::get(),
			RingMode::Flexible,
		);

		let members = generate_members_for_ring::<T>(SEED, ring_size);
		add_members_to_collection::<T>(&identifier, &members);
		assert_ok!(pallet::Pallet::<T>::onboard_members(&identifier, false));

		// Remove StaleRings entry that onboarding inserted.
		StaleRings::<T>::remove(identifier, RI_ZERO);
		assert!(!StaleRings::<T>::contains_key(identifier, RI_ZERO));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, identifier, RI_ZERO);

		assert!(StaleRings::<T>::contains_key(identifier, RI_ZERO));

		Ok(())
	}

	// Implements a test for each benchmark. Execute with:
	// `cargo test -p indiv-pallet-members --features runtime-benchmarks`.
	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
