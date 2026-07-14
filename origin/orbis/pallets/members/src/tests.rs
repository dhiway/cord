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

use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use indiv_support::traits::{
	AppendOnlyMembers, FlexibleMembers, MembershipProver, RingExponent, RingStatus,
};
use sp_runtime::transaction_validity::InvalidTransaction;
use verifiable::{BatchProofItem, GenerateVerifiable};

/// Generate members with a distinct first byte to avoid collisions with standard
/// `generate_members` calls. Returns (member, secret) pairs.
fn generate_members_with_offset(
	identifier: Identifier,
	start: u8,
	end: u8,
	offset_byte: u8,
) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
	let mut members = Vec::new();
	for i in start..=end {
		let mut seed = [i; 32];
		seed[0] = offset_byte;
		let secret = MockCrypto::new_secret(seed);
		let public = MockCrypto::member_from_secret(&secret);
		members.push((public, secret));
	}

	let member_keys: Vec<_> = members.iter().map(|(m, _)| *m).collect();
	<MembersPallet as AppendOnlyMembers>::add_members(&identifier, member_keys)
		.expect("Failed to add members");

	members
}

mod collection_tests {
	use super::*;

	#[test]
	fn create_collection_works() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			let identifier = TEST_IDENTIFIER;

			assert_ok!(<MembersPallet as AppendOnlyMembers>::create_collection(
				owner.clone(),
				&identifier,
				5,
				RingMode::AppendOnly,
				RingExponent::R2e9,
				None,
			));

			// Check collection exists
			assert!(Collections::<Test>::contains_key(identifier));

			// Check collection info
			let info = Collections::<Test>::get(identifier).unwrap();
			assert_eq!(info.owner, CollectionOwner::External(owner.clone()));
			assert_eq!(info.mode, RingMode::AppendOnly);
			assert_eq!(info.ring_size, RingExponent::R2e9);

			// Check onboarding size
			assert_eq!(OnboardingSize::<Test>::get(identifier), 5);

			// Check owner's identifiers
			let owner_key = CollectionOwner::External(owner);
			let identifiers = IdentifiersOf::<Test>::get(&owner_key).unwrap();
			assert!(identifiers.contains(&identifier));
		});
	}

	#[test]
	fn create_collection_fails_if_already_exists() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::create_collection(
					owner,
					&TEST_IDENTIFIER,
					5,
					RingMode::AppendOnly,
					RingExponent::R2e9,
					None,
				),
				Error::<Test>::CollectionAlreadyExists
			);
		});
	}

	#[test]
	fn create_collection_fails_if_onboarding_size_too_large() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			let max_ring_size = RingExponent::R2e9.ring_capacity();

			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::create_collection(
					owner,
					&TEST_IDENTIFIER,
					max_ring_size + 1,
					RingMode::AppendOnly,
					RingExponent::R2e9,
					None,
				),
				Error::<Test>::InvalidOnboardingSize
			);
		});
	}

	#[test]
	fn create_flexible_collection_works() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			let identifier = TEST_IDENTIFIER;

			assert_ok!(<MembersPallet as AppendOnlyMembers>::create_collection(
				owner,
				&identifier,
				5,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			let info = Collections::<Test>::get(identifier).unwrap();
			assert_eq!(info.mode, RingMode::Flexible);
		});
	}
}

mod member_tests {
	use super::*;

	#[test]
	fn add_members_works() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			create_test_collection(TEST_IDENTIFIER, 5);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));

			// The MemberAdded event is emitted.
			System::assert_has_event(Event::<Test>::MemberAdded { key: member }.into());

			// Check member is in the onboarding queue
			let position = Members::<Test>::get(TEST_IDENTIFIER, member).unwrap();
			assert!(matches!(position, RingPosition::Onboarding { .. }));
		});
	}

	#[test]
	fn add_members_fails_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::add_members(
					&NONEXISTENT_IDENTIFIER,
					vec![member]
				),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn add_invalid_member_fails() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// INVALID_MEMBER is defined in mock as always invalid
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::add_members(
					&TEST_IDENTIFIER,
					vec![INVALID_MEMBER]
				),
				Error::<Test>::InvalidMemberKey
			);
		});
	}

	#[test]
	fn add_duplicate_member_fails() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));

			// Adding the same member again should fail
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::add_members(&TEST_IDENTIFIER, vec![member]),
				Error::<Test>::KeyAlreadyInUse
			);
		});
	}

	#[test]
	fn add_multiple_members_works() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let members = generate_members(TEST_IDENTIFIER, 1, 5);

			// All members should be in the onboarding queue
			for (member, _) in &members {
				let position = Members::<Test>::get(TEST_IDENTIFIER, member).unwrap();
				assert!(matches!(position, RingPosition::Onboarding { .. }));
			}
		});
	}
}

mod onboarding_tests {
	use super::*;

	#[test]
	fn onboard_members_works() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// Add enough members to meet onboarding size
			let members = generate_members(TEST_IDENTIFIER, 1, 10);

			// Run onboarding
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Check that members are now included in a ring
			let included_count = members
				.iter()
				.filter(|(m, _)| {
					let position = Members::<Test>::get(TEST_IDENTIFIER, m).unwrap();
					matches!(position, RingPosition::Included { .. })
				})
				.count();
			assert!(included_count > 0);
		});
	}

	#[test]
	fn onboard_returns_false_with_insufficient_members() {
		TestExt::new().execute_with(|| {
			// Create collection with onboarding size of 5
			create_test_collection(TEST_IDENTIFIER, 5);

			// Only add 2 members (less than onboarding size)
			let _members = generate_members(TEST_IDENTIFIER, 1, 2);

			// Onboarding should return Ok(false) due to insufficient members
			assert_eq!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false), Ok(false));
		});
	}
}

mod ring_building_tests {
	use super::*;

	#[test]
	fn build_ring_works() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			create_test_collection(TEST_IDENTIFIER, 5);
			let _members = generate_members(TEST_IDENTIFIER, 1, 10);

			// Onboard members first
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Check if we should build ring
			let ring_index = 0;
			let maybe_to_include =
				MembersPallet::should_build_ring(&TEST_IDENTIFIER, ring_index, 10);
			assert!(maybe_to_include.is_some());

			// Build the ring
			let to_include = maybe_to_include.unwrap();
			assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, ring_index, to_include));

			// The RingBuilt event is emitted.
			System::assert_has_event(
				Event::<Test>::RingBuilt { identifier: TEST_IDENTIFIER, ring_index }.into(),
			);

			// Check ring root exists
			assert!(Root::<Test>::contains_key(TEST_IDENTIFIER, ring_index));

			// Check ring status
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, ring_index);
			assert!(status.included > 0);
		});
	}

	/// Corrupted storage where `ring_status.total` is 10 but no keys exist in `RingKeys`.
	/// The `defensive_assert!` catches the inconsistency (10 != 0) and panics in debug builds.
	/// Only compiled under `debug_assertions` since `defensive_assert!` is a no-panic log in
	/// release.
	#[cfg(debug_assertions)]
	#[test]
	#[should_panic(expected = "Inconsistent ring page state")]
	fn build_ring_panics_on_total_keys_mismatch() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			RingKeysStatus::<Test>::insert(
				TEST_IDENTIFIER,
				0u32,
				RingStatus { total: 10, included: 0, immutable_since: None },
			);

			let _ = MembersPallet::build_ring(&TEST_IDENTIFIER, 0, 10);
		});
	}

	/// All keys already included (`total == included == 5`) but `build_ring` is called with
	/// `to_include = 5`. Without the `to_push == 0` guard this is an infinite loop because
	/// `to_push` stays 0 and `to_include` never decreases. The guard breaks out immediately.
	#[test]
	fn build_ring_terminates_when_nothing_left_to_include() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			RingKeysStatus::<Test>::insert(
				TEST_IDENTIFIER,
				0u32,
				RingStatus { total: 5, included: 5, immutable_since: None },
			);

			assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, 5));
		});
	}
}

mod ring_status_tests {
	use super::*;

	#[test]
	fn ring_status_returns_none_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			let status =
				<MembersPallet as AppendOnlyMembers>::ring_status(&NONEXISTENT_IDENTIFIER, 0);
			assert!(status.is_none());
		});
	}

	#[test]
	fn ring_status_works_for_existing_collection() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let status = <MembersPallet as AppendOnlyMembers>::ring_status(&TEST_IDENTIFIER, 0);
			assert!(status.is_some());
			let status = status.unwrap();
			assert_eq!(status.total, 0);
			assert_eq!(status.included, 0);
		});
	}
}

mod member_status_tests {
	use super::*;

	#[test]
	fn member_status_returns_none_for_nonexistent_member() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let secret = MockCrypto::new_secret([99; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			let status =
				<MembersPallet as AppendOnlyMembers>::member_status(&TEST_IDENTIFIER, &member);
			assert!(status.is_none());
		});
	}

	#[test]
	fn member_status_returns_none_for_wrong_collection() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			create_test_collection(TEST_IDENTIFIER_2, 5);

			// Add member to TEST_IDENTIFIER
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));

			// Query status for wrong collection
			let status =
				<MembersPallet as AppendOnlyMembers>::member_status(&TEST_IDENTIFIER_2, &member);
			assert!(status.is_none());
		});
	}

	#[test]
	fn member_status_works_for_existing_member() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));

			let status =
				<MembersPallet as AppendOnlyMembers>::member_status(&TEST_IDENTIFIER, &member);
			assert!(status.is_some());
			assert!(matches!(status.unwrap(), RingPosition::Onboarding { .. }));
		});
	}
}

mod active_count_tests {
	use super::*;

	#[test]
	fn active_count_is_zero_for_new_collection() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			assert_eq!(<MembersPallet as AppendOnlyMembers>::active_count(&TEST_IDENTIFIER), 0);
		});
	}

	#[test]
	fn active_count_is_zero_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			// For nonexistent collections, active_count returns 0 (default)
			assert_eq!(
				<MembersPallet as AppendOnlyMembers>::active_count(&NONEXISTENT_IDENTIFIER),
				0
			);
		});
	}

	#[test]
	fn active_count_increases_after_onboarding() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			let _members = generate_members(TEST_IDENTIFIER, 1, 10);

			assert_eq!(<MembersPallet as AppendOnlyMembers>::active_count(&TEST_IDENTIFIER), 0);

			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			assert!(<MembersPallet as AppendOnlyMembers>::active_count(&TEST_IDENTIFIER) > 0);
		});
	}
}

mod removal_session_tests {
	use super::*;

	#[test]
	fn start_removal_session_works() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));

			let state = RingsState::<Test>::get(TEST_IDENTIFIER);
			assert!(state.mutating());
		});
	}

	#[test]
	fn end_removal_session_works() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER));

			let state = RingsState::<Test>::get(TEST_IDENTIFIER);
			assert!(state.append_only());
		});
	}

	#[test]
	fn end_removal_session_fails_without_active_session() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_noop!(
				<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER),
				Error::<Test>::NoRemovalSession
			);
		});
	}

	#[test]
	fn remove_members_fails_without_active_session() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_noop!(
				<MembersPallet as FlexibleMembers>::remove_members(&TEST_IDENTIFIER, &[member]),
				Error::<Test>::NoRemovalSession
			);
		});
	}

	#[test]
	fn remove_members_works_with_active_session() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			let members = generate_members(TEST_IDENTIFIER, 1, 10);

			// Onboard members
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Build the ring
			let maybe_to_include = MembersPallet::should_build_ring(&TEST_IDENTIFIER, 0, 10);
			if let Some(to_include) = maybe_to_include {
				assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, to_include));
			}

			// Find an included member
			let included_member = members
				.iter()
				.find(|(m, _)| {
					let position = Members::<Test>::get(TEST_IDENTIFIER, m).unwrap();
					matches!(position, RingPosition::Included { .. })
				})
				.map(|(m, _)| *m);

			if let Some(member) = included_member {
				// Start removal session
				assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(
					&TEST_IDENTIFIER
				));

				// Remove the member
				assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
					&TEST_IDENTIFIER,
					&[member]
				));

				// Check member is now suspended
				let position = Members::<Test>::get(TEST_IDENTIFIER, member).unwrap();
				assert!(matches!(position, RingPosition::Suspended));
			}
		});
	}

	#[test]
	fn member_removed_event_is_deposited() {
		TestExt::new().execute_with(|| {
			// Set block number > 0 to enable event recording
			System::set_block_number(1);

			create_test_collection(TEST_IDENTIFIER, 5);
			let members = generate_members(TEST_IDENTIFIER, 1, 10);

			// Onboard members
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Build the ring
			let maybe_to_include = MembersPallet::should_build_ring(&TEST_IDENTIFIER, 0, 10);
			if let Some(to_include) = maybe_to_include {
				assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, to_include));
			}

			// Find an included member
			let included_member = members
				.iter()
				.find(|(m, _)| {
					let position = Members::<Test>::get(TEST_IDENTIFIER, m).unwrap();
					matches!(position, RingPosition::Included { .. })
				})
				.map(|(m, _)| *m)
				.expect("should have at least one included member");

			// Start removal session
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));

			// Queue the member for removal (marks as Suspended)
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&TEST_IDENTIFIER,
				&[included_member]
			));

			// End the removal session to transition back to append_only state
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER));

			// Clear events before the removal
			System::reset_events();

			// Trigger the actual removal from the ring
			MembersPallet::remove_suspended_keys(&TEST_IDENTIFIER, 0);

			// Check that MemberRemoved event was deposited
			let events = System::events();
			assert!(
				events.iter().any(|e| matches!(
					e.event,
					RuntimeEvent::MembersPallet(Event::MemberRemoved { key }) if key == included_member
				)),
				"MemberRemoved event should be deposited"
			);
		});
	}

	#[test]
	fn start_removal_session_returns_ok_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			// Should return Ok without error for non-existent collection
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(
				&NONEXISTENT_IDENTIFIER
			));

			// RingsState should remain at default (not mutating)
			let state = RingsState::<Test>::get(NONEXISTENT_IDENTIFIER);
			assert!(!state.mutating());
		});
	}

	#[test]
	fn end_removal_session_returns_ok_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			// Should return Ok without error for non-existent collection
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(
				&NONEXISTENT_IDENTIFIER
			));

			// RingsState should remain at default
			let state = RingsState::<Test>::get(NONEXISTENT_IDENTIFIER);
			assert!(!state.mutating());
		});
	}

	#[test]
	fn remove_suspended_keys_clears_root_and_increments_revision() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			let members = generate_members(TEST_IDENTIFIER, 1, 10);

			// Onboard members
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Build the ring
			let maybe_to_include = MembersPallet::should_build_ring(&TEST_IDENTIFIER, 0, 10);
			if let Some(to_include) = maybe_to_include {
				assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, to_include));
			}

			// Record the initial root state
			let root_before =
				Root::<Test>::get(TEST_IDENTIFIER, 0).expect("root should exist after build_ring");
			assert!(!root_before.root.is_empty(), "root should have members after build_ring");
			let revision_before = root_before.revision;

			// Find an included member to remove
			let included_member = members
				.iter()
				.find(|(m, _)| {
					let position = Members::<Test>::get(TEST_IDENTIFIER, m).unwrap();
					matches!(position, RingPosition::Included { .. })
				})
				.map(|(m, _)| *m)
				.expect("should have at least one included member");

			// Start removal session, suspend the member, end session
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&TEST_IDENTIFIER,
				&[included_member]
			));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER));

			// Perform the actual removal
			MembersPallet::remove_suspended_keys(&TEST_IDENTIFIER, 0);

			// Root should be cleared (empty commitment) and revision incremented
			let root_after =
				Root::<Test>::get(TEST_IDENTIFIER, 0).expect("root should still exist");
			assert!(
				root_after.root.is_empty(),
				"root should be cleared after removing suspended keys"
			);
			assert_eq!(
				root_after.revision,
				revision_before + 1,
				"revision should be incremented after removing suspended keys"
			);

			// Ring should be marked as stale for rebuild
			assert!(
				StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0),
				"ring should be marked as stale after removing suspended keys"
			);

			// RingKeysStatus should reflect the removal
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0);
			assert_eq!(status.included, 0, "included should be reset to 0 after removal");
		});
	}

	#[test]
	fn suspended_member_added_to_another_collection_can_be_readded_to_original() {
		TestExt::new().execute_with(|| {
			let collection_a = TEST_IDENTIFIER;
			let collection_b = TEST_IDENTIFIER_2;

			// 1. Create 2 collections
			// create_test_collection creates Flexible collections by default,
			// which is required to use removal/suspension mechanics.
			create_test_collection(collection_a, 1);
			create_test_collection(collection_b, 1);

			// 2. Add a member to collection A
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&collection_a,
				vec![member]
			));

			// Onboard and build ring to make them fully active in A first
			assert_ok!(MembersPallet::onboard_members(&collection_a, true));

			// Member should be included in A
			let position = Members::<Test>::get(collection_a, member).unwrap();
			assert!(matches!(position, RingPosition::Included { .. }));

			// 3. Suspend the member in collection A
			// Must start a removal session to modify membership in Flexible collections
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&collection_a));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&collection_a,
				&[member]
			));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&collection_a));

			// Member should be suspended in A
			let position = Members::<Test>::get(collection_a, member).unwrap();
			assert!(matches!(position, RingPosition::Suspended));

			// 4. Add the member to collection B
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&collection_b,
				vec![member]
			));

			// Member should be onboarding in B
			let position = Members::<Test>::get(collection_b, member).unwrap();
			assert!(matches!(position, RingPosition::Onboarding { .. }));

			// 5. Re-add the member to collection A
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&collection_a,
				vec![member]
			));

			// Member should be onboarding in A
			let position = Members::<Test>::get(collection_a, member).unwrap();
			assert!(matches!(position, RingPosition::Onboarding { .. }));
		});
	}

	#[test]
	fn suspend_all_members_does_not_leave_phantom_stale_ring() {
		TestExt::new().execute_with(|| {
			// Setup: create a collection, onboard members, and build the ring.
			let members = setup_collection_with_built_ring(TEST_IDENTIFIER, 10);
			assert!(Root::<Test>::contains_key(TEST_IDENTIFIER, 0));

			// StaleRings should be clear after a successful build.
			assert!(!StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0));

			// Collect all included members.
			let included: Vec<_> = members
				.iter()
				.filter_map(|(m, _)| {
					let pos = Members::<Test>::get(TEST_IDENTIFIER, m)?;
					matches!(pos, RingPosition::Included { .. }).then_some(*m)
				})
				.collect();
			assert!(!included.is_empty(), "should have included members");

			// Suspend every member.
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&TEST_IDENTIFIER,
				&included
			));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER));

			// Process the suspensions.
			MembersPallet::remove_suspended_keys(&TEST_IDENTIFIER, 0);

			// The ring now has total=0, included=0. There is nothing to build, so
			// StaleRings must NOT contain an entry for this ring.
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0);
			assert_eq!(status.total, 0);
			assert_eq!(status.included, 0);
			assert!(
				!StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0),
				"empty ring should not be marked as stale"
			);
		});
	}
}

