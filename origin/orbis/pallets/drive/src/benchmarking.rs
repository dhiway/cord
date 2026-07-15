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

use super::*;
use alloc::{vec, vec::Vec};
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::traits::Hash as HashT;

const SEED: u32 = 0;

pub trait BenchmarkHelper {
	fn make_publishable(manifest: Commitment, provider: Commitment);
}

fn drive<T: Config>(owner: &T::AccountId, version: u64) -> T::Hash {
	let id = T::Hashing::hash_of(&(b"benchmark-drive", owner));
	let name: DriveNameOf<T> = b"benchmark".to_vec().try_into().expect("bounded name");
	let now = frame_system::Pallet::<T>::block_number();
	Drives::<T>::insert(
		id,
		DriveRecord {
			owner: owner.clone(),
			name,
			root_manifest: None,
			root_provider_commitment: None,
			version,
			status: DriveStatus::Active,
			created_at: now,
			updated_at: now,
		},
	);
	OwnerDrives::<T>::try_mutate(owner, |items| items.try_push(id)).expect("drive index has space");
	id
}

fn path_with_len(len: u32) -> DrivePath {
	if len == 1 {
		return b"/".to_vec().try_into().expect("root is bounded")
	}
	let components = (len + 63) / 64;
	let mut letters = len - components;
	let mut bytes = Vec::with_capacity(len as usize);
	for index in 0..components {
		bytes.push(b'/');
		let components_left = components - index - 1;
		let component_len = letters.saturating_sub(components_left).min(63);
		bytes.extend(core::iter::repeat_n(b'a', component_len as usize));
		letters -= component_len;
	}
	bytes.try_into().expect("bounded valid path")
}

fn parent_path(path: &DrivePath) -> Option<DrivePath> {
	if path.as_slice() == b"/" {
		return None
	}
	let last = path.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
	if last == 0 {
		Some(b"/".to_vec().try_into().expect("root is bounded"))
	} else {
		Some(path[..last].to_vec().try_into().expect("parent is bounded"))
	}
}

fn seed_parents<T: Config>(drive_id: T::Hash, target: &DrivePath, owner: &T::AccountId) {
	let mut parents = Vec::new();
	for (index, byte) in target.iter().enumerate().skip(1) {
		if *byte == b'/' {
			let parent: DrivePath = target[..index].to_vec().try_into().expect("bounded parent");
			parents.push(parent);
		}
	}
	let now = frame_system::Pallet::<T>::block_number();
	for parent in &parents {
		DriveNodes::<T>::insert(
			drive_id,
			parent,
			DriveNode {
				kind: NodeKind::Directory,
				manifest: None,
				provider_commitment: None,
				metadata: Default::default(),
				version: 1,
				updated_by: owner.clone(),
				updated_at: now,
			},
		);
		if let Some(grandparent) = parent_path(parent) {
			DriveChildCount::<T>::mutate(drive_id, grandparent, |count| {
				*count = count.saturating_add(1)
			});
		}
	}
	DriveNodeCount::<T>::insert(drive_id, parents.len() as u32);
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn create_drive(n: Linear<1, 256>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let name: DriveNameOf<T> = vec![b'a'; n as usize].try_into().expect("bounded name");
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), name);
	}

	#[benchmark]
	fn update_root(h: Linear<0, 63>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let drive_id = drive::<T>(&owner, 1);
		let history: DriveHistory = (0..h)
			.map(|i| RootVersion {
				manifest: Some([i as u8; 32]),
				provider_commitment: Some([i as u8; 32]),
				version: i as u64,
			})
			.collect::<Vec<_>>()
			.try_into()
			.expect("bounded history");
		RootHistory::<T>::insert(drive_id, history);
		let manifest = [0x42; 32];
		let provider = [0x24; 32];
		T::BenchmarkHelper::make_publishable(manifest, provider);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), drive_id, 1, None, manifest, provider);
	}

	#[benchmark]
	fn set_grant() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let subject: T::AccountId = account("subject", 1, SEED);
		let drive_id = drive::<T>(&owner, 1);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), drive_id, 1, subject, Some(DriveRole::Admin));
	}

	#[benchmark]
	fn transfer_drive() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let new_owner: T::AccountId = account("owner", 1, SEED);
		let drive_id = drive::<T>(&owner, 1);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), drive_id, 1, new_owner);
	}

	#[benchmark]
	fn archive_drive() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let drive_id = drive::<T>(&owner, 1);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), drive_id, 1);
	}

	#[benchmark]
	fn write_node_create(p: Linear<1, 4096>, m: Linear<0, 64>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let drive_id = drive::<T>(&owner, 1);
		let path = path_with_len(p);
		seed_parents::<T>(drive_id, &path, &owner);
		let metadata: Metadata = (0..m)
			.map(|i| MetadataEntry {
				key: vec![i as u8 + 1].try_into().unwrap(),
				value: vec![0; 256].try_into().unwrap(),
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		#[block]
		{
			assert_ok!(Pallet::<T>::write_node(
				RawOrigin::Signed(owner).into(),
				drive_id,
				1,
				path,
				NodeKind::Directory,
				None,
				None,
				metadata,
			));
		}
	}

	#[benchmark]
	fn write_node_update_file(p: Linear<1, 4096>, m: Linear<0, 64>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let drive_id = drive::<T>(&owner, 1);
		let path = path_with_len(p);
		seed_parents::<T>(drive_id, &path, &owner);
		let old_manifest = [1; 32];
		let new_manifest = [3; 32];
		let new_provider = [4; 32];
		T::BenchmarkHelper::make_publishable(new_manifest, new_provider);
		let now = frame_system::Pallet::<T>::block_number();
		DriveNodes::<T>::insert(
			drive_id,
			&path,
			DriveNode {
				kind: NodeKind::File,
				manifest: Some(old_manifest),
				provider_commitment: Some([2; 32]),
				metadata: Default::default(),
				version: 1,
				updated_by: owner.clone(),
				updated_at: now,
			},
		);
		DriveNodeCount::<T>::mutate(drive_id, |count| *count = count.saturating_add(1));
		if let Some(parent) = parent_path(&path) {
			DriveChildCount::<T>::mutate(drive_id, parent, |count| {
				*count = count.saturating_add(1)
			});
		}
		ActiveFileReferences::<T>::insert(old_manifest, 1);
		let metadata: Metadata = (0..m)
			.map(|i| MetadataEntry {
				key: vec![i as u8 + 1].try_into().unwrap(),
				value: vec![0; 256].try_into().unwrap(),
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		#[block]
		{
			assert_ok!(Pallet::<T>::write_node(
				RawOrigin::Signed(owner).into(),
				drive_id,
				1,
				path,
				NodeKind::File,
				Some(new_manifest),
				Some(new_provider),
				metadata,
			));
		}
	}

	#[benchmark]
	fn remove_node() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let drive_id = drive::<T>(&owner, 1);
		let target: DrivePath = b"/file".to_vec().try_into().unwrap();
		let now = frame_system::Pallet::<T>::block_number();
		DriveNodes::<T>::insert(
			drive_id,
			&target,
			DriveNode {
				kind: NodeKind::File,
				manifest: Some([1; 32]),
				provider_commitment: Some([2; 32]),
				metadata: Default::default(),
				version: 1,
				updated_by: owner.clone(),
				updated_at: now,
			},
		);
		DriveNodeCount::<T>::insert(drive_id, 1);
		DriveChildCount::<T>::insert(drive_id, parent_path(&target).unwrap(), 1);
		ActiveFileReferences::<T>::insert([1; 32], 1);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), drive_id, 1, target);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
