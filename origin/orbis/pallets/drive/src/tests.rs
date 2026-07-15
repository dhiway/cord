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
	mock::*, DriveChildCount, DriveNameOf, DriveNodeCount, DrivePath, DriveRole, Drives, Error,
	Metadata, MetadataEntry, NodeKind, Pallet, RootHistory,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_orbis_storage_control_primitives::CommitmentState;

fn name(value: &[u8]) -> DriveNameOf<Test> {
	value.to_vec().try_into().unwrap()
}

fn path(value: &[u8]) -> DrivePath {
	value.to_vec().try_into().unwrap()
}

fn create(owner: u64) -> <Test as frame_system::Config>::Hash {
	assert_ok!(Drive::create_drive(RuntimeOrigin::signed(owner), name(b"foundation")));
	Drives::<Test>::iter().next().unwrap().0
}

#[test]
fn names_and_paths_enforce_exact_byte_and_depth_boundaries() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::<Test>::validate_name(&vec![b'a'; 255]));
		assert_ok!(Pallet::<Test>::validate_name(&vec![b'a'; 256]));
		assert_noop!(
			Pallet::<Test>::validate_name(&vec![b'a'; 257]),
			Error::<Test>::DriveNameInvalid
		);
		for invalid in [b".".as_slice(), b"..", b"a/b", b"a\0b", &[0xff]] {
			assert_noop!(Pallet::<Test>::validate_name(invalid), Error::<Test>::DriveNameInvalid);
		}
		assert_ok!(Pallet::<Test>::validate_path(b"/a"));
		let depth_64 = format!("/{}", vec!["a"; 64].join("/"));
		let depth_65 = format!("/{}", vec!["a"; 65].join("/"));
		assert_ok!(Pallet::<Test>::validate_path(depth_64.as_bytes()));
		assert_noop!(
			Pallet::<Test>::validate_path(depth_65.as_bytes()),
			Error::<Test>::DepthExceeded
		);
		let path_4096 = format!("/{}", vec!["a".repeat(255); 16].join("/"));
		let mut path_4097 = path_4096.clone();
		path_4097.push(b'a' as char);
		assert_eq!(path_4096.len(), 4096);
		assert_ok!(Pallet::<Test>::validate_path(path_4096.as_bytes()));
		assert_noop!(
			Pallet::<Test>::validate_path(path_4097.as_bytes()),
			Error::<Test>::PathTooLong
		);
	});
}

#[test]
fn root_updates_are_optimistic_publishable_and_history_is_exactly_64() {
	new_test_ext().execute_with(|| {
		let drive = create(1);
		let missing = [1; 32];
		let provider = [9; 32];
		assert_noop!(
			Drive::update_root(RuntimeOrigin::signed(1), drive, 1, None, missing, provider),
			Error::<Test>::ManifestMissing
		);
		set_commitment(missing, CommitmentState::Tombstoned, provider);
		assert_noop!(
			Drive::update_root(RuntimeOrigin::signed(1), drive, 1, None, missing, provider),
			Error::<Test>::ManifestTombstoned
		);
		set_commitment(missing, CommitmentState::Publishable, provider);
		assert_noop!(
			Drive::update_root(RuntimeOrigin::signed(1), drive, 0, None, missing, provider),
			Error::<Test>::DriveVersionConflict
		);
		let mut previous = None;
		for index in 0..64u8 {
			let manifest = [index.saturating_add(2); 32];
			set_commitment(manifest, CommitmentState::Publishable, provider);
			assert_ok!(Drive::update_root(
				RuntimeOrigin::signed(1),
				drive,
				1 + index as u64,
				previous,
				manifest,
				provider,
			));
			previous = Some(manifest);
		}
		assert_eq!(RootHistory::<Test>::get(drive).len(), 64);
		let overflow = [99; 32];
		set_commitment(overflow, CommitmentState::Publishable, provider);
		assert_noop!(
			Drive::update_root(RuntimeOrigin::signed(1), drive, 65, previous, overflow, provider),
			Error::<Test>::RootHistoryFull
		);
		assert_eq!(Drives::<Test>::get(drive).unwrap().version, 65);
		let active_root = previous.unwrap();
		assert_eq!(Pallet::<Test>::active_root_references(&active_root), 1);
		assert_ok!(Drive::archive_drive(RuntimeOrigin::signed(1), drive, 65));
		assert_eq!(Pallet::<Test>::active_root_references(&active_root), 0);
	});
}