mod merge_rings_tests {
	use super::*;

	#[test]
	fn merge_rings_works() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let identifier = TEST_IDENTIFIER;

			// Create Flexible collection with onboarding_size=1
			create_test_collection(identifier, 1);

			// Fill ring 0 to capacity (255 members) using seeds 1..=255
			let ring0_members = generate_members(identifier, 1, 255);

			// Drain onboarding queue
			while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

			// Build ring 0
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
				} else {
					break;
				}
			}

			// Verify ring 0 is full and CurrentRingIndex advanced to 1
			let status0 = RingKeysStatus::<Test>::get(identifier, 0);
			assert_eq!(status0.total, 255);
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 1);

			// Add 10 members to ring 1 using a different seed pattern
			let ring1_members = generate_members_with_offset(identifier, 1, 10, 0xAA);

			// Drain onboarding queue for ring 1
			while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

			// Build ring 1
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 1, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 1, to_include));
				} else {
					break;
				}
			}

			let status1 = RingKeysStatus::<Test>::get(identifier, 1);
			assert_eq!(status1.total, 10);

			// Advance to ring 2 so neither ring 0 nor ring 1 is current
			manually_advance_to_ring(&identifier, 2);

			// Start removal session and remove 130 members from ring 0
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));

			let members_to_remove: Vec<_> =
				ring0_members.iter().take(130).map(|(m, _)| *m).collect();
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&identifier,
				&members_to_remove
			));

			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Clean ring 0
			MembersPallet::remove_suspended_keys(&identifier, 0);

			// Verify ring 0 now has 125 members (below threshold of 127)
			let status0_after = RingKeysStatus::<Test>::get(identifier, 0);
			assert_eq!(status0_after.total, 125);
			assert!(status0_after.total < 255 / 2);

			// Ring 1 has 10 members (also below 127)
			let status1_check = RingKeysStatus::<Test>::get(identifier, 1);
			assert!(status1_check.total < 255 / 2);

			// Build ring 0 so it's no longer stale (remove stale status first)
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
				} else {
					break;
				}
			}

			// Merge ring 1 into ring 0
			assert_ok!(MembersPallet::merge_rings(RuntimeOrigin::signed(1), identifier, 0, 1));

			// The RingsMerged event is emitted.
			System::assert_has_event(
				Event::<Test>::RingsMerged { identifier, base_ring_index: 0, target_ring_index: 1 }
					.into(),
			);

			// Verify: ring 0 has combined count (125 + 10 = 135)
			let merged_status = RingKeysStatus::<Test>::get(identifier, 0);
			assert_eq!(merged_status.total, 135);

			// Ring 1 metadata should be cleared
			assert!(!Root::<Test>::contains_key(identifier, 1));
			assert!(RingKeys::<Test>::get((&identifier, 1u32, 0u32)).is_empty());
			assert_eq!(RingKeysStatus::<Test>::get(identifier, 1).total, 0);

			// Ring 0 should be marked stale (needs rebuild)
			assert!(StaleRings::<Test>::contains_key(identifier, 0));

			// Members previously in ring 1 should now point to ring 0
			for (member, _) in &ring1_members {
				let position = Members::<Test>::get(identifier, member).unwrap();
				assert!(
					matches!(position, RingPosition::Included { ring_index: 0, .. }),
					"Member from ring 1 should now be in ring 0"
				);
			}
		});
	}

	#[test]
	fn merge_rings_fails_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			assert_noop!(
				MembersPallet::merge_rings(RuntimeOrigin::signed(1), NONEXISTENT_IDENTIFIER, 0, 1),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn merge_rings_fails_for_same_ring() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_noop!(
				MembersPallet::merge_rings(RuntimeOrigin::signed(1), TEST_IDENTIFIER, 0, 0),
				Error::<Test>::InvalidRing
			);
		});
	}
}

mod set_onboarding_size_tests {
	use super::*;

	#[test]
	fn set_onboarding_size_fails_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			assert_noop!(
				MembersPallet::set_onboarding_size(
					RuntimeOrigin::root(),
					NONEXISTENT_IDENTIFIER,
					5
				),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn set_onboarding_size_fails_for_signed_origins() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_noop!(
				MembersPallet::set_onboarding_size(RuntimeOrigin::signed(1), TEST_IDENTIFIER, 3),
				sp_runtime::DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn set_onboarding_size_works_with_root() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			create_test_collection(TEST_IDENTIFIER, 5);

			assert_ok!(MembersPallet::set_onboarding_size(
				RuntimeOrigin::root(),
				TEST_IDENTIFIER,
				3
			));

			// The OnboardingSizeSet event is emitted.
			System::assert_has_event(
				Event::<Test>::OnboardingSizeSet {
					identifier: TEST_IDENTIFIER,
					onboarding_size: 3,
				}
				.into(),
			);

			assert_eq!(OnboardingSize::<Test>::get(TEST_IDENTIFIER), 3);
		});
	}

	#[test]
	fn set_onboarding_size_fails_if_too_large() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			let max_ring_size = RingExponent::R2e9.ring_capacity();

			assert_noop!(
				MembersPallet::set_onboarding_size(
					RuntimeOrigin::root(),
					TEST_IDENTIFIER,
					max_ring_size + 1
				),
				Error::<Test>::InvalidOnboardingSize
			);
		});
	}
}

mod remove_ring_tests {
	use super::*;

	#[test]
	fn remove_ring_works() {
		TestExt::new().execute_with(|| {
			let _members = setup_collection_with_built_ring(TEST_IDENTIFIER, 10);

			// Verify ring 0 exists with members
			assert!(Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0);
			assert!(status.total > 0);

			// Advance to ring 1 so ring 0 is no longer current
			manually_advance_to_ring(&TEST_IDENTIFIER, 1);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Root should be cleared
			assert!(!Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			// StaleRings should be cleared
			assert!(!StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0));
			// PendingSuspensions should be cleared
			assert!(!PendingSuspensions::<Test>::contains_key(TEST_IDENTIFIER, 0));
			// RingKeysStatus total should be 0
			assert_eq!(RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0).total, 0);

			// RingDeletionQueue should have entries (keys queued for lazy deletion)
			assert!(RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn remove_ring_fails_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::remove_ring(&NONEXISTENT_IDENTIFIER, 0),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn remove_ring_fails_for_current_ring() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// Ring 0 is the current ring
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0),
				Error::<Test>::InvalidRing
			);
		});
	}
}

mod verify_membership_tests {
	use super::*;

	#[test]
	fn verify_membership_fails_for_nonexistent_ring() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// Create a dummy proof - Simple crypto uses ([u8; 64], [u8; 32]) as proof type
			let dummy_proof = verifiable::mock::MockProof::default();
			let context = [0u8; 32];
			let message = b"test";

			// Try to verify against a nonexistent ring
			let result = <MembersPallet as MembershipProver>::verify_membership(
				&TEST_IDENTIFIER,
				&dummy_proof,
				99, // nonexistent ring
				context,
				message,
			);

			assert_eq!(result.unwrap_err(), Error::<Test>::NoRoot.into());
		});
	}
}

mod multiple_collections_tests {
	use super::*;

	#[test]
	fn owner_can_have_multiple_collections() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::create_collection(
				owner.clone(),
				&TEST_IDENTIFIER,
				5,
				RingMode::AppendOnly,
				RingExponent::R2e9,
				None,
			));

			assert_ok!(<MembersPallet as AppendOnlyMembers>::create_collection(
				owner.clone(),
				&TEST_IDENTIFIER_2,
				5,
				RingMode::AppendOnly,
				RingExponent::R2e9,
				None,
			));

			// Check both collections exist
			assert!(Collections::<Test>::contains_key(TEST_IDENTIFIER));
			assert!(Collections::<Test>::contains_key(TEST_IDENTIFIER_2));

			// Check owner has both identifiers
			let owner_key = CollectionOwner::External(owner);
			let identifiers = IdentifiersOf::<Test>::get(&owner_key).unwrap();
			assert!(identifiers.contains(&TEST_IDENTIFIER));
			assert!(identifiers.contains(&TEST_IDENTIFIER_2));
		});
	}

	#[test]
	fn members_are_isolated_between_collections() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);
			create_test_collection(TEST_IDENTIFIER_2, 5);

			// Add member to first collection
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));

			// Member should exist in first collection but not second
			let status1 =
				<MembersPallet as AppendOnlyMembers>::member_status(&TEST_IDENTIFIER, &member);
			assert!(status1.is_some());

			let status2 =
				<MembersPallet as AppendOnlyMembers>::member_status(&TEST_IDENTIFIER_2, &member);
			assert!(status2.is_none());
		});
	}

	#[test]
	fn member_can_belong_to_multiple_collections() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 1);
			create_test_collection(TEST_IDENTIFIER_2, 1);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			// Add member to both collections.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER_2,
				vec![member]
			));

			// Member should exist in both collections.
			assert!(Members::<Test>::get(TEST_IDENTIFIER, member).is_some());
			assert!(Members::<Test>::get(TEST_IDENTIFIER_2, member).is_some());
		});
	}

	#[test]
	fn member_suspension_does_not_cross_collections() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 1);
			create_test_collection(TEST_IDENTIFIER_2, 1);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			// Add member to both collections and onboard.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				vec![member]
			));
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER_2,
				vec![member]
			));
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER_2, false));

			// Suspend member in the first collection.
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&TEST_IDENTIFIER));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&TEST_IDENTIFIER,
				&[member]
			));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&TEST_IDENTIFIER));

			// Member should be suspended in the first collection.
			let position = Members::<Test>::get(TEST_IDENTIFIER, member).unwrap();
			assert!(matches!(position, RingPosition::Suspended));

			// Member should still be included in the second collection.
			let position = Members::<Test>::get(TEST_IDENTIFIER_2, member).unwrap();
			assert!(matches!(position, RingPosition::Included { .. }));
		});
	}
}

mod immutable_since_tests {
	use super::*;
	use indiv_support::traits::RingExponent;

	const APPEND_ONLY_IDENTIFIER: Identifier = [10u8; 32];
	const APPEND_ONLY_IDENTIFIER_2: Identifier = [11u8; 32];
	const FLEXIBLE_IDENTIFIER: Identifier = [20u8; 32];

	/// Helper to create an AppendOnly collection.
	fn create_append_only(identifier: Identifier, onboarding_size: u32) {
		let owner = MockLocation(1);
		<MembersPallet as AppendOnlyMembers>::create_collection(
			owner,
			&identifier,
			onboarding_size,
			RingMode::AppendOnly,
			RingExponent::R2e9, // capacity = 255
			None,
		)
		.expect("Failed to create collection");
	}

	/// Add members with unique seeds based on identifier and range.
	fn add_members_unique(identifier: &Identifier, start: u16, count: u16) -> Vec<MemberOf<Test>> {
		let members: Vec<_> = (0..count)
			.map(|i| {
				// Create unique seed by combining identifier byte and index
				let idx = start + i;
				let mut seed = [0u8; 32];
				seed[0] = identifier[0];
				seed[1] = (idx >> 8) as u8;
				seed[2] = (idx & 0xff) as u8;
				let secret = MockCrypto::new_secret(seed);
				MockCrypto::member_from_secret(&secret)
			})
			.collect();

		<MembersPallet as AppendOnlyMembers>::add_members(identifier, members.clone())
			.expect("Failed to add members");
		members
	}

	#[test]
	fn immutable_since_is_none_before_ring_is_full() {
		TestExt::new().execute_with(|| {
			// Create an AppendOnly collection with small onboarding size
			create_append_only(APPEND_ONLY_IDENTIFIER, 5);

			// Add only a few members (not enough to fill the ring of capacity 255)
			add_members_unique(&APPEND_ONLY_IDENTIFIER, 0, 10);

			// Onboard all members and build the ring
			advance_to_block(10);

			// Ring status should exist but immutable_since should be None
			let status =
				<MembersPallet as AppendOnlyMembers>::ring_status(&APPEND_ONLY_IDENTIFIER, 0);
			assert!(status.is_some());
			let status = status.unwrap();
			assert_eq!(status.total, 10);
			assert_eq!(status.included, 10);
			assert!(
				status.immutable_since.is_none(),
				"Ring is not full, immutable_since should be None"
			);
		});
	}

	#[test]
	fn immutable_since_is_set_when_append_only_ring_becomes_full() {
		TestExt::new().execute_with(|| {
			// Create an AppendOnly collection with small onboarding size
			create_append_only(APPEND_ONLY_IDENTIFIER, 5);
			add_members_unique(&APPEND_ONLY_IDENTIFIER, 0, 255);

			// Onboard all members and build the ring
			advance_to_block(10);

			// Ring status should show immutable_since is set
			let status =
				<MembersPallet as AppendOnlyMembers>::ring_status(&APPEND_ONLY_IDENTIFIER, 0);
			assert!(status.is_some());
			let status = status.unwrap();
			assert_eq!(status.total, 255, "Ring should have 255 members total");
			assert_eq!(status.included, 255, "All 255 members should be included");
			assert!(
				status.immutable_since.is_some(),
				"Ring is full and all members included, immutable_since should be set"
			);
			// Check that the timestamp is reasonable (our mock time starts at 1_000_000 seconds)
			let immutable_time = status.immutable_since.unwrap();
			assert!(immutable_time >= 1_000_000, "Timestamp should be at least 1_000_000");
		});
	}

	#[test]
	fn immutable_since_works_with_multiple_collections() {
		TestExt::new().execute_with(|| {
			// Create first AppendOnly collection and fill it
			create_append_only(APPEND_ONLY_IDENTIFIER, 5);
			add_members_unique(&APPEND_ONLY_IDENTIFIER, 0, 255);

			// Onboard all members and build the ring
			advance_to_block(10);

			// Verify ring 0 is full and immutable
			let status0 =
				<MembersPallet as AppendOnlyMembers>::ring_status(&APPEND_ONLY_IDENTIFIER, 0);
			assert!(status0.is_some());
			let status0 = status0.unwrap();
			assert_eq!(status0.total, 255);
			assert!(
				status0.immutable_since.is_some(),
				"First collection's ring should be immutable"
			);

			// Create second collection with only partial fill
			create_append_only(APPEND_ONLY_IDENTIFIER_2, 5);
			add_members_unique(&APPEND_ONLY_IDENTIFIER_2, 0, 20);

			// Onboard all members and build the ring
			advance_to_block(20);

			// Ring 0 of second collection should NOT be immutable (not full)
			let status_2 =
				<MembersPallet as AppendOnlyMembers>::ring_status(&APPEND_ONLY_IDENTIFIER_2, 0);
			assert!(status_2.is_some());
			let status_2 = status_2.unwrap();
			assert_eq!(status_2.total, 20);
			assert!(
				status_2.immutable_since.is_none(),
				"Second collection's ring is not full, should not be immutable"
			);
		});
	}

	#[test]
	fn immutable_since_is_never_set_for_flexible_collections() {
		TestExt::new().execute_with(|| {
			// Create a Flexible collection with small onboarding size
			create_test_collection(FLEXIBLE_IDENTIFIER, 5);
			add_members_unique(&FLEXIBLE_IDENTIFIER, 0, 255);

			// Onboard all members and build the ring
			advance_to_block(10);

			// Ring status should exist but immutable_since should always be None for Flexible
			let status = <MembersPallet as AppendOnlyMembers>::ring_status(&FLEXIBLE_IDENTIFIER, 0);
			assert!(status.is_some());
			let status = status.unwrap();
			assert_eq!(status.total, 255, "Flexible collection should also have 255 members");
			assert_eq!(status.included, 255);
			assert!(
				status.immutable_since.is_none(),
				"Flexible collections should never have immutable_since set, even when full"
			);
		});
	}
}

/// Helper to create a collection, add members, onboard them, and build a ring.
fn setup_collection_with_built_ring(
	identifier: Identifier,
	member_count: u8,
) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
	create_test_collection(identifier, 5);
	let members = generate_members(identifier, 1, member_count);

	// Onboard members
	assert_ok!(MembersPallet::onboard_members(&identifier, false));

	// Build the ring
	let maybe_to_include = MembersPallet::should_build_ring(&identifier, 0, member_count as u32);
	if let Some(to_include) = maybe_to_include {
		assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
	}

	members
}

/// Helper to manually advance to a new ring by setting CurrentRingIndex.
/// This simulates what would happen when a ring becomes full.
fn manually_advance_to_ring(identifier: &Identifier, new_ring: RingIndex) {
	CurrentRingIndex::<Test>::insert(identifier, new_ring);
}

mod ring_removal_tests {
	use super::*;

	#[test]
	fn remove_ring_succeeds_for_non_current_ring() {
		TestExt::new().execute_with(|| {
			// Create collection and build ring 0
			let _members = setup_collection_with_built_ring(TEST_IDENTIFIER, 10);

			// Verify ring 0 has members
			assert!(Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0);
			assert!(status.total > 0);

			// Manually advance to ring 1 (simulates ring 0 becoming full)
			manually_advance_to_ring(&TEST_IDENTIFIER, 1);

			// Now remove ring 0 (which is no longer current)
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Verify ring metadata is removed
			assert!(!Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert!(!StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert!(!PendingSuspensions::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert_eq!(RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0).total, 0);

			// But RingKeys should still exist (not yet processed by deletion queue)
			assert!(!RingKeys::<Test>::get((&TEST_IDENTIFIER, 0u32, 0u32)).is_empty());

			// RingDeletionQueue should have entries
			assert!(RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn remove_ring_is_idempotent() {
		TestExt::new().execute_with(|| {
			setup_collection_with_built_ring(TEST_IDENTIFIER, 10);

			// Manually advance to ring 1
			manually_advance_to_ring(&TEST_IDENTIFIER, 1);

			// Remove ring 0 twice
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Should still only have one set of entries in the deletion queue
			assert!(RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn remove_ring_on_empty_ring() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// Manually set up ring 1 as current (ring 0 never had members)
			CurrentRingIndex::<Test>::insert(TEST_IDENTIFIER, 1u32);

			// Try to remove ring 0 which has no members
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// No entries should be in the deletion queue (nothing to delete)
			assert!(!RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn remove_ring_correctly_calculates_page_count() {
		TestExt::new().execute_with(|| {
			create_test_collection(TEST_IDENTIFIER, 5);

			// Add some members normally
			let _members = generate_members(TEST_IDENTIFIER, 1, 40);
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));

			// Build ring
			let to_include = MembersPallet::should_build_ring(&TEST_IDENTIFIER, 0, 40).unwrap();
			assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, to_include));

			// Manually advance to next ring
			manually_advance_to_ring(&TEST_IDENTIFIER, 1);

			// Verify initial state
			let status = RingKeysStatus::<Test>::get(TEST_IDENTIFIER, 0);
			assert!(status.total > 0);

			// Remove the ring
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Should have 1 page in deletion queue (40 members < 255 page size)
			assert!(RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
			assert!(!RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 1)));
		});
	}

	#[test]
	fn remove_ring_full_flow_via_ocw() {
		TestExt::new().execute_with(|| {
			setup_collection_with_built_ring(TEST_IDENTIFIER, 20);

			// Verify ring 0 is built
			assert!(Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			let ring_keys = RingKeys::<Test>::get((&TEST_IDENTIFIER, 0u32, 0u32));
			assert!(!ring_keys.is_empty());

			// Manually advance to ring 1
			manually_advance_to_ring(&TEST_IDENTIFIER, 1);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Process deletion queue
			advance_to_block(10);

			// Everything related to ring 0 should be cleaned up
			assert!(!Root::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert!(!StaleRings::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert!(!PendingSuspensions::<Test>::contains_key(TEST_IDENTIFIER, 0));
			assert!(RingKeys::<Test>::get((&TEST_IDENTIFIER, 0u32, 0u32)).is_empty());
			assert!(!RingDeletionQueue::<Test>::contains_key((TEST_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn removed_ring_members_are_deleted_from_members_storage() {
		TestExt::new().execute_with(|| {
			// Create a simple scenario where we can track specific members
			create_test_collection(TEST_IDENTIFIER, 5);

			// Add exactly 10 members
			let mut ring0_members = Vec::new();
			for i in 1..=10u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				ring0_members.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&TEST_IDENTIFIER,
				ring0_members.clone()
			));

			// Onboard and build ring
			assert_ok!(MembersPallet::onboard_members(&TEST_IDENTIFIER, false));
			let to_include = MembersPallet::should_build_ring(&TEST_IDENTIFIER, 0, 10).unwrap();
			assert_ok!(MembersPallet::build_ring(&TEST_IDENTIFIER, 0, to_include));

			// Verify members are in Members storage with ring 0
			for member in &ring0_members {
				let position =
					Members::<Test>::get(TEST_IDENTIFIER, member).expect("Member should exist");
				assert!(matches!(position, RingPosition::Included { ring_index: 0, .. }));
			}

			// Advance to next ring
			CurrentRingIndex::<Test>::insert(TEST_IDENTIFIER, 1u32);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&TEST_IDENTIFIER, 0));

			// Process deletion queue via OCW
			advance_to_block(10);

			// Members from ring 0 should be deleted from Members storage
			for member in &ring0_members {
				assert!(
					Members::<Test>::get(TEST_IDENTIFIER, member).is_none(),
					"Member should be deleted after ring removal"
				);
			}
		});
	}

	#[test]
	fn remove_ring_retains_moved_member_via_ocw() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let identifier = TEST_IDENTIFIER;
			// 1. Create collection with onboarding size 1 (Flexible mode is default in
			//    create_test_collection)
			create_test_collection(identifier, 1);

			// 2. Add 1 member
			let members = generate_members(identifier, 1, 1);
			let member = members[0].0;

			// Onboard to Ring 0
			assert_ok!(MembersPallet::onboard_members(&identifier, false));
			// Build Ring 0
			let to_include = MembersPallet::should_build_ring(&identifier, 0, 1).unwrap();
			assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));

			// Verify member is in Ring 0
			let position = Members::<Test>::get(identifier, member).unwrap();
			assert!(matches!(position, RingPosition::Included { ring_index: 0, .. }));

			// 3. Manually advance to Ring 1 (simulating Ring 0 full)
			// We use the helper function available in this module
			CurrentRingIndex::<Test>::insert(identifier, 1);

			// 4. Start mutation session, suspend the key from Ring 0
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(&identifier, &[member]));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Verify member is Suspended (and key is technically still in Ring 0 keys as cleanup
			// didn't run)
			let position = Members::<Test>::get(identifier, member).unwrap();
			assert!(matches!(position, RingPosition::Suspended));

			// 5. Resume the key
			assert_ok!(MembersPallet::add_members(&identifier, vec![member]));

			// 6. Onboard again. Since current ring is 1, it goes to Ring 1.
			assert_ok!(MembersPallet::onboard_members(&identifier, true));

			// The MembersOnboarded event is emitted.
			System::assert_has_event(Event::<Test>::MembersOnboarded { identifier }.into());

			// Verify member is now in Ring 1
			let position = Members::<Test>::get(identifier, member).unwrap();
			assert!(matches!(position, RingPosition::Included { ring_index: 1, .. }));

			// Verify key is in Ring 1 RingKeys
			let ring1_keys = RingKeys::<Test>::get((&identifier, 1u32, 0u32));
			assert!(ring1_keys.contains(&member));

			// Verify key is STILL in Ring 0 RingKeys (stale reference)
			let ring0_keys = RingKeys::<Test>::get((&identifier, 0u32, 0u32));
			assert!(ring0_keys.contains(&member));

			// 7. Directly delete Ring 0
			assert_ok!(MembersPallet::remove_ring(&identifier, 0));

			// 8. Process deletion queue
			advance_to_block(10);

			// 9. The key is in Ring 1 in RingKeys...
			let ring1_keys_after = RingKeys::<Test>::get((&identifier, 1u32, 0u32));
			assert!(ring1_keys_after.contains(&member));

			// ...and the member position points to the new ring
			assert_eq!(
				Members::<Test>::get(identifier, member).unwrap(),
				RingPosition::Included { ring_index: 1, ring_page: 0, ring_position: 0 },
				"Member was deleted despite being active in Ring 1"
			);
		});
	}
}

mod collection_deletion_tests {
	use super::*;

	const DELETION_IDENTIFIER: Identifier = [100u8; 32];
	const DELETION_IDENTIFIER_2: Identifier = [101u8; 32];

	/// Helper to create a collection owned by a specific location.
	fn create_owned_collection(identifier: Identifier, owner: MockLocation, onboarding_size: u32) {
		<MembersPallet as AppendOnlyMembers>::create_collection(
			owner,
			&identifier,
			onboarding_size,
			RingMode::Flexible,
			indiv_support::traits::RingExponent::R2e9,
			None,
		)
		.expect("Failed to create collection");
	}

	/// Helper to add members and optionally onboard/build ring.
	fn setup_collection_with_members(
		identifier: Identifier,
		member_count: u8,
		build_ring: bool,
	) -> Vec<MemberOf<Test>> {
		let mut members = Vec::new();
		for i in 1..=member_count {
			let secret = MockCrypto::new_secret([i; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			members.push(member);
		}
		assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(&identifier, members.clone()));

		if build_ring {
			assert_ok!(MembersPallet::onboard_members(&identifier, false));
			if let Some(to_include) =
				MembersPallet::should_build_ring(&identifier, 0, member_count as u32)
			{
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}
		}

		members
	}

	// =========================================================================
	// delete_collection function tests
	// =========================================================================

	#[test]
	fn delete_collection_fails_for_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::delete_collection(
					owner,
					&NONEXISTENT_IDENTIFIER
				),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn delete_collection_fails_if_not_owner() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			let not_owner = MockLocation(2);

			create_owned_collection(DELETION_IDENTIFIER, owner, 5);

			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::delete_collection(
					not_owner,
					&DELETION_IDENTIFIER
				),
				Error::<Test>::NotCollectionOwner
			);
		});
	}

	#[test]
	fn delete_collection_fails_if_already_suspended() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// First delete succeeds - moves from Collections to SuspendedCollections
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER
			));

			// Second delete fails with CollectionNotFound because it's no longer in Collections
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::delete_collection(
					owner,
					&DELETION_IDENTIFIER
				),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn delete_collection_succeeds() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Verify collection exists in Collections
			assert!(Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// The CollectionMarkedForDeletion event is emitted.
			System::assert_has_event(
				Event::<Test>::CollectionMarkedForDeletion { identifier: DELETION_IDENTIFIER }
					.into(),
			);

			// Verify moved to SuspendedCollections
			assert!(!Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
		});
	}