#[test]
fn tree_is_normalized_ordered_and_uses_writer_grants() {
	new_test_ext().execute_with(|| {
		let drive = create(1);
		let empty: Metadata = Default::default();
		assert_ok!(Drive::set_grant(
			RuntimeOrigin::signed(1),
			drive,
			1,
			2,
			Some(DriveRole::Writer)
		));
		assert_ok!(Drive::set_grant(
			RuntimeOrigin::signed(1),
			drive,
			2,
			3,
			Some(DriveRole::Reader)
		));
		assert_noop!(
			Drive::set_grant(RuntimeOrigin::signed(1), drive, 3, 4, Some(DriveRole::Reader)),
			Error::<Test>::GrantLimitReached
		);
		assert_ok!(Drive::write_node(
			RuntimeOrigin::signed(2),
			drive,
			3,
			path(b"/z"),
			NodeKind::Directory,
			None,
			None,
			empty.clone()
		));
		assert_ok!(Drive::write_node(
			RuntimeOrigin::signed(2),
			drive,
			4,
			path(b"/a"),
			NodeKind::Directory,
			None,
			None,
			empty.clone()
		));
		assert_eq!(DriveNodeCount::<Test>::get(drive), 2);
		assert_eq!(DriveChildCount::<Test>::get(drive, path(b"/")), 2);
		let manifest = [3; 32];
		let provider = [4; 32];
		set_commitment(manifest, CommitmentState::Publishable, provider);
		assert_ok!(Drive::write_node(
			RuntimeOrigin::signed(2),
			drive,
			5,
			path(b"/a/file"),
			NodeKind::File,
			Some(manifest),
			Some(provider),
			empty,
		));
		assert_noop!(
			Drive::remove_node(RuntimeOrigin::signed(2), drive, 6, path(b"/a")),
			Error::<Test>::DirectoryNotEmpty
		);
		assert_eq!(Pallet::<Test>::active_manifest_references(&manifest), 1);
		assert_ok!(Drive::transfer_drive(RuntimeOrigin::signed(1), drive, 6, 3));
		assert_eq!(Pallet::<Test>::active_manifest_references(&manifest), 1);
		assert_ok!(Drive::remove_node(RuntimeOrigin::signed(3), drive, 7, path(b"/a/file")));
		assert_eq!(Pallet::<Test>::active_manifest_references(&manifest), 0);
		assert_ok!(Drive::write_node(
			RuntimeOrigin::signed(3),
			drive,
			8,
			path(b"/a/file-2"),
			NodeKind::File,
			Some(manifest),
			Some(provider),
			Default::default(),
		));
		assert_eq!(Pallet::<Test>::active_manifest_references(&manifest), 1);
		assert_noop!(
			Drive::archive_drive(RuntimeOrigin::signed(3), drive, 9),
			Error::<Test>::DriveNotEmpty
		);
		assert_ok!(Drive::remove_node(RuntimeOrigin::signed(3), drive, 9, path(b"/a/file-2")));
		assert_eq!(Pallet::<Test>::active_manifest_references(&manifest), 0);
		assert_ok!(Drive::remove_node(RuntimeOrigin::signed(3), drive, 10, path(b"/a")));
		assert_ok!(Drive::remove_node(RuntimeOrigin::signed(3), drive, 11, path(b"/z")));
		assert_ok!(Drive::archive_drive(RuntimeOrigin::signed(3), drive, 12));
	});
}

#[test]
fn metadata_must_be_unique_and_raw_byte_sorted() {
	new_test_ext().execute_with(|| {
		let item = |key: &[u8]| MetadataEntry {
			key: key.to_vec().try_into().unwrap(),
			value: BoundedVec::default(),
		};
		let sorted: Metadata = vec![item(b"A"), item(b"a")].try_into().unwrap();
		let duplicate: Metadata = vec![item(b"a"), item(b"a")].try_into().unwrap();
		let reverse: Metadata = vec![item(b"b"), item(b"a")].try_into().unwrap();
		assert_ok!(Pallet::<Test>::validate_metadata(&sorted));
		assert_noop!(
			Pallet::<Test>::validate_metadata(&duplicate),
			Error::<Test>::MetadataOrderInvalid
		);
		assert_noop!(
			Pallet::<Test>::validate_metadata(&reverse),
			Error::<Test>::MetadataOrderInvalid
		);
	});
}

#[test]
fn weights_are_monotonic_at_contract_boundaries() {
	use crate::WeightInfo;
	assert!(
		<() as WeightInfo>::create_drive(256).ref_time() >
			<() as WeightInfo>::create_drive(255).ref_time()
	);
	assert!(
		<() as WeightInfo>::update_root(64).ref_time() >
			<() as WeightInfo>::update_root(63).ref_time()
	);
	assert!(Pallet::<Test>::write_node_weight(4096, 64)
		.all_gte(Pallet::<Test>::write_node_weight(4095, 64)));
	let create = <() as WeightInfo>::write_node_create(4096, 64);
	let update = <() as WeightInfo>::write_node_update_file(4096, 64);
	let dispatch = Pallet::<Test>::write_node_weight(4096, 64);
	assert!(dispatch.all_gte(create));
	assert!(dispatch.all_gte(update));
}