	#[test]
	fn suspended_collection_blocks_new_collection_with_same_id() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Delete collection (moves to suspended)
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER
			));

			// Try to create new collection with same identifier
			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::create_collection(
					owner,
					&DELETION_IDENTIFIER,
					5,
					RingMode::Flexible,
					indiv_support::traits::RingExponent::R2e9,
					None,
				),
				Error::<Test>::CollectionAlreadyExists
			);
		});
	}

	#[test]
	fn add_members_fails_for_suspended_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Try to add members
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			assert_noop!(
				<MembersPallet as AppendOnlyMembers>::add_members(
					&DELETION_IDENTIFIER,
					vec![member]
				),
				Error::<Test>::CollectionNotFound
			);
		});
	}

	#[test]
	fn verify_membership_fails_for_suspended_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members and build ring
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Verify ring exists
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Try to verify membership (should fail because collection not in Collections)
			let dummy_proof = verifiable::mock::MockProof::default();
			let context = [0u8; 32];
			let result = <MembersPallet as MembershipProver>::verify_membership(
				&DELETION_IDENTIFIER,
				&dummy_proof,
				0,
				context,
				b"test",
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::CollectionNotFound.into());
		});
	}

	// =========================================================================
	// Deletion processing stage tests
	// =========================================================================

	#[test]
	fn enqueue_ring_for_deletion_removes_ring_metadata() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members and build ring
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Verify ring metadata exists
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total > 0);

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Enqueue ring for deletion directly
			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);

			// Ring metadata should be removed
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(!StaleRings::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(!PendingSuspensions::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert_eq!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total, 0);

			// RingDeletionQueue should have entries for the ring pages
			assert!(RingDeletionQueue::<Test>::contains_key((DELETION_IDENTIFIER, 0, 0)));
		});
	}

	#[test]
	fn enqueue_ring_for_deletion_with_empty_ring() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Ring was never built.
			assert_eq!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total, 0);
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Enqueue the empty ring.
			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);

			// No pages should be enqueued (ring was empty → page_count == 0).
			assert!(RingDeletionQueue::<Test>::iter_prefix((DELETION_IDENTIFIER,))
				.next()
				.is_none());

			// Ring metadata should still be cleaned up.
			assert_eq!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total, 0);
		});
	}

	#[test]
	fn deleting_onboarding_queue_page_removes_members() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members but DON'T build ring (leave in onboarding queue)
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 10, false);

			// Verify members are in onboarding queue
			for member in &members {
				let position =
					Members::<Test>::get(DELETION_IDENTIFIER, member).expect("Member should exist");
				assert!(matches!(position, RingPosition::Onboarding { .. }));
			}

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Delete onboarding queue pages directly
			let page = OnboardingQueue::<Test>::take(DELETION_IDENTIFIER, 0u32);
			for member in page.iter() {
				Members::<Test>::remove(DELETION_IDENTIFIER, member);
			}

			// Members should be deleted
			for member in &members {
				assert!(
					Members::<Test>::get(DELETION_IDENTIFIER, member).is_none(),
					"Member should be deleted after collection deletion"
				);
			}
		});
	}

	#[test]
	fn finalizing_stage_removes_all_remaining_storage() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Delete and fully process
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process all stages via OCW
			advance_to_block(10);

			// All storage should be removed
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!CurrentRingIndex::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!OnboardingSize::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!ActiveMembers::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!RingsState::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!QueuePageIndices::<Test>::contains_key(DELETION_IDENTIFIER));
		});
	}

	#[test]
	fn finalizing_stage_removes_from_identifiers_of() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);

			// Create two collections for the same owner
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			create_owned_collection(DELETION_IDENTIFIER_2, owner.clone(), 5);

			// Verify owner has both identifiers
			let owner_key = CollectionOwner::External(owner.clone());
			let identifiers = IdentifiersOf::<Test>::get(&owner_key).unwrap();
			assert!(identifiers.contains(&DELETION_IDENTIFIER));
			assert!(identifiers.contains(&DELETION_IDENTIFIER_2));

			// Delete only the first collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER
			));

			// Process until completion via OCW
			advance_to_block(10);

			// First identifier should be removed, second should remain
			let identifiers = IdentifiersOf::<Test>::get(&owner_key).unwrap();
			assert!(!identifiers.contains(&DELETION_IDENTIFIER));
			assert!(identifiers.contains(&DELETION_IDENTIFIER_2));
		});
	}

	// =========================================================================
	// End-to-end collection deletion tests using OCW
	// =========================================================================

	#[test]
	fn delete_empty_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Delete empty collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// Collection should be fully deleted
			assert!(!Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
		});
	}

	#[test]
	fn delete_collection_with_only_onboarding_queue() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members but don't onboard
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 15, false);

			// Verify onboarding queue has members
			assert!(!OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// Everything should be cleaned up
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());

			// Members should be deleted
			for member in &members {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
		});
	}

	#[test]
	fn delete_collection_with_only_built_ring() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members and build ring (onboarding queue should be empty after)
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Verify ring is built
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// The CollectionDeleted event is emitted.
			System::assert_has_event(
				Event::<Test>::CollectionDeleted { identifier: DELETION_IDENTIFIER }.into(),
			);

			// Everything should be cleaned up
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingKeys::<Test>::get((&DELETION_IDENTIFIER, 0u32, 0u32)).is_empty());

			// Members should be deleted
			for member in &members {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
		});
	}

	#[test]
	fn delete_collection_with_rings_and_queue() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// First batch: add and build ring
			let ring_members = setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Second batch: add but don't build (stays in queue)
			let mut queue_members = Vec::new();
			for i in 50..60u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				queue_members.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER,
				queue_members.clone()
			));

			// Verify both ring and queue have members
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(!OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// Everything should be cleaned up
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingKeys::<Test>::get((&DELETION_IDENTIFIER, 0u32, 0u32)).is_empty());
			assert!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());

			// All members should be deleted
			for member in ring_members.iter().chain(queue_members.iter()) {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
		});
	}

	#[test]
	fn full_deletion_flow_end_to_end() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 20, true);

			// Verify everything exists
			assert!(Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingsState::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(OnboardingSize::<Test>::contains_key(DELETION_IDENTIFIER));

			let owner_key = CollectionOwner::External(owner.clone());
			assert!(IdentifiersOf::<Test>::get(&owner_key).unwrap().contains(&DELETION_IDENTIFIER));

			// Delete collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// Verify complete cleanup
			assert!(!Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(!RingsState::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!OnboardingSize::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!ActiveMembers::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!CurrentRingIndex::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!QueuePageIndices::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(RingKeys::<Test>::get((&DELETION_IDENTIFIER, 0u32, 0u32)).is_empty());
			assert!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());
			assert!(!RingDeletionQueue::<Test>::contains_key((DELETION_IDENTIFIER, 0, 0)));

			// Owner's identifier list should not contain the deleted collection
			assert!(!IdentifiersOf::<Test>::get(&owner_key)
				.unwrap_or_default()
				.contains(&DELETION_IDENTIFIER));

			// All members should be deleted
			for member in &members {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
		});
	}

	#[test]
	fn enqueue_ring_deletion_authorized_removes_ring_metadata() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Verify ring exists before deletion.
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total > 0);

			// Delete collection (moves to SuspendedCollections).
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Run one OCW cycle — should submit enqueue_ring_deletion_authorized.
			advance_to_block(2);

			// Ring metadata should be removed and pages enqueued.
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert_eq!(RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0).total, 0);
			assert!(RingDeletionQueue::<Test>::iter_prefix((DELETION_IDENTIFIER,))
				.next()
				.is_some());
		});
	}

	#[test]
	fn delete_onboarding_queue_page_authorized_removes_members() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members without building ring (only queue pages).
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 5, false);
			assert!(!OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Run OCW cycles — should submit delete_onboarding_queue_page_authorized
			// and finalize_collection_deletion_authorized.
			advance_to_block(10);

			// Queue pages and members should be removed.
			assert!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());
			for member in &members {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
			// Collection should be fully finalized.
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
		});
	}

	#[test]
	fn finalize_collection_deletion_authorized_cleans_up_storage() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			let owner_key = CollectionOwner::External(owner.clone());
			assert!(IdentifiersOf::<Test>::get(&owner_key).unwrap().contains(&DELETION_IDENTIFIER));

			// Empty collection — finalization should proceed immediately.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			advance_to_block(10);

			// All per-collection storage should be gone.
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!CurrentRingIndex::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!OnboardingSize::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!ActiveMembers::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!RingsState::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!QueuePageIndices::<Test>::contains_key(DELETION_IDENTIFIER));

			// Owner's identifier list should no longer contain the deleted collection.
			assert!(!IdentifiersOf::<Test>::get(&owner_key)
				.unwrap_or_default()
				.contains(&DELETION_IDENTIFIER));
		});
	}

	#[test]
	fn full_deletion_with_rings_and_queue_end_to_end() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Set up both ring and queue members.
			let ring_members = setup_collection_with_members(DELETION_IDENTIFIER, 10, true);
			let mut queue_members = Vec::new();
			for i in 50..55u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				queue_members.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER,
				queue_members.clone()
			));

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process all stages via OCW.
			advance_to_block(10);

			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			assert!(RingKeys::<Test>::get((&DELETION_IDENTIFIER, 0u32, 0u32)).is_empty());
			assert!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).is_empty());
			for member in ring_members.iter().chain(queue_members.iter()) {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER, member).is_none());
			}
		});
	}

	// =========================================================================
	// Authorization validation tests
	// =========================================================================

	// --- ensure_can_enqueue_ring_deletion ---

	#[test]
	fn ensure_can_enqueue_ring_deletion_rejects_non_suspended_collection() {
		TestExt::new().execute_with(|| {
			create_owned_collection(DELETION_IDENTIFIER, MockLocation(1), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Collection is active, not suspended.
			assert!(
				MembersPallet::ensure_can_enqueue_ring_deletion(&DELETION_IDENTIFIER, 0).is_err()
			);
		});
	}

	#[test]
	fn ensure_can_enqueue_ring_deletion_rejects_missing_ring() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// No rings exist (empty collection).
			assert!(
				MembersPallet::ensure_can_enqueue_ring_deletion(&DELETION_IDENTIFIER, 0).is_err()
			);
		});
	}

	#[test]
	fn ensure_can_enqueue_ring_deletion_rejects_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			assert!(MembersPallet::ensure_can_enqueue_ring_deletion(&NONEXISTENT_IDENTIFIER, 0)
				.is_err());
		});
	}

	#[test]
	fn ensure_can_enqueue_ring_deletion_accepts_valid_state() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			assert!(
				MembersPallet::ensure_can_enqueue_ring_deletion(&DELETION_IDENTIFIER, 0).is_ok()
			);
		});
	}

	#[test]
	fn ensure_can_enqueue_ring_deletion_rejects_already_enqueued_ring() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Enqueue ring 0.
			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);

			// Ring 0's RingKeysStatus has been removed, so a second attempt is rejected.
			assert!(
				MembersPallet::ensure_can_enqueue_ring_deletion(&DELETION_IDENTIFIER, 0).is_err()
			);
		});
	}

	// --- ensure_can_delete_onboarding_queue_page ---

	#[test]
	fn ensure_can_delete_onboarding_queue_page_rejects_non_suspended_collection() {
		TestExt::new().execute_with(|| {
			create_owned_collection(DELETION_IDENTIFIER, MockLocation(1), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 5, false);

			assert!(MembersPallet::ensure_can_delete_onboarding_queue_page(
				&DELETION_IDENTIFIER,
				0
			)
			.is_err());
		});
	}

	#[test]
	fn ensure_can_delete_onboarding_queue_page_rejects_when_rings_remain() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// RingKeysStatus still has entries.
			assert!(MembersPallet::ensure_can_delete_onboarding_queue_page(
				&DELETION_IDENTIFIER,
				0
			)
			.is_err());
		});
	}

	#[test]
	fn ensure_can_delete_onboarding_queue_page_rejects_when_ring_pages_pending() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			// Add members to both ring and queue.
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Enqueue ring pages (clears RingKeysStatus but populates RingDeletionQueue).
			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);
			assert!(RingDeletionQueue::<Test>::iter_prefix((DELETION_IDENTIFIER,))
				.next()
				.is_some());

			assert!(MembersPallet::ensure_can_delete_onboarding_queue_page(
				&DELETION_IDENTIFIER,
				0
			)
			.is_err());
		});
	}

	#[test]
	fn ensure_can_delete_onboarding_queue_page_rejects_nonexistent_page() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Empty collection — no onboarding queue pages.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			assert!(MembersPallet::ensure_can_delete_onboarding_queue_page(
				&DELETION_IDENTIFIER,
				0
			)
			.is_err());
		});
	}

	#[test]
	fn ensure_can_delete_onboarding_queue_page_accepts_valid_state() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			// Add members without building a ring (only queue pages exist).
			setup_collection_with_members(DELETION_IDENTIFIER, 5, false);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// No rings, no ring pages, queue page exists.
			assert!(MembersPallet::ensure_can_delete_onboarding_queue_page(
				&DELETION_IDENTIFIER,
				0
			)
			.is_ok());
		});
	}

	// --- ensure_can_finalize_collection_deletion ---

	#[test]
	fn ensure_can_finalize_rejects_non_suspended_collection() {
		TestExt::new().execute_with(|| {
			create_owned_collection(DELETION_IDENTIFIER, MockLocation(1), 5);

			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_err());
		});
	}

	#[test]
	fn ensure_can_finalize_rejects_when_rings_remain() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_err());
		});
	}

	#[test]
	fn ensure_can_finalize_rejects_when_ring_pages_pending() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);

			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_err());
		});
	}

	#[test]
	fn ensure_can_finalize_rejects_when_onboarding_queue_remains() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			// Only queue pages, no built rings.
			setup_collection_with_members(DELETION_IDENTIFIER, 5, false);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// No rings or ring pages, but queue pages remain.
			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_err());
		});
	}

	#[test]
	fn ensure_can_finalize_rejects_nonexistent_collection() {
		TestExt::new().execute_with(|| {
			assert!(MembersPallet::ensure_can_finalize_collection_deletion(
				&NONEXISTENT_IDENTIFIER
			)
			.is_err());
		});
	}

	#[test]
	fn ensure_can_finalize_accepts_empty_suspended_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Empty collection — nothing to clean up.
			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_ok());
		});
	}

	#[test]
	fn ensure_can_finalize_accepts_after_full_cleanup() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Enqueue and process ring pages.
			MembersPallet::enqueue_ring_for_deletion(&DELETION_IDENTIFIER, 0);
			let pages: Vec<_> = RingDeletionQueue::<Test>::iter_keys().collect();
			for (identifier, ring_index, page_index) in pages {
				RingDeletionQueue::<Test>::remove((&identifier, ring_index, page_index));
				MembersPallet::delete_ring_page(identifier, ring_index, page_index);
			}

			assert!(MembersPallet::ensure_can_finalize_collection_deletion(&DELETION_IDENTIFIER)
				.is_ok());
		});
	}

	// =========================================================================
	// Multiple collections tests
	// =========================================================================

	#[test]
	fn multiple_suspended_collections_deleted_concurrently() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);

			// Create two collections
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			create_owned_collection(DELETION_IDENTIFIER_2, owner.clone(), 5);

			// Suspend both
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER
			));
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER_2
			));

			// Both should be in SuspendedCollections
			assert!(SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER_2));

			// Process collection deletion
			advance_to_block(10);

			// Both should be fully deleted
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER_2));
		});
	}

	#[test]
	fn active_collections_unaffected_by_deletion_of_another() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);

			// Create two collections
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);
			create_owned_collection(DELETION_IDENTIFIER_2, owner.clone(), 5);

			// Add members to first collection
			let _members1 = setup_collection_with_members(DELETION_IDENTIFIER, 10, true);

			// Add DIFFERENT members to second collection (use different seeds)
			let mut members2 = Vec::new();
			for i in 50..60u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				members2.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER_2,
				members2.clone()
			));
			assert_ok!(MembersPallet::onboard_members(&DELETION_IDENTIFIER_2, false));
			if let Some(to_include) =
				MembersPallet::should_build_ring(&DELETION_IDENTIFIER_2, 0, 10)
			{
				assert_ok!(MembersPallet::build_ring(&DELETION_IDENTIFIER_2, 0, to_include));
			}

			// Delete only the first
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner.clone(),
				&DELETION_IDENTIFIER
			));

			// Process collection deletion
			advance_to_block(10);

			// First collection should be deleted
			assert!(!Collections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));

			// Second collection should still be active
			assert!(Collections::<Test>::contains_key(DELETION_IDENTIFIER_2));
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER_2, 0));

			// Second collection's members should still exist
			for member in &members2 {
				assert!(Members::<Test>::get(DELETION_IDENTIFIER_2, member).is_some());
			}
		});
	}

	#[test]
	fn ocw_skips_suspended_collections_for_normal_operations() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members but don't build ring yet
			let mut members = Vec::new();
			for i in 1..=10u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				members.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER,
				members.clone()
			));

			// Suspend the collection BEFORE onboarding/building
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// OCW should NOT try to onboard or build rings for suspended collection
			// It should only process deletion
			advance_to_block(10);

			// Collection should be deleted, not have rings built
			assert!(!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER));
			assert!(!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
		});
	}

	#[test]
	fn member_deletion_does_not_cross_collections() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 1);
			create_owned_collection(DELETION_IDENTIFIER_2, owner.clone(), 1);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);

			// Add member to both collections.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER,
				vec![member]
			));
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER_2,
				vec![member]
			));

			// Delete the first collection.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Process collection deletion.
			advance_to_block(10);

			// Member should still exist in the second collection.
			assert!(Members::<Test>::get(DELETION_IDENTIFIER_2, member).is_some());
		});
	}

	#[test]
	fn empty_onboarding_queue_page_does_not_deadlock_deletion() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add exactly one member to the onboarding queue (don't build ring).
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 1, false);
			assert_eq!(OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32).len(), 1);

			// Start a removal session and suspend the only member in the queue.
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(
				&DELETION_IDENTIFIER
			));
			assert_ok!(MembersPallet::queue_member_suspensions(&DELETION_IDENTIFIER, &members));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(
				&DELETION_IDENTIFIER
			));

			// The queue page now exists but is EMPTY.
			let page = OnboardingQueue::<Test>::get(DELETION_IDENTIFIER, 0u32);
			assert!(page.is_empty(), "Page should be empty after suspending all members");
			assert!(
				OnboardingQueue::<Test>::iter_prefix(DELETION_IDENTIFIER).next().is_some(),
				"Empty page key should still exist in storage"
			);

			// Now delete the collection.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));

			// Run OCW cycles — deletion should complete.
			advance_to_block(10);

			// Collection must be fully deleted, not stuck.
			assert!(
				!SuspendedCollections::<Test>::contains_key(DELETION_IDENTIFIER),
				"Collection should not be stuck in SuspendedCollections"
			);
			assert!(
				OnboardingQueue::<Test>::iter_prefix(DELETION_IDENTIFIER).next().is_none(),
				"Onboarding queue should be fully cleaned up"
			);
		});
	}

	#[test]
	fn no_ring_root_change_emission_when_root_does_not_exist() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members and onboard them, but DON'T build the ring.
			let mut members = Vec::new();
			for i in 1..=10u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				members.push(member);
			}
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&DELETION_IDENTIFIER,
				members.clone()
			));
			assert_ok!(MembersPallet::onboard_members(&DELETION_IDENTIFIER, false));

			// Verify: ring has members but NO root.
			let ring_status = RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0);
			assert!(ring_status.total > 0, "Ring should have members");
			assert!(
				!Root::<Test>::contains_key(DELETION_IDENTIFIER, 0),
				"Root should NOT exist yet"
			);

			// Delete the collection and run OCW to trigger enqueue_ring_deletion_authorized.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&DELETION_IDENTIFIER
			));
			advance_to_block(2);

			// No OldRoots entry should have been created since there was no root to archive.
			assert!(
				OldRoots::<Test>::iter_prefix((DELETION_IDENTIFIER, 0u32)).next().is_none(),
				"No root should have been archived"
			);
		});
	}

	#[test]
	fn no_ring_keys_empty_page_leak_after_full_suspension() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let owner = MockLocation(1);
			create_owned_collection(DELETION_IDENTIFIER, owner.clone(), 5);

			// Add members and build the ring.
			let members = setup_collection_with_members(DELETION_IDENTIFIER, 5, true);

			// Verify ring is built.
			assert!(Root::<Test>::contains_key(DELETION_IDENTIFIER, 0));
			let ring_status = RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0);
			assert_eq!(ring_status.total, 5);

			// Suspend ALL members in the ring.
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(
				&DELETION_IDENTIFIER
			));
			assert_ok!(MembersPallet::queue_member_suspensions(&DELETION_IDENTIFIER, &members));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(
				&DELETION_IDENTIFIER
			));

			// Process the suspensions via OCW.
			advance_to_block(10);

			// RingKeysStatus.total should be 0.
			let ring_status = RingKeysStatus::<Test>::get(DELETION_IDENTIFIER, 0);
			assert_eq!(ring_status.total, 0, "All members were suspended");

			// RingKeys page must have been removed, not left as an empty entry.
			assert!(
				RingKeys::<Test>::iter_prefix((&DELETION_IDENTIFIER,)).next().is_none(),
				"RingKeys page should not exist after all members were suspended"
			);
		});
	}
}

mod end_to_end_tests {
	use super::*;

	const E2E_IDENTIFIER: Identifier = [42u8; 32];

	/// Helper: create collection, add members, onboard all, build ring.
	/// Returns (member, secret) pairs.
	fn setup_and_build(
		identifier: Identifier,
		onboarding_size: u32,
		member_count: u8,
	) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
		create_test_collection(identifier, onboarding_size);
		let members = generate_members(identifier, 1, member_count);

		// Drain onboarding queue
		while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

		// Build ring 0, one call to `build_ring` should be enough in all cases
		loop {
			let maybe = MembersPallet::should_build_ring(&identifier, 0, member_count as u32);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			} else {
				break;
			}
		}

		members
	}

	#[test]
	fn append_only_collection_lifecycle() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create AppendOnly collection with onboarding size 5
			create_append_only_collection(identifier, 5);

			// Add 10 members
			let members = generate_members(identifier, 1, 10);

			// Verify all are in onboarding state
			for (member, _) in &members {
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(status.is_some());
				assert!(matches!(status.unwrap(), RingPosition::Onboarding { .. }));
			}

			// Onboard members
			assert_ok!(MembersPallet::onboard_members(&identifier, false));

			// Build ring
			let ring_index = 0;
			let maybe_to_include = MembersPallet::should_build_ring(&identifier, ring_index, 10);
			assert!(maybe_to_include.is_some());
			let to_include = maybe_to_include.unwrap();
			assert_ok!(MembersPallet::build_ring(&identifier, ring_index, to_include));

			// Verify members are included
			for (member, _) in &members {
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(status.is_some());
				assert!(
					matches!(status.unwrap(), RingPosition::Included { .. }),
					"Member should be included after ring build"
				);
			}

			// Verify active_count = 10
			assert_eq!(<MembersPallet as AppendOnlyMembers>::active_count(&identifier), 10);

			// Verify ring_status shows members
			let ring_status =
				<MembersPallet as AppendOnlyMembers>::ring_status(&identifier, ring_index);
			assert!(ring_status.is_some());
			let ring_status = ring_status.unwrap();
			assert_eq!(ring_status.total, 10);
			assert_eq!(ring_status.included, 10);

			// Create a proof and verify membership
			let (member, secret) = &members[0];
			assert!(Root::<Test>::contains_key(identifier, ring_index));
			let ring_members =
				<MembersPallet as AppendOnlyMembers>::ring_members(&identifier, ring_index);

			// Create commitment
			let commitment = MockCrypto::open((), member, ring_members.into_iter()).unwrap();

			let context = [1u8; 32];
			let message = b"test message";

			// Create proof
			let (proof, _alias) =
				MockCrypto::create(commitment, secret, &context, message).unwrap();

			// Verify membership through the pallet
			let result = <MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof,
				ring_index,
				context,
				message,
			);
			assert!(result.is_ok());
			let revised_alias = result.unwrap();
			assert_eq!(revised_alias.ring, ring_index);
			assert_eq!(revised_alias.ca.context, context);
		});
	}

	#[test]
	fn flexible_collection_member_removal() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create Flexible collection, add 10 members, onboard, build ring 0
			let members = setup_and_build(identifier, 5, 10);

			// Verify all included, active count should be 10
			for (member, _) in &members {
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(matches!(status.unwrap(), RingPosition::Included { .. }));
			}
			assert_eq!(<MembersPallet as AppendOnlyMembers>::active_count(&identifier), 10);

			// Start removal session
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			assert!(RingsState::<Test>::get(identifier).mutating());

			// Remove 2 members
			let members_to_remove: Vec<_> = members.iter().take(2).map(|(m, _)| *m).collect();
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&identifier,
				&members_to_remove
			));

			// Verify suspended
			for member in &members_to_remove {
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(matches!(status.unwrap(), RingPosition::Suspended));
			}

			// End removal session
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));
			assert!(RingsState::<Test>::get(identifier).append_only());

			// Verify PendingSuspensions has entries for ring 0
			assert!(PendingSuspensions::<Test>::contains_key(identifier, 0));

			// Remove suspended keys
			MembersPallet::remove_suspended_keys(&identifier, 0);

			// Verify ring is stale, root cleared (empty)
			assert!(StaleRings::<Test>::contains_key(identifier, 0));
			let root_after = Root::<Test>::get(identifier, 0).unwrap();
			assert!(root_after.root.is_empty());

			// Rebuild ring
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			assert!(maybe.is_some());
			assert_ok!(MembersPallet::build_ring(&identifier, 0, maybe.unwrap()));

			// Verify new root is non-empty (for these tests we can actually look inside the root
			// but with Bandersnatch cryptography we wouldn't know unless we tried to verify a proof
			// against the new root)
			let root_rebuilt = Root::<Test>::get(identifier, 0).unwrap();
			assert!(!root_rebuilt.root.is_empty());

			// Verify remaining 8 members still included
			for (member, _) in members.iter().skip(2) {
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(
					matches!(status.unwrap(), RingPosition::Included { .. }),
					"Remaining member should still be included"
				);
			}

			// Verify that active count has decreased
			assert_eq!(<MembersPallet as AppendOnlyMembers>::active_count(&identifier), 8);

			// Verify removed members' proofs no longer work against new root
			let (removed_member, _removed_secret) = &members[0];
			let new_ring_members =
				<MembersPallet as AppendOnlyMembers>::ring_members(&identifier, 0);

			// The removed member is no longer in the ring members, so open should fail
			let open_result = MockCrypto::open((), removed_member, new_ring_members.into_iter());
			assert!(
				open_result.is_err(),
				"Removed member should not be able to open a commitment against the rebuilt ring"
			);
		});
	}

	#[test]
	fn onboarding_queue_privacy_protection() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create collection with onboarding size 5
			create_test_collection(identifier, 5);

			// Add 3 members (below threshold)
			let _first_batch = generate_members(identifier, 1, 3);

			// Onboarding should return `Ok(false)` because there aren't enough members to onboard
			// to meet the onboarding size requirement
			assert_eq!(MembersPallet::onboard_members(&identifier, false), Ok(false));

			// Verify members still in onboarding
			for i in 1..=3u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, &member);
				assert!(matches!(status.unwrap(), RingPosition::Onboarding { .. }));
			}

			// Add 2 more (total 5, meets threshold)
			let _second_batch = generate_members(identifier, 4, 5);

			// Onboarding should now succeed
			let result = MembersPallet::onboard_members(&identifier, false);
			assert_eq!(result, Ok(true));

			// Verify all 5 onboarded into ring keys
			let ring_keys = RingKeys::<Test>::get((&identifier, 0u32, 0u32));
			assert_eq!(ring_keys.len(), 5);

			// Build ring, verify included
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 5);
			assert!(maybe.is_some());
			assert_ok!(MembersPallet::build_ring(&identifier, 0, maybe.unwrap()));

			for i in 1..=5u8 {
				let secret = MockCrypto::new_secret([i; 32]);
				let member = MockCrypto::member_from_secret(&secret);
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, &member);
				assert!(matches!(status.unwrap(), RingPosition::Included { .. }));
			}
		});
	}

	#[test]
	fn ring_lifecycle_with_multiple_rings() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create Flexible collection with onboarding size 1
			create_test_collection(identifier, 1);

			// Generate and add 255 members to fill ring 0
			let ring0_members = generate_members(identifier, 1, 255);

			// Drain onboarding queue
			while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

			// Build ring 0
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
				} else {
					break;
				}
			}

			// Verify CurrentRingIndex advanced to 1 (ring 0 full)
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 1);

			// Generate 10 more members using a different seed pattern
			let _ring1_members = generate_members_with_offset(identifier, 1, 10, 0xBB);

			// Onboard into ring 1
			while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

			// Build ring 1
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 1, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 1, to_include));
				} else {
					break;
				}
			}

			// Advance to ring 2 so neither 0 nor 1 is current
			manually_advance_to_ring(&identifier, 2);

			// Verify ring status for both rings
			let status0 =
				<MembersPallet as AppendOnlyMembers>::ring_status(&identifier, 0).unwrap();
			assert_eq!(status0.total, 255);
			let status1 =
				<MembersPallet as AppendOnlyMembers>::ring_status(&identifier, 1).unwrap();
			assert_eq!(status1.total, 10);

			// Remove 130 members from ring 0
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			let to_remove: Vec<_> = ring0_members.iter().take(130).map(|(m, _)| *m).collect();
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(&identifier, &to_remove));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Clean ring 0
			MembersPallet::remove_suspended_keys(&identifier, 0);

			// Ring 0 now has 125 members, ring 1 has 10 — both below merge threshold, which is less
			// than half of the max ring size 255, so 127
			let status0 =
				<MembersPallet as AppendOnlyMembers>::ring_status(&identifier, 0).unwrap();
			assert_eq!(status0.total, 125);

			// Build ring 0 to clear stale status
			loop {
				let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
				if let Some(to_include) = maybe {
					assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
				} else {
					break;
				}
			}

			// Merge rings 0 and 1
			assert_ok!(MembersPallet::merge_rings(RuntimeOrigin::signed(1), identifier, 0, 1));

			// Verify merged ring 0 has combined members (125 + 10 = 135)
			let merged_status =
				<MembersPallet as AppendOnlyMembers>::ring_status(&identifier, 0).unwrap();
			assert_eq!(merged_status.total, 135);

			// Ring 1 should be cleared
			assert!(!Root::<Test>::contains_key(identifier, 1));
			assert_eq!(RingKeysStatus::<Test>::get(identifier, 1).total, 0);

			// Now remove ring 0 (top ring index is 2 after manually advancing)
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&identifier, 0));

			// `RingDeletionQueue` should be populated
			assert!(RingDeletionQueue::<Test>::contains_key((identifier, 0, 0)));

			// Process deletion queue
			advance_to_block(10);

			// Verify ring keys cleaned up
			assert!(RingKeys::<Test>::get((&identifier, 0u32, 0u32)).is_empty());
			assert!(!RingDeletionQueue::<Test>::contains_key((identifier, 0, 0)));
		});
	}

	#[test]
	fn anonymous_authentication_with_contextual_aliases() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create collection, add members, onboard, build ring
			let members = setup_and_build(identifier, 5, 10);

			let ring_index = 0;
			let ring_members =
				<MembersPallet as AppendOnlyMembers>::ring_members(&identifier, ring_index);

			// Pick a member
			let (member, secret) = &members[0];

			let context_a = [0xAA; 32];
			let context_b = [0xBB; 32];
			let message = b"auth message";

			// Create proof in context_a
			let commitment_a = MockCrypto::open((), member, ring_members.iter().cloned()).unwrap();
			let (proof_a, _) =
				MockCrypto::create(commitment_a, secret, &context_a, message).unwrap();

			// Verify in context_a
			let result_a = <MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof_a,
				ring_index,
				context_a,
				message,
			);
			assert!(result_a.is_ok());
			let alias_a = result_a.unwrap();

			// Create proof in context_b
			let commitment_b = MockCrypto::open((), member, ring_members.iter().cloned()).unwrap();
			let (proof_b, _) =
				MockCrypto::create(commitment_b, secret, &context_b, message).unwrap();

			// Verify in context_b
			let result_b = <MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof_b,
				ring_index,
				context_b,
				message,
			);
			assert!(result_b.is_ok());
			let alias_b = result_b.unwrap();

			// Aliases should differ across contexts (different context → different alias)
			// Note: With the Simple mock crypto, the alias is always the public key
			// regardless of context, so this test verifies the structural flow. In production
			// with real Bandersnatch ring-VRF, these would be unlinkable.
			assert_eq!(alias_a.ca.context, context_a);
			assert_eq!(alias_b.ca.context, context_b);
			assert_ne!(alias_a.ca.context, alias_b.ca.context);

			// Create another proof in context_a — should yield same alias
			let commitment_a2 = MockCrypto::open((), member, ring_members.iter().cloned()).unwrap();
			let (proof_a2, _) =
				MockCrypto::create(commitment_a2, secret, &context_a, message).unwrap();

			let result_a2 = <MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof_a2,
				ring_index,
				context_a,
				message,
			);
			assert!(result_a2.is_ok());
			let alias_a2 = result_a2.unwrap();

			// Same context → same alias (deterministic)
			assert_eq!(alias_a.ca.alias, alias_a2.ca.alias);
		});
	}

	#[test]
	fn flexible_collection_removal_session_rebuilds_ring() {
		TestExt::new().execute_with(|| {
			let identifier = E2E_IDENTIFIER;

			// Create Flexible collection, add 10 members, onboard, build ring
			let members = setup_and_build(identifier, 5, 10);

			let ring_index = 0;

			// Record root before removal
			let root_before = Root::<Test>::get(identifier, ring_index).unwrap();
			let revision_before = root_before.revision;
			assert!(!root_before.root.is_empty());

			// Start removal session, suspend 2 members, end session
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			let to_remove: Vec<_> = members.iter().take(2).map(|(m, _)| *m).collect();
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(&identifier, &to_remove));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Remove suspended keys
			MembersPallet::remove_suspended_keys(&identifier, ring_index);

			// Verify ring stale, root cleared
			assert!(StaleRings::<Test>::contains_key(identifier, ring_index));
			let root_cleared = Root::<Test>::get(identifier, ring_index).unwrap();
			assert!(root_cleared.root.is_empty());

			// Rebuild ring
			let maybe = MembersPallet::should_build_ring(&identifier, ring_index, 255);
			assert!(maybe.is_some());
			assert_ok!(MembersPallet::build_ring(&identifier, ring_index, maybe.unwrap()));

			// Verify new root differs from original
			let root_after = Root::<Test>::get(identifier, ring_index).unwrap();
			assert!(!root_after.root.is_empty());
			assert_ne!(root_before.root, root_after.root);

			// Verify ring_revision incremented (removal increments once, rebuild increments again)
			let revision_after =
				<MembersPallet as MembershipProver>::ring_revision(&identifier, ring_index)
					.unwrap();
			assert!(
				revision_after > revision_before,
				"Revision should have incremented after removal and rebuild"
			);

			// Verify suspended members no longer in ring keys
			let ring_keys = RingKeys::<Test>::get((&identifier, ring_index as u32, 0u32));
			for member in &to_remove {
				assert!(!ring_keys.contains(member), "Suspended member should not be in ring keys");
			}

			// Verify remaining members still in ring keys and Included
			for (member, _) in members.iter().skip(2) {
				assert!(
					ring_keys.contains(member),
					"Remaining member should still be in ring keys"
				);
				let status =
					<MembersPallet as AppendOnlyMembers>::member_status(&identifier, member);
				assert!(matches!(status.unwrap(), RingPosition::Included { .. }));
			}
		});
	}
}

mod verify_memberships_in_ring_tests {
	use super::*;

	const BATCH_IDENTIFIER: Identifier = [77u8; 32];

	fn verification_item(
		proof: &<MockCrypto as GenerateVerifiable>::Proof,
		message: &[u8],
		context: Context,
	) -> BatchProofItem<<MockCrypto as GenerateVerifiable>::Proof> {
		BatchProofItem {
			proof: proof.clone(),
			context: context.to_vec(),
			message: message.to_vec(),
		}
	}

	/// Helper: create collection, add members, onboard all, build ring 0.
	fn setup_ring(
		identifier: Identifier,
		onboarding_size: u32,
		member_count: u8,
	) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
		create_test_collection(identifier, onboarding_size);
		let members = generate_members(identifier, 1, member_count);

		while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

		loop {
			let maybe = MembersPallet::should_build_ring(&identifier, 0, member_count as u32);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			} else {
				break;
			}
		}

		members
	}

	/// Helper: create a valid proof for a member in a ring.
	fn make_proof(
		identifier: &Identifier,
		ring_index: RingIndex,
		member: &MemberOf<Test>,
		secret: &SecretOf<Test>,
		context: &[u8; 32],
		message: &[u8],
	) -> <MockCrypto as GenerateVerifiable>::Proof {
		let ring_members =
			<MembersPallet as AppendOnlyMembers>::ring_members(identifier, ring_index);
		let commitment = MockCrypto::open((), member, ring_members.into_iter()).unwrap();
		MockCrypto::create(commitment, secret, context, message).unwrap().0
	}

	// Verifies batch verification returns one alias per proof and preserves input order.
	#[test]
	fn valid_batch_returns_aliases_in_input_order() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0xAA; 32];
			let message = b"batch test msg!!";

			// Build proofs for several members and submit them in one batch.
			let proofs = members[0..3]
				.iter()
				.map(|(member, secret)| {
					make_proof(&BATCH_IDENTIFIER, ring_index, member, secret, &context, message)
				})
				.collect::<Vec<_>>();
			let items = proofs
				.iter()
				.map(|proof| verification_item(proof, message, context))
				.collect::<Vec<_>>();

			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			);
			assert!(results.is_ok());
			let aliases = results.unwrap();
			assert_eq!(aliases.len(), 3);

			// Verify each alias matches what single verify_membership would return.
			for (i, (member, secret)) in members[0..3].iter().enumerate() {
				let proof =
					make_proof(&BATCH_IDENTIFIER, ring_index, member, secret, &context, message);
				let single = <MembersPallet as MembershipProver>::verify_membership(
					&BATCH_IDENTIFIER,
					&proof,
					ring_index,
					context,
					message,
				)
				.unwrap();
				assert_eq!(aliases[i].ca.alias, single.ca.alias);
				assert_eq!(aliases[i].ca.context, context);
				assert_eq!(aliases[i].ring, ring_index);
				assert_eq!(aliases[i].revision, single.revision);
			}
		});
	}

	// Verifies a single invalid proof rejects the entire batch even after a valid item.
	#[test]
	fn invalid_proof_fails_whole_batch() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0xBB; 32];
			let message = b"batch fail test!";

			// Put one valid proof and one invalid proof in the same batch.
			let (member0, secret0) = &members[0];
			let valid_proof =
				make_proof(&BATCH_IDENTIFIER, ring_index, member0, secret0, &context, message);
			let dummy_proof = verifiable::mock::MockProof::default();

			let items = vec![
				verification_item(&valid_proof, message, context),
				verification_item(&dummy_proof, message, context),
			];

			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::InvalidProof.into());
		});
	}

	// Verifies the batch aborts when item 0 is invalid even if item 1 is valid.
	#[test]
	fn invalid_first_proof_fails_whole_batch_even_if_later_items_are_valid() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0xBC; 32];
			let message = b"batch fail first!";

			// Put the invalid proof first and a valid proof second.
			let (member1, secret1) = &members[1];
			let valid_proof =
				make_proof(&BATCH_IDENTIFIER, ring_index, member1, secret1, &context, message);
			let dummy_proof = verifiable::mock::MockProof::default();

			let items = vec![
				verification_item(&dummy_proof, message, context),
				verification_item(&valid_proof, message, context),
			];

			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::InvalidProof.into());
		});
	}

	// Verifies a batch can contain proofs for distinct messages under the same context.
	#[test]
	fn mixed_messages_work() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0xCC; 32];
			let msg_a = b"message alpha--!";
			let msg_b = b"message bravo--!";

			// Create proofs for two different messages under the same context.
			let (member_a, secret_a) = &members[0];
			let (member_b, secret_b) = &members[1];

			let proof_a =
				make_proof(&BATCH_IDENTIFIER, ring_index, member_a, secret_a, &context, msg_a);
			let proof_b =
				make_proof(&BATCH_IDENTIFIER, ring_index, member_b, secret_b, &context, msg_b);

			let items = vec![
				verification_item(&proof_a, msg_a, context),
				verification_item(&proof_b, msg_b, context),
			];

			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			);
			assert!(results.is_ok());
			assert_eq!(results.unwrap().len(), 2);
		});
	}

	// Verifies a batch can contain proofs for distinct contexts with the same message.
	#[test]
	fn mixed_contexts_work() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let ctx_a = [0xDD; 32];
			let ctx_b = [0xEE; 32];
			let message = b"same message!!!!";

			// Create proofs for the same message under two different contexts.
			let (member_a, secret_a) = &members[0];
			let (member_b, secret_b) = &members[1];

			let proof_a =
				make_proof(&BATCH_IDENTIFIER, ring_index, member_a, secret_a, &ctx_a, message);
			let proof_b =
				make_proof(&BATCH_IDENTIFIER, ring_index, member_b, secret_b, &ctx_b, message);

			let items = vec![
				verification_item(&proof_a, message, ctx_a),
				verification_item(&proof_b, message, ctx_b),
			];

			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			);
			assert!(results.is_ok());
			let aliases = results.unwrap();
			assert_eq!(aliases.len(), 2);
			assert_eq!(aliases[0].ca.context, ctx_a);
			assert_eq!(aliases[1].ca.context, ctx_b);
		});
	}

	// Verifies batch verification fails when the collection does not exist.
	#[test]
	fn missing_collection_fails() {
		TestExt::new().execute_with(|| {
			let dummy_proof = verifiable::mock::MockProof::default();
			let items = vec![verification_item(&dummy_proof, b"msg!msg!msg!msg!", [0u8; 32])];

			// Attempt batch verification against a collection that does not exist.
			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&NONEXISTENT_IDENTIFIER,
				0,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::CollectionNotFound.into());
		});
	}

	// Verifies batch verification fails when the requested ring root does not exist.
	#[test]
	fn missing_ring_fails() {
		TestExt::new().execute_with(|| {
			create_test_collection(BATCH_IDENTIFIER, 5);
			let dummy_proof = verifiable::mock::MockProof::default();
			let items = vec![verification_item(&dummy_proof, b"msg!msg!msg!msg!", [0u8; 32])];

			// Attempt batch verification against a ring root that was never built.
			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				99, // nonexistent ring
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::NoRoot.into());
		});
	}

	// Verifies batch verification returns the current ring revision in the aliases.
	#[test]
	fn returned_revision_metadata_is_correct() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0xFF; 32];
			let message = b"revision test!!!";

			let expected_revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_IDENTIFIER, ring_index)
					.unwrap();

			// Verify the returned alias metadata carries the current ring revision.
			let (member, secret) = &members[0];
			let proof =
				make_proof(&BATCH_IDENTIFIER, ring_index, member, secret, &context, message);
			let items = vec![verification_item(&proof, message, context)];

			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring(
				&BATCH_IDENTIFIER,
				ring_index,
				&items,
			)
			.unwrap();

			assert_eq!(results[0].revision, expected_revision);
			assert_eq!(results[0].ring, ring_index);
		});
	}
}

mod self_inclusion_tests {
	use super::*;

	const SELF_INCLUSION_DELAY: u64 = 3600; // 1 hour

	#[test]
	fn self_include_works() {
		use sp_runtime::traits::DispatchTransaction;

		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			// Create a collection with a high onboarding size so automatic onboarding won't
			// trigger, and self-inclusion enabled.
			create_self_inclusion_collection(identifier, 200, SELF_INCLUSION_DELAY);

			// Add a single member — not enough for auto-onboarding (needs 200).
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member]
			));

			// Verify member is in the onboarding queue.
			assert!(matches!(
				Members::<Test>::get(identifier, member).unwrap(),
				RingPosition::Onboarding { .. }
			));

			// Advance time past the delay.
			advance_time(SELF_INCLUSION_DELAY + 1);

			// Build the extension and call, then run end-to-end through the extension
			// (validate → prepare → dispatch).
			let now = MockTime::now().as_secs();
			let (call, ext) = build_self_include_ext(&secret, identifier, member, now);
			let info = call.get_dispatch_info();
			let len = call.encode().len();
			let dispatch_result = ext
				.dispatch_transaction(RuntimeOrigin::none(), call, &info, len, 0)
				.expect("extension validation should succeed");
			assert_ok!(dispatch_result);

			// Verify member is now included in a ring.
			let position = Members::<Test>::get(identifier, member).unwrap();
			assert!(matches!(position, RingPosition::Included { ring_index: 0, .. }));

			// Verify the ring is marked stale.
			assert!(StaleRings::<Test>::contains_key(identifier, 0));

			// Verify active member count and ring index unchanged (ring not full).
			assert_eq!(ActiveMembers::<Test>::get(identifier), 1);
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 0);

			// Verify the member was removed from the onboarding queue.
			let (head, tail) = QueuePageIndices::<Test>::get(identifier);
			assert_eq!((head, tail), (0, 0));
			let keys = OnboardingQueue::<Test>::get(identifier, head);
			assert!(!keys.contains(&member));
		});
	}

	#[test]
	fn self_include_single_page_queue_does_not_corrupt_indices() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			create_self_inclusion_collection(identifier, 200, SELF_INCLUSION_DELAY);

			// Add a single member on the first queue page.
			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member]
			));

			// Verify there is exactly one queue page: head == tail.
			let (head, tail) = QueuePageIndices::<Test>::get(identifier);
			assert_eq!(head, tail);
			assert_eq!(head, 0);

			advance_time(SELF_INCLUSION_DELAY + 1);

			// Self-include the only member, emptying the only queue page.
			assert_ok!(Pallet::<Test>::do_self_include(identifier, member));

			// The self-included member should now be in the ring.
			assert!(matches!(
				Members::<Test>::get(identifier, member).unwrap(),
				RingPosition::Included { ring_index: 0, ring_page: 0, ring_position: 0 }
			));

			// After self-inclusion the queue must still be consistent: head == tail.
			let (head_after, tail_after) = QueuePageIndices::<Test>::get(identifier);
			assert_eq!(
				head_after, tail_after,
				"head must equal tail when queue is empty, got head={head_after} tail={tail_after}"
			);
			assert_eq!(head_after, 0);

			// Adding a new member should still work — the queue isn't corrupted.
			let secret2 = MockCrypto::new_secret([2; 32]);
			let member2 = MockCrypto::member_from_secret(&secret2);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member2]
			));
			assert!(matches!(
				Members::<Test>::get(identifier, member2).unwrap(),
				RingPosition::Onboarding { queue_page: 0, .. }
			));
		});
	}

	/// Helper to build a self_include RuntimeCall and sign the inherited implication.
	fn build_self_include_ext(
		secret: &SecretOf<Test>,
		identifier: Identifier,
		member: MemberOf<Test>,
		call_valid_at: u64,
	) -> (RuntimeCall, crate::extension::AsMember<Test>) {
		use crate::extension::{AsMember, AsMemberInfo};

		let call: RuntimeCall =
			crate::Call::<Test>::self_include { identifier, member, call_valid_at }.into();
		// The inherited implication for a single extension is TxBaseImplication((0u8, &call)).
		let msg = (0u8, &call).using_encoded(sp_io::hashing::blake2_256);
		let signature = MockCrypto::sign(secret, &msg[..]).expect("signing works");
		let ext = AsMember::<Test>::new(Some(AsMemberInfo::SelfInclude(signature)));
		(call, ext)
	}

	#[test]
	fn self_include_fails_when_not_enabled() {
		use sp_runtime::traits::DispatchTransaction;

		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			// Create a collection WITHOUT self-inclusion.
			create_test_collection(identifier, 200);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member]
			));

			let now = MockTime::now().as_secs();
			let (call, ext) = build_self_include_ext(&secret, identifier, member, now);
			let len = call.encode().len();

			// The extension should reject because self-inclusion is not enabled.
			assert!(ext
				.test_run(RuntimeOrigin::none(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.is_err());
		});
	}

	#[test]
	fn self_include_fails_when_too_early() {
		use sp_runtime::traits::DispatchTransaction;

		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			create_self_inclusion_collection(identifier, 200, SELF_INCLUSION_DELAY);

			let secret = MockCrypto::new_secret([1; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![member]
			));

			// Don't advance time — the member just joined so the delay hasn't passed.
			let now = MockTime::now().as_secs();
			let (call, ext) = build_self_include_ext(&secret, identifier, member, now);
			let len = call.encode().len();

			// The extension should reject because the delay hasn't elapsed.
			assert!(ext
				.test_run(RuntimeOrigin::none(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.is_err());
		});
	}

	/// Helper to onboard queued members and build a ring without creating the collection.
	fn onboard_and_build(identifier: Identifier, ring_index: RingIndex) {
		// Onboard in batches until the queue is exhausted.
		while MembersPallet::onboard_members(&identifier, false).unwrap_or(false) {}
		// Build the ring including all onboarded-but-not-yet-included members.
		let limit = RingExponent::R2e9.ring_capacity();
		while let Some(to_include) =
			MembersPallet::should_build_ring(&identifier, ring_index, limit)
		{
			assert_ok!(MembersPallet::build_ring(&identifier, ring_index, to_include));
		}
	}

	#[test]
	fn self_include_fails_for_non_onboarding_member() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			create_self_inclusion_collection(identifier, 5, SELF_INCLUSION_DELAY);

			// Add enough members for automatic onboarding.
			let members = generate_members(identifier, 1, 10);
			onboard_and_build(identifier, 0);

			// The first member should now be Included.
			let (member, _) = &members[0];
			assert!(matches!(
				Members::<Test>::get(identifier, member).unwrap(),
				RingPosition::Included { .. }
			));

			// Trying to self-include an already-included member should fail.
			assert_noop!(
				Pallet::<Test>::do_self_include(identifier, *member),
				Error::<Test>::NotOnboarding
			);
		});
	}

	#[test]
	fn self_include_fails_for_non_member() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			create_self_inclusion_collection(identifier, 200, SELF_INCLUSION_DELAY);

			let secret = MockCrypto::new_secret([99; 32]);
			let non_member = MockCrypto::member_from_secret(&secret);

			assert_noop!(
				Pallet::<Test>::do_self_include(identifier, non_member),
				Error::<Test>::NotMember
			);
		});
	}

	#[test]
	fn self_include_respects_ring_capacity() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;
			let capacity = RingExponent::R2e9.ring_capacity();
			// Use onboarding_size = 1 so all members can be onboarded.
			create_self_inclusion_collection(identifier, 1, SELF_INCLUSION_DELAY);

			// Fill ring 0 to capacity - 1 via normal onboarding.
			let fill_count = (capacity - 1) as u8;
			let _members = generate_members(identifier, 1, fill_count);
			onboard_and_build(identifier, 0);

			assert_eq!(RingKeysStatus::<Test>::get(identifier, 0u32).total, capacity - 1);
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 0);

			// Add two more members to the queue.
			let secret_last = MockCrypto::new_secret([0; 32]);
			let last_member = MockCrypto::member_from_secret(&secret_last);
			let mut seed_extra = [0u8; 32];
			seed_extra[0] = 255;
			let secret_extra = MockCrypto::new_secret(seed_extra);
			let extra_member = MockCrypto::member_from_secret(&secret_extra);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				vec![last_member, extra_member]
			));
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 0);

			advance_time(SELF_INCLUSION_DELAY + 1);

			// Self-include the last member — this fills ring 0 and should advance
			// CurrentRingIndex to 1.
			assert_ok!(Pallet::<Test>::do_self_include(identifier, last_member));
			assert!(matches!(
				Members::<Test>::get(identifier, last_member).unwrap(),
				RingPosition::Included { ring_index: 0, .. }
			));
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 1);

			// Self-include the extra member — should land on ring 1.
			assert_ok!(Pallet::<Test>::do_self_include(identifier, extra_member));
			assert!(matches!(
				Members::<Test>::get(identifier, extra_member).unwrap(),
				RingPosition::Included { ring_index: 1, .. }
			));
			// Ring 1 has only 1 member, so current ring index stays at 1.
			assert_eq!(CurrentRingIndex::<Test>::get(identifier), 1);
		});
	}
}

mod verify_memberships_in_ring_at_rev_tests {
	use super::*;

	const BATCH_AT_REV_IDENTIFIER: Identifier = [78u8; 32];

	fn verification_item(
		proof: &<MockCrypto as GenerateVerifiable>::Proof,
		message: &[u8],
		context: Context,
	) -> BatchProofItem<<MockCrypto as GenerateVerifiable>::Proof> {
		BatchProofItem {
			proof: proof.clone(),
			context: context.to_vec(),
			message: message.to_vec(),
		}
	}

	fn setup_ring(
		identifier: Identifier,
		onboarding_size: u32,
		member_count: u8,
	) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
		create_test_collection(identifier, onboarding_size);
		let members = generate_members(identifier, 1, member_count);

		while MembersPallet::onboard_members(&identifier, false) == Ok(true) {}

		loop {
			let maybe = MembersPallet::should_build_ring(&identifier, 0, member_count as u32);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			} else {
				break;
			}
		}

		members
	}

	fn make_proof(
		identifier: &Identifier,
		ring_index: RingIndex,
		member: &MemberOf<Test>,
		secret: &SecretOf<Test>,
		context: &[u8; 32],
		message: &[u8],
	) -> <MockCrypto as GenerateVerifiable>::Proof {
		let ring_members =
			<MembersPallet as AppendOnlyMembers>::ring_members(identifier, ring_index);
		let commitment = MockCrypto::open((), member, ring_members.into_iter()).unwrap();
		MockCrypto::create(commitment, secret, context, message).unwrap().0
	}

	#[test]
	fn works_for_current_and_old_revision_preserving_input_order() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_AT_REV_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let old_context = [0x11; 32];
			let old_message = b"batch at rev old!";
			let old_revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_AT_REV_IDENTIFIER, 0)
					.unwrap();

			let old_proofs = members[0..3]
				.iter()
				.map(|(member, secret)| {
					make_proof(
						&BATCH_AT_REV_IDENTIFIER,
						ring_index,
						member,
						secret,
						&old_context,
						old_message,
					)
				})
				.collect::<Vec<_>>();
			let old_items = old_proofs
				.iter()
				.map(|proof| verification_item(proof, old_message, old_context))
				.collect::<Vec<_>>();

			let new_members = generate_members_with_offset(BATCH_AT_REV_IDENTIFIER, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&BATCH_AT_REV_IDENTIFIER, false));
			if let Some(to_include) =
				MembersPallet::should_build_ring(&BATCH_AT_REV_IDENTIFIER, ring_index, 255)
			{
				assert_ok!(MembersPallet::build_ring(
					&BATCH_AT_REV_IDENTIFIER,
					ring_index,
					to_include
				));
			}

			let current_revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_AT_REV_IDENTIFIER, 0)
					.unwrap();
			assert!(current_revision > old_revision);

			let old_results =
				<MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
					&BATCH_AT_REV_IDENTIFIER,
					ring_index,
					old_revision,
					&old_items,
				)
				.unwrap();
			assert_eq!(old_results.len(), old_items.len());

			for (result, proof) in old_results.iter().zip(old_proofs.iter()) {
				let single = <MembersPallet as MembershipProver>::verify_membership_at_rev(
					&BATCH_AT_REV_IDENTIFIER,
					proof,
					ring_index,
					old_revision,
					old_context,
					old_message,
				)
				.unwrap();
				assert_eq!(result.alias, single.alias);
				assert_eq!(result.context, old_context);
			}

			let current_context = [0x22; 32];
			let current_message = b"batch at rev new!";
			let current_proofs = new_members[0..2]
				.iter()
				.map(|(member, secret)| {
					make_proof(
						&BATCH_AT_REV_IDENTIFIER,
						ring_index,
						member,
						secret,
						&current_context,
						current_message,
					)
				})
				.collect::<Vec<_>>();
			let current_items = current_proofs
				.iter()
				.map(|proof| verification_item(proof, current_message, current_context))
				.collect::<Vec<_>>();

			let current_results =
				<MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
					&BATCH_AT_REV_IDENTIFIER,
					ring_index,
					current_revision,
					&current_items,
				)
				.unwrap();
			assert_eq!(current_results.len(), current_items.len());

			for (result, proof) in current_results.iter().zip(current_proofs.iter()) {
				let single = <MembersPallet as MembershipProver>::verify_membership_at_rev(
					&BATCH_AT_REV_IDENTIFIER,
					proof,
					ring_index,
					current_revision,
					current_context,
					current_message,
				)
				.unwrap();
				assert_eq!(result.alias, single.alias);
				assert_eq!(result.context, current_context);
			}
		});
	}

	#[test]
	fn old_revision_fails_after_retention_expires() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_AT_REV_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0x33; 32];
			let message = b"batch at rev expiry";
			let old_revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_AT_REV_IDENTIFIER, 0)
					.unwrap();

			let proof = make_proof(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				&members[0].0,
				&members[0].1,
				&context,
				message,
			);
			let items = vec![verification_item(&proof, message, context)];

			let _new_members = generate_members_with_offset(BATCH_AT_REV_IDENTIFIER, 11, 15, 101);
			assert_ok!(MembersPallet::onboard_members(&BATCH_AT_REV_IDENTIFIER, false));
			if let Some(to_include) =
				MembersPallet::should_build_ring(&BATCH_AT_REV_IDENTIFIER, ring_index, 255)
			{
				assert_ok!(MembersPallet::build_ring(
					&BATCH_AT_REV_IDENTIFIER,
					ring_index,
					to_include
				));
			}

			advance_time(601);

			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				old_revision,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::NoRoot.into());
		});
	}

	#[test]
	fn fails_when_ring_is_removed() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_AT_REV_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0x44; 32];
			let message = b"batch removed ring";
			let revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_AT_REV_IDENTIFIER, 0)
					.unwrap();

			let proofs = members[0..3]
				.iter()
				.map(|(member, secret)| {
					make_proof(
						&BATCH_AT_REV_IDENTIFIER,
						ring_index,
						member,
						secret,
						&context,
						message,
					)
				})
				.collect::<Vec<_>>();
			let items = proofs
				.iter()
				.map(|proof| verification_item(proof, message, context))
				.collect::<Vec<_>>();

			// Verify batch works before removal
			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				revision,
				&items,
			)
			.expect("Batch should verify before ring removal");
			assert_eq!(results.len(), items.len());

			// Manually advance to the next ring to allow ring 0 to be removed
			CurrentRingIndex::<Test>::insert(BATCH_AT_REV_IDENTIFIER, 1u32);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(
				&BATCH_AT_REV_IDENTIFIER,
				0
			));

			// Verify batch fails after removal (instantly expired)
			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				revision,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::NoRoot.into());
		});
	}

	#[test]
	fn fails_when_collection_is_deleted() {
		TestExt::new().execute_with(|| {
			let members = setup_ring(BATCH_AT_REV_IDENTIFIER, 5, 10);
			let ring_index = 0;
			let context = [0x55; 32];
			let message = b"batch deleted collection";
			let revision =
				<MembersPallet as MembershipProver>::ring_revision(&BATCH_AT_REV_IDENTIFIER, 0)
					.unwrap();

			let proofs = members[0..3]
				.iter()
				.map(|(member, secret)| {
					make_proof(
						&BATCH_AT_REV_IDENTIFIER,
						ring_index,
						member,
						secret,
						&context,
						message,
					)
				})
				.collect::<Vec<_>>();
			let items = proofs
				.iter()
				.map(|proof| verification_item(proof, message, context))
				.collect::<Vec<_>>();

			// Verify batch works before deletion
			let results = <MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				revision,
				&items,
			)
			.expect("Batch should verify before collection deletion");
			assert_eq!(results.len(), items.len());

			// Delete collection
			let owner = MockLocation(1);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&BATCH_AT_REV_IDENTIFIER
			));

			// Verify batch fails after deletion
			let result = <MembersPallet as MembershipProver>::verify_memberships_in_ring_at_rev(
				&BATCH_AT_REV_IDENTIFIER,
				ring_index,
				revision,
				&items,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::CollectionNotFound.into());
		});
	}
}

mod ocw_tests {
	use super::*;

	const OCW_IDENTIFIER: Identifier = [50u8; 32];

	// TODO: Add coverage for merging onboarding queue pages.
	#[test]
	fn ocw_full_flow() {
		TestExt::new().execute_with(|| {
			let identifier = OCW_IDENTIFIER;

			// Create AppendOnly collection with onboarding size 1.
			create_append_only_collection(identifier, 1);

			// Add 10 members.
			let members = generate_members(identifier, 1, 10);

			// No members onboarded yet, no ring built.
			assert_eq!(RingKeysStatus::<Test>::get(identifier, 0).total, 0);
			assert!(!Root::<Test>::contains_key(identifier, 0));

			// Onboard all members and build the ring.
			advance_to_block(10);

			// All 10 members onboarded, ring built.
			assert_eq!(RingKeysStatus::<Test>::get(identifier, 0).total, 10);
			assert!(!StaleRings::<Test>::contains_key(identifier, 0));
			assert!(Root::<Test>::contains_key(identifier, 0));

			// Suspend a member.
			let suspended_member = vec![members[0].0];
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(
				&identifier,
				&suspended_member
			));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Pending suspensions exist.
			assert!(MembersPallet::should_remove_suspended_keys(&identifier, 0, true));

			// Remove suspended keys.
			advance_to_block(20);

			// Suspended keys removed, ring rebuilt with 9 members.
			assert!(!MembersPallet::should_remove_suspended_keys(&identifier, 0, true));
			assert_eq!(RingKeysStatus::<Test>::get(identifier, 0).total, 9);

			// Add the suspended member back to the collection.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::add_members(
				&identifier,
				suspended_member
			));

			// Advance to ring 1 and remove ring 0.
			manually_advance_to_ring(&identifier, 1);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&identifier, 0));
			// Ring deletion queue populated.
			assert!(RingDeletionQueue::<Test>::contains_key((identifier, 0, 0)));

			// Process deletion queue.
			advance_to_block(30);

			// Ring 0 fully cleaned up.
			assert!(!RingDeletionQueue::<Test>::contains_key((identifier, 0, 0)));
			assert!(!Root::<Test>::contains_key(identifier, 0));
			assert!(RingKeys::<Test>::get((&identifier, 0u32, 0u32)).is_empty());

			// Delete the collection.
			let owner = MockLocation(1);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(owner, &identifier));
			// Collection suspended.
			assert!(SuspendedCollections::<Test>::contains_key(identifier));

			// Process collection deletion.
			advance_to_block(40);

			// Collection fully cleaned up.
			assert!(!SuspendedCollections::<Test>::contains_key(identifier));
			assert!(!Collections::<Test>::contains_key(identifier));

			// All members deleted.
			for (member, _) in &members {
				assert!(Members::<Test>::get(identifier, member).is_none());
			}
		});
	}
}

mod mark_ring_stale_tests {
	use super::*;
	use sp_runtime::transaction_validity::InvalidTransaction;

	const STALE_IDENTIFIER: Identifier = [60u8; 32];

	#[test]
	fn ensure_can_mark_ring_stale_works() {
		TestExt::new().execute_with(|| {
			create_test_collection(STALE_IDENTIFIER, 5);

			// Add members and onboard (creates StaleRings entry).
			generate_members(STALE_IDENTIFIER, 1, 5);
			assert_ok!(MembersPallet::onboard_members(&STALE_IDENTIFIER, false));

			// Ring has total > included.
			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert!(status.total > status.included);

			// Remove the StaleRings entry to simulate it being lost.
			StaleRings::<Test>::remove(STALE_IDENTIFIER, 0);
			assert!(!StaleRings::<Test>::contains_key(STALE_IDENTIFIER, 0));

			// Validation should succeed.
			assert!(MembersPallet::ensure_can_mark_ring_stale(&STALE_IDENTIFIER, 0).is_ok());
		});
	}

	#[test]
	fn ensure_can_mark_ring_stale_fails_for_suspended_collection() {
		TestExt::new().execute_with(|| {
			let owner = MockLocation(1);
			create_test_collection(STALE_IDENTIFIER, 5);

			// Delete collection.
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(
				owner,
				&STALE_IDENTIFIER
			));

			// Validation should reject.
			assert_eq!(
				MembersPallet::ensure_can_mark_ring_stale(&STALE_IDENTIFIER, 0),
				Err(InvalidTransaction::Stale.into())
			);
		});
	}

	#[test]
	fn ensure_can_mark_ring_stale_fails_if_already_stale() {
		TestExt::new().execute_with(|| {
			create_test_collection(STALE_IDENTIFIER, 5);
			generate_members(STALE_IDENTIFIER, 1, 5);
			assert_ok!(MembersPallet::onboard_members(&STALE_IDENTIFIER, false));

			// StaleRings entry already exists from onboarding.
			assert!(StaleRings::<Test>::contains_key(STALE_IDENTIFIER, 0));

			// Validation should reject.
			assert_eq!(
				MembersPallet::ensure_can_mark_ring_stale(&STALE_IDENTIFIER, 0),
				Err(InvalidTransaction::Stale.into())
			);
		});
	}

	#[test]
	fn ensure_can_mark_ring_stale_fails_if_ring_fully_included() {
		TestExt::new().execute_with(|| {
			// Add members, onboard, and build ring (total == included).
			setup_collection_with_built_ring(STALE_IDENTIFIER, 5);

			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert_eq!(status.total, status.included);

			// Remove StaleRings entry (build_ring clears it).
			StaleRings::<Test>::remove(STALE_IDENTIFIER, 0);

			// Validation should reject — nothing to rebuild.
			assert_eq!(
				MembersPallet::ensure_can_mark_ring_stale(&STALE_IDENTIFIER, 0),
				Err(InvalidTransaction::Stale.into())
			);
		});
	}

	#[test]
	fn ensure_can_mark_ring_stale_fails_for_nonexistent_ring() {
		TestExt::new().execute_with(|| {
			// No collection, no ring — RingKeysStatus returns default (total=0, included=0).
			assert_eq!(
				MembersPallet::ensure_can_mark_ring_stale(&STALE_IDENTIFIER, 0),
				Err(InvalidTransaction::Stale.into())
			);
		});
	}

	#[test]
	fn mark_ring_stale_triggers_ring_build_via_ocw() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);

			// Create collection with onboarding size 1 so small batches get onboarded.
			create_test_collection(STALE_IDENTIFIER, 1);
			let _members = generate_members(STALE_IDENTIFIER, 1, 5);

			// Onboard and build ring manually.
			assert_ok!(MembersPallet::onboard_members(&STALE_IDENTIFIER, false));
			let to_include = MembersPallet::should_build_ring(&STALE_IDENTIFIER, 0, 255).unwrap();
			assert_ok!(MembersPallet::build_ring(&STALE_IDENTIFIER, 0, to_include));
			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert_eq!(status.total, status.included);

			// Add more members and onboard them (but don't build).
			generate_members(STALE_IDENTIFIER, 6, 8);
			assert_ok!(MembersPallet::onboard_members(&STALE_IDENTIFIER, false));

			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert!(status.total > status.included);

			// Remove StaleRings to simulate it being lost.
			StaleRings::<Test>::remove(STALE_IDENTIFIER, 0);

			// Ring won't be rebuilt without the stale entry.
			advance_to_block(10);
			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert!(status.total > status.included, "Ring not rebuilt without StaleRings entry");

			// Mark it stale via the extrinsic.
			assert_ok!(MembersPallet::mark_ring_stale_authorized(
				RuntimeOrigin::from(frame_system::RawOrigin::Authorized),
				STALE_IDENTIFIER,
				0,
			));

			// Now the OCW should pick it up and rebuild.
			advance_to_block(20);
			let status = RingKeysStatus::<Test>::get(STALE_IDENTIFIER, 0);
			assert_eq!(status.total, status.included, "Ring should be rebuilt after marking stale");
		});
	}
}

mod old_roots_tests {
	use super::*;
	use sp_runtime::transaction_validity::InvalidTransaction;

	/// Helper to setup a collection, add members, onboard and build ring.
	fn setup_collection_and_build_ring(
		identifier: Identifier,
		onboarding_size: u32,
		member_count: u8,
	) -> Vec<(MemberOf<Test>, SecretOf<Test>)> {
		create_test_collection(identifier, onboarding_size);
		let members = generate_members(identifier, 0, member_count);
		MembersPallet::onboard_members(&identifier, false).unwrap();
		let to_include = MembersPallet::should_build_ring(&identifier, 0, 255).unwrap();
		MembersPallet::build_ring(&identifier, 0, to_include).unwrap();

		members
	}

	/// Helper to create a proof for membership verification.
	fn create_proof_for_member(
		identifier: &Identifier,
		ring_index: RingIndex,
		member: &MemberOf<Test>,
		secret: &SecretOf<Test>,
		context: [u8; 32],
		message: &[u8],
	) -> (<MockCrypto as GenerateVerifiable>::Proof, Alias) {
		let ring_members =
			<MembersPallet as AppendOnlyMembers>::ring_members(identifier, ring_index);
		let commitment = MockCrypto::open((), member, ring_members.into_iter()).unwrap();
		MockCrypto::create(commitment, secret, &context, message).unwrap()
	}

	// Test for:
	// - `verify_membership_at_rev` works for old revision
	// - `verify_membership_at_rev` works for the current revision
	// - after the retention period expires, `verify_membership_at_rev` no longer works for the old
	//   revision
	#[test]
	fn verify_membership_at_rev_works_for_old_revision() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let members = setup_collection_and_build_ring(identifier, 5, 10);

			// Get first revision
			let rev0 = <MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();
			assert_eq!(rev0, 0);

			// Get the root at revision 0
			let root_rev0 = Root::<Test>::get(identifier, 0).unwrap().root.clone();

			// Create proof for member at revision 0
			let (member, secret) = &members[0];
			let context = [1u8; 32];
			let message = b"test message rev0";
			let (proof_rev0, _alias) =
				create_proof_for_member(&identifier, 0, member, secret, context, message);

			// Verify membership at current revision works
			<MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof_rev0,
				0,
				context,
				message,
			)
			.expect("Proof should verify against current revision");
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0,
				context,
				message,
			)
			.expect("Proof should verify at revision 0");

			// Add more members to trigger a new revision
			let new_members = generate_members_with_offset(identifier, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&identifier, false));

			// Build ring again (creates revision 1)
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			// Verify revision incremented
			let rev1 = <MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();
			assert_eq!(rev1, 1);

			// Verify old root is stored in OldRoots
			let old_root_entry = OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(0u32)));
			assert!(old_root_entry.is_some(), "Old root should be stored");
			assert_eq!(old_root_entry.unwrap().root, root_rev0);

			// Create proof for new member at revision 1
			let (new_member, new_secret) = &new_members[0];
			let new_ring_members =
				<MembersPallet as AppendOnlyMembers>::ring_members(&identifier, 0);
			let new_commitment =
				MockCrypto::open((), new_member, new_ring_members.into_iter()).unwrap();
			let message_rev1 = b"test message rev1";
			let (proof_rev1, _) =
				MockCrypto::create(new_commitment, new_secret, &context, message_rev1).unwrap();

			// Verify new proof works with current revision
			<MembersPallet as MembershipProver>::verify_membership(
				&identifier,
				&proof_rev1,
				0,
				context,
				message_rev1,
			)
			.expect("New proof should verify against current revision");
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev1,
				0,
				1, // revision 1
				context,
				message_rev1,
			)
			.expect("New proof should verify against current revision");

			// Verify old proof works with verify_membership_at_rev for rev 0
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect("Old proof should verify at old revision");
			// Verify new proof works with verify_membership_at_rev for rev 0
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev1,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect_err("New proof should NOT verify at old revision");

			// Advance time beyond retention period (600 seconds = 10 minutes)
			advance_time(601);

			// Verify old proof no longer verifies at revision 0
			let result = <MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::NoRoot.into());
		});
	}

	// Test that trying to authorize cleanup before expiration fails
	#[test]
	fn clean_up_old_roots_fails_before_expiration() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup and build initial ring, then rebuild to create old root
			setup_collection_and_build_ring(identifier, 5, 10);
			let _ = generate_members_with_offset(identifier, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&identifier, false));
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			// Verify old root exists
			assert!(OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(0u32))).is_some());

			// Try to authorize cleanup before expiration (should fail with Future error)
			let result = MembersPallet::ensure_can_clean_up_old_roots(&identifier, 0);
			assert!(result.is_err(), "Authorization should fail before expiration");
			// The error is InvalidTransaction::Future when not yet expired
			assert_eq!(
				result.unwrap_err(),
				InvalidTransaction::Future.into(),
				"Should return Future invalid transaction"
			);

			// Verify old root is still there
			assert!(
				OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(0u32))).is_some(),
				"Old root should still exist"
			);
		});
	}

	// Test that the offchain worker cleans up some old roots for different rings in
	// different collections.
	#[test]
	fn ocw_cleans_up_old_roots() {
		TestExt::new().execute_with(|| {
			let id1 = TEST_IDENTIFIER;
			let id2 = [45u8; 32];

			let num_old_roots = 300;
			create_append_only_collection(id1, 1);
			create_append_only_collection(id2, 1);

			// Create multiple old roots by repeatedly adding 1 member and rebuilding
			for i in 0..num_old_roots {
				for id in &[id1, id2] {
					let secret = create_unique_secret();
					let public = MockCrypto::member_from_secret(&secret);
					<MembersPallet as AppendOnlyMembers>::add_members(id, vec![public]).unwrap();
					MembersPallet::onboard_members(id, false).unwrap();
					MembersPallet::build_ring(id, i / 255, 1).unwrap();
				}
			}

			assert_eq!(OldRoots::<Test>::iter_prefix((id1, 0u32)).count(), 254);
			assert_eq!(OldRoots::<Test>::iter_prefix((id1, 1u32)).count(), 44);
			assert_eq!(OldRoots::<Test>::iter_prefix((id2, 0u32)).count(), 254);
			assert_eq!(OldRoots::<Test>::iter_prefix((id2, 1u32)).count(), 44);

			// Make time beyond expiration (600 seconds + 1)
			advance_time(601);

			// Offchain worker submits cleanup transaction for first 1000 roots of each ring
			advance_to_block(1);

			// No root left
			assert_eq!(OldRoots::<Test>::iter_keys().count(), 0);
		});
	}

	// Test that the offchain worker only cleans up roots up until the specified limit
	#[test]
	fn ocw_cleans_up_old_roots_max_cleanup_limit() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Create 1023 old roots - more than CLEANUP_LIMIT=1000, so cleanup happens in 2 batches
			let num_old_roots = 1023;
			assert_eq!(
				OFFCHAIN_WORKER_OLD_ROOT_CLEANUP_LIMIT, 1000,
				"Test assumes CLEANUP_LIMIT is 1000"
			);

			// Create the collection
			<MembersPallet as AppendOnlyMembers>::create_collection(
				MockLocation(1),
				&identifier,
				1,
				RingMode::AppendOnly,
				RingExponent::R2e14,
				None,
			)
			.unwrap();

			// Create multiple old roots by repeatedly adding 1 member and rebuilding
			for _ in 0..=num_old_roots {
				let secret = create_unique_secret();
				let public = MockCrypto::member_from_secret(&secret);
				<MembersPallet as AppendOnlyMembers>::add_members(&identifier, vec![public])
					.unwrap();
				MembersPallet::onboard_members(&identifier, false).unwrap();
				MembersPallet::build_ring(&identifier, 0, 1).unwrap();
			}

			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0)).count(), 1023);

			// Make time beyond expiration (600 seconds + 1)
			advance_time(601);

			// First batch: offchain worker submits cleanup transaction for first 1000 roots
			advance_to_block(1);
			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0u32)).count(), 23);

			// Second batch: cleanup the remaining 23 roots
			advance_to_block(2);
			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0u32)).count(), 0);
		});
	}

	/// Test offchain worker only removes expired roots and events are emitted.
	#[test]
	fn ocw_only_removes_expired_old_roots() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with onboarding_size=1
			setup_collection_and_build_ring(identifier, 1, 5);

			// Create 5 old roots at initial time (T=1,000,000)
			for _ in 0..5 {
				let secret = create_unique_secret();
				let public = MockCrypto::member_from_secret(&secret);
				<MembersPallet as AppendOnlyMembers>::add_members(&identifier, vec![public])
					.unwrap();
				MembersPallet::onboard_members(&identifier, false).unwrap();
				MembersPallet::build_ring(&identifier, 0, 1).unwrap();
			}

			// Verify we have 5 old roots (revisions 0-4)
			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0u32)).count(), 5);

			// Advance time by 400 seconds (T=1,000,400) - not yet expired
			advance_time(400);

			// Create 5 more old roots at T=1,000,400
			for _ in 0..5 {
				let secret = create_unique_secret();
				let public = MockCrypto::member_from_secret(&secret);
				<MembersPallet as AppendOnlyMembers>::add_members(&identifier, vec![public])
					.unwrap();
				MembersPallet::onboard_members(&identifier, false).unwrap();
				MembersPallet::build_ring(&identifier, 0, 1).unwrap();
			}

			// Verify we now have 10 old roots (revisions 0-9)
			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0u32)).count(), 10);

			// Advance time by 300 seconds (T=1,000,700)
			// First 5 roots are now expired
			// Later 5 roots are NOT expired
			advance_time(300);

			// Clear events before cleanup
			System::reset_events();

			// Run offchain worker - should only remove expired roots
			advance_to_block(1);

			// Verify OldRootCleanedUp events were deposited for revisions 0-4
			let events = System::events();
			for rev in 0..5u32 {
				assert!(
					events.iter().any(|e| matches!(
						&e.event,
						RuntimeEvent::MembersPallet(Event::OldRootCleanedUp {
							identifier: id,
							ring_index: 0,
							revision
						}) if *id == identifier && *revision == rev
					)),
					"OldRootCleanedUp event should be deposited for revision {rev}"
				);
			}

			// Verify only 5 non-expired roots remain (the ones created at T=1,000,400)
			let remaining = OldRoots::<Test>::iter_prefix((identifier, 0u32)).count();
			assert_eq!(remaining, 5, "Only non-expired old roots should remain");

			// Verify the remaining roots are revisions 5-9 (the later ones)
			for rev in 0..5u32 {
				assert!(OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(rev))).is_none());
			}
			for rev in 5..10u32 {
				assert!(OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(rev))).is_some());
			}

			// Now advance time to expire the remaining roots
			advance_time(400);

			// Run offchain worker again
			advance_to_block(2);

			// All roots should now be removed
			assert_eq!(OldRoots::<Test>::iter_prefix((identifier, 0u32)).count(), 0);
		});
	}

	/// Test that for OldRoots the revision in `BigEndianU32` preserves numeric order during
	/// iteration.
	///
	/// This is critical because Substrate's storage iteration uses lexicographic ordering of
	/// the encoded key bytes. Little-endian encoding (SCALE default) would break ordering at
	/// 256 boundaries: e.g., 256 (0x00 0x01 0x00 0x00) would sort before 1 (0x01 0x00 0x00 0x00).
	/// Big-endian encoding ensures 255 < 256 < 257 in both numeric and lexicographic order.
	#[test]
	fn old_roots_big_endian_revision_index_order_is_correct() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Insert revisions 255, 256, 257 directly to OldRoots storage
			// to test that iteration returns them in ascending order.
			let revisions = [255u32, 256u32, 257u32];

			// Insert old roots for each revision
			for &rev in &revisions {
				OldRoots::<Test>::insert(
					(identifier, 0u32, BigEndianU32(rev)),
					OldRoot::<Test> {
						root: MockCrypto::start_members(()).into(),
						archived_at: 1000,
					},
				);
			}

			// Collect revision indices from iteration in order
			let iterated_revisions: Vec<u32> = OldRoots::<Test>::iter_prefix((identifier, 0u32))
				.map(|(rev, _)| rev.0)
				.collect();

			// Verify iteration returns revisions in ascending numeric order
			assert_eq!(
				iterated_revisions,
				vec![255u32, 256u32, 257u32],
				"Iteration must return revisions in ascending numeric order. \
				If this fails, BigEndianU32 encoding is broken."
			);
		});
	}

	#[test]
	fn verify_membership_at_rev_fails_when_ring_is_deleted() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let members = setup_collection_and_build_ring(identifier, 5, 10);

			// Create proof for member at revision 0
			let (member, secret) = &members[0];
			let context = [1u8; 32];
			let message = b"test message rev0";
			let (proof_rev0, _alias) =
				create_proof_for_member(&identifier, 0, member, secret, context, message);

			// Add more members to trigger a new revision
			let _new_members = generate_members_with_offset(identifier, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&identifier, false));

			// Build ring again (creates revision 1)
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			// Verify old proof works with verify_membership_at_rev for rev 0
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect("Old proof should verify at old revision");

			// Manually advance to the next ring to allow ring 0 to be removed
			CurrentRingIndex::<Test>::insert(identifier, 1u32);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&identifier, 0));

			// Verify that proofs fail when the ring is removed (instantly expired).
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect_err("Old proof should NOT verify when ring is removed");
		});
	}

	#[test]
	fn verify_membership_at_rev_fails_when_collection_is_deleted() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let members = setup_collection_and_build_ring(identifier, 5, 10);

			// Create proof for member at revision 0
			let (member, secret) = &members[0];
			let context = [1u8; 32];
			let message = b"test message rev0";
			let (proof_rev0, _alias) =
				create_proof_for_member(&identifier, 0, member, secret, context, message);

			// Add more members to trigger a new revision
			let _new_members = generate_members_with_offset(identifier, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&identifier, false));

			// Build ring again (creates revision 1)
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			// Verify old proof works with verify_membership_at_rev for rev 0
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect("Old proof should verify at old revision");

			// Delete collection (moves it to SuspendedCollections)
			let owner = MockLocation(1); // Default owner established in create_test_collection
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(owner, &identifier));

			// Verify old proof no longer verifies at revision 0 (CollectionNotFound)
			let result = <MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			);
			assert_eq!(result.unwrap_err(), Error::<Test>::CollectionNotFound.into());
		});
	}

	/// When members are suspended and their keys removed, test that the
	/// previous ring root is archived in `OldRoots` so that proofs generated
	/// against it remain verifiable:
	///
	/// - immediately after removal, and
	/// - after the ring is rebuilt
	#[test]
	fn old_root_preserved_and_proofs_valid_after_key_suspension() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let members = setup_collection_and_build_ring(identifier, 5, 10);

			// Record root before removal
			let root_before = Root::<Test>::get(identifier, 0).unwrap().root.clone();

			// Create proof for member at revision 0
			let (member, secret) = &members[2]; // Use a member that won't be suspended
			let context = [1u8; 32];
			let message = b"test message rev0";
			let (proof_rev0, _alias) =
				create_proof_for_member(&identifier, 0, member, secret, context, message);

			// Start removal session, suspend 2 members, end session
			assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
			let to_remove: Vec<_> = members.iter().take(2).map(|(m, _)| *m).collect();
			assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(&identifier, &to_remove));
			assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));

			// Remove suspended keys. This creates an empty intermediate and explicitly writes
			// the previous root directly to the OldRoots storage.
			MembersPallet::remove_suspended_keys(&identifier, 0);

			// Verify old root is stored in OldRoots
			let old_root_entry = OldRoots::<Test>::get((identifier, 0u32, BigEndianU32(0u32)));
			assert!(old_root_entry.is_some(), "Old root should be stored after suspending keys");
			assert_eq!(old_root_entry.unwrap().root, root_before);

			// Verify old proof works with verify_membership_at_rev for rev 0
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect("Old proof should verify at old revision");

			// Rebuild the ring with the remaining members
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			// Verify the old proof STILL works with verify_membership_at_rev for rev 0 after
			// rebuild
			<MembersPallet as MembershipProver>::verify_membership_at_rev(
				&identifier,
				&proof_rev0,
				0,
				0, // revision 0
				context,
				message,
			)
			.expect("Old proof should verify at old revision after rebuilding");
		});
	}

	#[test]
	fn is_revision_valid_returns_false_when_ring_is_removed() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let _members = setup_collection_and_build_ring(identifier, 5, 10);

			let revision_0 =
				<MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();

			// Verify revision is valid before removal
			assert!(
				<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Revision should be valid before ring removal"
			);

			// Manually advance to the next ring to allow ring 0 to be removed
			CurrentRingIndex::<Test>::insert(identifier, 1u32);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&identifier, 0));

			// Verify revision is no longer valid after removal (instantly expired)
			assert!(
				!<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Revision should NOT be valid after ring removal"
			);
		});
	}

	#[test]
	fn is_revision_valid_returns_false_when_collection_is_deleted() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let _members = setup_collection_and_build_ring(identifier, 5, 10);

			let revision_0 =
				<MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();

			// Verify revision is valid before deletion
			assert!(
				<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Revision should be valid before collection deletion"
			);

			// Delete collection
			let owner = MockLocation(1);
			assert_ok!(<MembersPallet as AppendOnlyMembers>::delete_collection(owner, &identifier));

			// Verify revision is no longer valid after collection deletion
			assert!(
				!<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Revision should NOT be valid after collection deletion"
			);
		});
	}

	#[test]
	fn is_revision_valid_returns_false_for_old_revision_when_ring_is_removed() {
		TestExt::new().execute_with(|| {
			let identifier = TEST_IDENTIFIER;

			// Setup collection with initial members and build ring (revision 0)
			let _members = setup_collection_and_build_ring(identifier, 5, 10);

			let revision_0 =
				<MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();

			// Add more members to trigger a new revision
			let _new_members = generate_members_with_offset(identifier, 11, 15, 100);
			assert_ok!(MembersPallet::onboard_members(&identifier, false));

			// Build ring again (creates revision 1)
			let maybe = MembersPallet::should_build_ring(&identifier, 0, 255);
			if let Some(to_include) = maybe {
				assert_ok!(MembersPallet::build_ring(&identifier, 0, to_include));
			}

			let revision_1 =
				<MembersPallet as MembershipProver>::ring_revision(&identifier, 0).unwrap();
			assert!(revision_1 > revision_0);

			// Verify both revisions are valid before removal
			assert!(
				<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Old revision should be valid before ring removal"
			);
			assert!(
				<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_1),
				"Current revision should be valid before ring removal"
			);

			// Manually advance to the next ring to allow ring 0 to be removed
			CurrentRingIndex::<Test>::insert(identifier, 1u32);

			// Remove ring 0
			assert_ok!(<MembersPallet as AppendOnlyMembers>::remove_ring(&identifier, 0));

			// Verify both revisions are no longer valid after removal (instantly expired)
			assert!(
				!<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_0),
				"Old revision should NOT be valid after ring removal"
			);
			assert!(
				!<MembersPallet as MembershipProver>::is_revision_valid(&identifier, 0, revision_1),
				"Current revision should NOT be valid after ring removal"
			);
		});
	}
}

mod tag_collision_tests {
	use super::*;

	/// Compute the `provides` tag(s) a `build_ring_authorized` call would advertise to the pool,
	/// or `None` if `call` is not a `build_ring_authorized`.
	fn build_ring_tag(call: &RuntimeCall) -> Option<Vec<Vec<u8>>> {
		match call {
			RuntimeCall::MembersPallet(crate::Call::build_ring_authorized {
				identifier,
				ring_index,
				ring_exponent,
				revision,
				to_include,
				discriminator: _,
			}) => {
				let (valid, _) = MembersPallet::ensure_can_build_ring(
					identifier,
					*ring_index,
					*ring_exponent,
					*revision,
					*to_include,
				)
				.expect("the pooled build_ring transaction must validate");
				Some(valid.provides)
			},
			_ => None,
		}
	}

	/// Compute the `provides` tag(s) an `onboard_members_authorized` call would advertise, or
	/// `None` if `call` is not an `onboard_members_authorized`.
	fn onboard_members_tag(call: &RuntimeCall) -> Option<Vec<Vec<u8>>> {
		match call {
			RuntimeCall::MembersPallet(crate::Call::onboard_members_authorized {
				identifier,
				ring_index,
				head,
				first_member,
				discriminator: _,
			}) => {
				let (valid, _) = MembersPallet::ensure_can_onboard_members(
					identifier,
					*ring_index,
					*head,
					*first_member,
				)
				.expect("the pooled onboard_members transaction must validate");
				Some(valid.provides)
			},
			_ => None,
		}
	}

	/// Compute the `provides` tag(s) a `remove_suspended_keys_authorized` call would advertise, or
	/// `None` if `call` is not a `remove_suspended_keys_authorized`.
	fn remove_suspended_keys_tag(call: &RuntimeCall) -> Option<Vec<Vec<u8>>> {
		match call {
			RuntimeCall::MembersPallet(crate::Call::remove_suspended_keys_authorized {
				identifier,
				ring_index,
				revision,
				discriminator: _,
			}) => {
				let (valid, _) = MembersPallet::ensure_can_remove_suspended_keys(
					identifier,
					*ring_index,
					*revision,
				)
				.expect("the pooled remove_suspended_keys transaction must validate");
				Some(valid.provides)
			},
			_ => None,
		}
	}

	/// Whether `call` is a `build_ring_authorized` call that validates against the pool.
	fn is_build_ring_authorized(call: &RuntimeCall) -> bool {
		build_ring_tag(call).is_some()
	}

	/// Whether `call` is an `onboard_members_authorized` call that validates against the pool.
	fn is_onboard_members_authorized(call: &RuntimeCall) -> bool {
		onboard_members_tag(call).is_some()
	}

	/// Whether `call` is a `remove_suspended_keys_authorized` call that validates against the pool.
	fn is_remove_suspended_keys_authorized(call: &RuntimeCall) -> bool {
		remove_suspended_keys_tag(call).is_some()
	}

	fn queue_pending_suspensions(identifier: Identifier, members: &[MemberOf<Test>]) {
		assert_ok!(<MembersPallet as FlexibleMembers>::start_removal_session(&identifier));
		assert_ok!(<MembersPallet as FlexibleMembers>::remove_members(&identifier, members));
		assert_ok!(<MembersPallet as FlexibleMembers>::end_removal_session(&identifier));
		assert!(PendingSuspensions::<Test>::contains_key(identifier, 0));
	}

	/// Regression test for the tags of `build_ring_authorized` for the first and second build.
	///
	/// The tags must be different to avoid collision in the transaction pool.
	#[test]
	fn regression_build_ring_authorized_tag_distinct() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let id = TEST_IDENTIFIER;
			create_test_collection(id, 5);

			// Cohort 1: onboard members into ring 0. No `Root` exists yet, so the worker will
			// build it at the default revision 0.
			let _cohort1 = generate_members(id, 1, 10);
			assert_ok!(MembersPallet::onboard_members(&id, false));

			// Round 1: the offchain worker submits `build_ring_authorized` for ring 0. Capture the
			// call now, while no `Root` exists yet (this build will produce revision 0).
			run_offchain_worker(1);
			let round1 = inspect_pool_transactions();
			let call_round1 = round1
				.iter()
				.map(|e| &e.function)
				.find(|c| is_build_ring_authorized(c))
				.expect("round 1 build_ring present")
				.clone();
			// Capture the tag at this round's state (no `Root` yet, so it produces revision 0).
			let tag_round1 = build_ring_tag(&call_round1).unwrap();

			// Apply round 1: the `Root` for ring 0 is now created at revision 0.
			assert_ok!(MembersPallet::build_ring(&id, 0, 10));
			assert_eq!(Root::<Test>::get(id, 0).unwrap().revision, 0);

			// Cohort 2: more members onboarded into the SAME ring 0, which now needs a rebuild.
			let _cohort2 = generate_members(id, 11, 20);
			assert_ok!(MembersPallet::onboard_members(&id, false));

			// Round 2: the worker reads the just-built `Root` (still revision 0) and submits
			// another `build_ring` with revision 0. Capture the newly submitted transaction at
			// this round's state (a `Root` now exists, so this build will produce revision 1).
			let before = inspect_pool_transactions().len();
			run_offchain_worker(2);
			let round2 = inspect_pool_transactions();
			assert_eq!(
				round2.len(),
				before + 1,
				"a second build_ring was submitted for the rebuild"
			);
			let call_round2 = round2[before..]
				.iter()
				.map(|e| &e.function)
				.find(|c| is_build_ring_authorized(c))
				.expect("round 2 build_ring present")
				.clone();
			// Capture the tag at this round's state (a `Root` now exists, so it produces revision
			// 1).
			let tag_round2 = build_ring_tag(&call_round2).unwrap();

			// The encoded calls themselves must differ: a distinct tag alone is not enough, since
			// the pool also rejects a transaction whose encoding matches one already queued.
			assert_ne!(
				call_round1.encode(),
				call_round2.encode(),
				"build_ring call encoding must differ across cohorts, not just the tag",
			);

			// The rebuild also advertises a different `provides` tag (produced revision 1)
			// than the first build (produced revision 0), so the pool accepts both.
			assert_ne!(
				tag_round1, tag_round2,
				"build_ring tag must differ across cohorts once tagged by the produced revision",
			);
		});
	}

	/// Regression test for the tags of `onboard_members_authorized` across two onboarding rounds
	/// where a page drains while `head == tail` (so `head` stays put).
	///
	/// The tags must be different to avoid collision in the transaction pool. Each round's tag is
	/// computed at that round's state, since the head page's first key is read from storage.
	#[test]
	fn regression_onboard_members_authorized_tag_distinct() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let id = TEST_IDENTIFIER;
			create_test_collection(id, 5);

			// Round 1: a cohort sits on page 0 (head == tail == 0).
			let _cohort1 = generate_members(id, 1, 5);
			assert_eq!(QueuePageIndices::<Test>::get(id), (0, 0));

			run_offchain_worker(1);
			let round1 = inspect_pool_transactions();
			let call_round1 = round1
				.iter()
				.map(|e| &e.function)
				.find(|c| is_onboard_members_authorized(c))
				.expect("round 1 onboard_members present")
				.clone();
			// Capture the tag at this round's state (the head page's first key is read from
			// storage).
			let tag_round1 = onboard_members_tag(&call_round1).unwrap();

			// Apply round 1. The single page drains completely; because `head == tail`, the head
			// pointer is left at 0 rather than advanced.
			assert_ok!(MembersPallet::onboard_members(&id, true));
			assert_eq!(QueuePageIndices::<Test>::get(id), (0, 0));
			assert_eq!(OnboardingQueue::<Test>::decode_len(id, 0).unwrap_or(0), 0);

			// Round 2: new members queue onto the same page 0, so the next onboarding reuses head
			// 0.
			let _cohort2 = generate_members(id, 6, 10);
			assert_eq!(QueuePageIndices::<Test>::get(id), (0, 0));

			// (Round 2 also queues a build_ring for the now-stale ring; pick the onboard tx newly
			// added this round at this round's state.)
			let before = inspect_pool_transactions().len();
			run_offchain_worker(2);
			let round2 = inspect_pool_transactions();
			let call_round2 = round2[before..]
				.iter()
				.map(|e| &e.function)
				.find(|c| is_onboard_members_authorized(c))
				.expect("round 2 onboard_members present")
				.clone();
			// Capture the tag at this round's state (the head page's first key is read from
			// storage).
			let tag_round2 = onboard_members_tag(&call_round2).unwrap();

			// The encoded calls themselves must differ: a distinct tag alone is not enough, since
			// the pool also rejects a transaction whose encoding matches one already queued.
			assert_ne!(
				call_round1.encode(),
				call_round2.encode(),
				"onboard_members call encoding must differ across rounds, not just the tag",
			);

			// The head page's first key changed from cohort 1 to cohort 2, so the tags differ.
			assert_ne!(
				tag_round1, tag_round2,
				"onboard_members tag must differ once the head page's first key is included",
			);
		});
	}

	/// Regression test for the tags of `remove_suspended_keys_authorized` across two consecutive
	/// suspension sweeps on the same ring.
	///
	/// The tags must be different to avoid collision in the transaction pool. Each removal bumps
	/// the ring root revision, so each round's tag (read from the current `Root`) differs.
	#[test]
	fn regression_remove_suspended_keys_authorized_tag_distinct() {
		TestExt::new().execute_with(|| {
			System::set_block_number(1);
			let id = TEST_IDENTIFIER;
			create_test_collection(id, 5);

			// Populate and build ring 0 so members are fully included (Root at revision 0).
			let members = generate_members(id, 1, 20);
			assert_ok!(MembersPallet::onboard_members(&id, false));
			assert_ok!(MembersPallet::build_ring(&id, 0, 20));
			assert_eq!(Root::<Test>::get(id, 0).unwrap().revision, 0);

			// Sweep 1: suspend a few included members on ring 0, leaving pending suspensions.
			let batch1: Vec<_> = members[0..3].iter().map(|(m, _)| *m).collect();
			queue_pending_suspensions(id, &batch1);

			run_offchain_worker(1);
			let round1 = inspect_pool_transactions();
			let call_round1 = round1
				.iter()
				.map(|e| &e.function)
				.find(|c| is_remove_suspended_keys_authorized(c))
				.expect("round 1 remove_suspended_keys present")
				.clone();
			// Capture the tag at this round's state (read from the current `Root`, revision 0).
			let tag_round1 = remove_suspended_keys_tag(&call_round1).unwrap();

			// Apply sweep 1: clears the pending suspensions and bumps the root revision to 1.
			MembersPallet::remove_suspended_keys(&id, 0);
			assert!(!PendingSuspensions::<Test>::contains_key(id, 0));
			assert_eq!(Root::<Test>::get(id, 0).unwrap().revision, 1);

			// Sweep 2: suspend a different batch of still-included members on the same ring 0.
			let batch2: Vec<_> = members[5..8].iter().map(|(m, _)| *m).collect();
			queue_pending_suspensions(id, &batch2);

			// Pick the remove tx newly added this round at this round's state.
			let before = inspect_pool_transactions().len();
			run_offchain_worker(2);
			let round2 = inspect_pool_transactions();
			let call_round2 = round2[before..]
				.iter()
				.map(|e| &e.function)
				.find(|c| is_remove_suspended_keys_authorized(c))
				.expect("round 2 remove_suspended_keys present")
				.clone();
			// Capture the tag at this round's state (read from the current `Root`, revision 1).
			let tag_round2 = remove_suspended_keys_tag(&call_round2).unwrap();

			// The encoded calls themselves must differ: a distinct tag alone is not enough, since
			// the pool also rejects a transaction whose encoding matches one already queued.
			assert_ne!(
				call_round1.encode(),
				call_round2.encode(),
				"remove_suspended_keys call encoding must differ across sweeps, not just the tag",
			);

			// The root revision advanced from 0 to 1 between sweeps, so the tags differ.
			assert_ne!(
				tag_round1, tag_round2,
				"remove_suspended_keys tag must differ once the root revision is included",
			);
		});
	}

	#[test]
	fn remove_suspended_keys_with_no_revision_is_stale_when_root_exists() {
		TestExt::new().execute_with(|| {
			let id = TEST_IDENTIFIER;
			let members = setup_collection_with_built_ring(id, 20);
			assert_eq!(Root::<Test>::get(id, 0).unwrap().revision, 0);

			let batch: Vec<_> = members[0..3].iter().map(|(m, _)| *m).collect();
			queue_pending_suspensions(id, &batch);

			assert_eq!(
				MembersPallet::ensure_can_remove_suspended_keys(&id, 0, None),
				Err(InvalidTransaction::Stale.into())
			);
		});
	}

	#[test]
	fn remove_suspended_keys_with_revision_is_future_when_root_is_missing() {
		TestExt::new().execute_with(|| {
			let id = TEST_IDENTIFIER;
			create_test_collection(id, 5);
			let members = generate_members(id, 1, 20);
			assert_ok!(MembersPallet::onboard_members(&id, false));
			assert!(Root::<Test>::get(id, 0).is_none());

			let batch: Vec<_> = members[0..3].iter().map(|(m, _)| *m).collect();
			queue_pending_suspensions(id, &batch);

			assert_eq!(
				MembersPallet::ensure_can_remove_suspended_keys(&id, 0, Some(0)),
				Err(InvalidTransaction::Future.into())
			);
		});
	}
}
