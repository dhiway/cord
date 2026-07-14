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

//! Native Orbis drive registry.
//!
//! A drive records ownership and a reference to a root already committed in
//! `TransactionStorage`. Directory trees, file bytes, encryption and transport stay off-chain.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub type ContentHash = [u8; 32];

/// Runtime adapter proving that a hash belongs to the canonical content ledger.
pub trait StorageReferenceValidator {
	fn contains(content_hash: &ContentHash) -> bool;
}

impl StorageReferenceValidator for () {
	fn contains(_: &ContentHash) -> bool {
		false
	}
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum DriveStatus {
	Active,
	Archived,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DriveRecord<AccountId, Name, BlockNumber> {
	pub owner: AccountId,
	pub name: Name,
	pub root_storage_ref: Option<ContentHash>,
	pub version: u64,
	pub status: DriveStatus,
	pub created_at: BlockNumber,
	pub updated_at: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, transactional};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash as HashT;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub type DriveNameOf<T> = BoundedVec<u8, <T as Config>::MaxDriveNameBytes>;
	pub type DriveRecordOf<T> =
		DriveRecord<<T as frame_system::Config>::AccountId, DriveNameOf<T>, BlockNumberFor<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type StorageLedger: StorageReferenceValidator;
		#[pallet::constant]
		type MaxDriveNameBytes: Get<u32>;
		#[pallet::constant]
		type MaxDrivesPerOwner: Get<u32>;
		#[pallet::constant]
		type MaxControllersPerDrive: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Drives<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, DriveRecordOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type OwnerDrives<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxDrivesPerOwner>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type DriveControllers<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::Hash,
		BoundedVec<T::AccountId, T::MaxControllersPerDrive>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type DriveNonce<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DriveCreated { drive_id: T::Hash, owner: T::AccountId },
		DriveRootUpdated { drive_id: T::Hash, version: u64 },
		ControllerChanged { drive_id: T::Hash, controller: T::AccountId, enabled: bool },
		DriveTransferred { drive_id: T::Hash, old_owner: T::AccountId, new_owner: T::AccountId },
		DriveArchived { drive_id: T::Hash },
	}

	#[pallet::error]
	pub enum Error<T> {
		EmptyName,
		DriveNotFound,
		DriveAlreadyExists,
		DriveNotActive,
		NotDriveOwner,
		NotDriveController,
		StaleVersion,
		InvalidStorageReference,
		OwnerDriveLimitReached,
		ControllerLimitReached,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_drive())]
		#[transactional]
		pub fn create_drive(
			origin: OriginFor<T>,
			name: DriveNameOf<T>,
			root_storage_ref: Option<ContentHash>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			ensure!(!name.is_empty(), Error::<T>::EmptyName);
			Self::validate_storage_ref(root_storage_ref.as_ref())?;
			let nonce = DriveNonce::<T>::get(&owner);
			let genesis = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::default());
			let drive_id = T::Hashing::hash_of(&(b"orbis/drive/v1", genesis, &owner, nonce));
			ensure!(!Drives::<T>::contains_key(drive_id), Error::<T>::DriveAlreadyExists);
			OwnerDrives::<T>::try_mutate(&owner, |ids| ids.try_push(drive_id))
				.map_err(|_| Error::<T>::OwnerDriveLimitReached)?;
			let now = frame_system::Pallet::<T>::block_number();
			Drives::<T>::insert(
				drive_id,
				DriveRecord {
					owner: owner.clone(),
					name,
					root_storage_ref,
					version: 1,
					status: DriveStatus::Active,
					created_at: now,
					updated_at: now,
				},
			);
			DriveNonce::<T>::insert(&owner, nonce.saturating_add(1));
			Self::deposit_event(Event::DriveCreated { drive_id, owner });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_root())]
		pub fn update_root(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			root_storage_ref: Option<ContentHash>,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			Self::validate_storage_ref(root_storage_ref.as_ref())?;
			Drives::<T>::try_mutate(drive_id, |maybe| -> DispatchResult {
				let drive = maybe.as_mut().ok_or(Error::<T>::DriveNotFound)?;
				ensure!(drive.status == DriveStatus::Active, Error::<T>::DriveNotActive);
				Self::ensure_controller(drive_id, &drive.owner, &caller)?;
				ensure!(drive.version == expected_version, Error::<T>::StaleVersion);
				drive.root_storage_ref = root_storage_ref;
				drive.version = drive.version.saturating_add(1);
				drive.updated_at = frame_system::Pallet::<T>::block_number();
				Self::deposit_event(Event::DriveRootUpdated { drive_id, version: drive.version });
				Ok(())
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_controller())]
		pub fn set_controller(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			controller: T::AccountId,
			enabled: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			ensure!(drive.owner == owner, Error::<T>::NotDriveOwner);
			ensure!(drive.status == DriveStatus::Active, Error::<T>::DriveNotActive);
			DriveControllers::<T>::try_mutate(drive_id, |controllers| -> DispatchResult {
				if enabled {
					if !controllers.contains(&controller) {
						controllers
							.try_push(controller.clone())
							.map_err(|_| Error::<T>::ControllerLimitReached)?;
					}
				} else if let Some(index) = controllers.iter().position(|who| who == &controller) {
					controllers.swap_remove(index);
				}
				Ok(())
			})?;
			Self::deposit_event(Event::ControllerChanged { drive_id, controller, enabled });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::transfer_drive())]
		#[transactional]
		pub fn transfer_drive(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let old_owner = ensure_signed(origin)?;
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			ensure!(drive.owner == old_owner, Error::<T>::NotDriveOwner);
			ensure!(drive.status == DriveStatus::Active, Error::<T>::DriveNotActive);
			OwnerDrives::<T>::try_mutate(&new_owner, |ids| ids.try_push(drive_id))
				.map_err(|_| Error::<T>::OwnerDriveLimitReached)?;
			OwnerDrives::<T>::mutate(&old_owner, |ids| {
				if let Some(index) = ids.iter().position(|id| id == &drive_id) {
					ids.swap_remove(index);
				}
			});
			drive.owner = new_owner.clone();
			drive.updated_at = frame_system::Pallet::<T>::block_number();
			Drives::<T>::insert(drive_id, drive);
			DriveControllers::<T>::remove(drive_id);
			Self::deposit_event(Event::DriveTransferred { drive_id, old_owner, new_owner });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::archive_drive())]
		pub fn archive_drive(origin: OriginFor<T>, drive_id: T::Hash) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Drives::<T>::try_mutate(drive_id, |maybe| -> DispatchResult {
				let drive = maybe.as_mut().ok_or(Error::<T>::DriveNotFound)?;
				ensure!(drive.owner == owner, Error::<T>::NotDriveOwner);
				ensure!(drive.status == DriveStatus::Active, Error::<T>::DriveNotActive);
				drive.status = DriveStatus::Archived;
				drive.updated_at = frame_system::Pallet::<T>::block_number();
				Ok(())
			})?;
			DriveControllers::<T>::remove(drive_id);
			Self::deposit_event(Event::DriveArchived { drive_id });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn validate_storage_ref(reference: Option<&ContentHash>) -> DispatchResult {
			if let Some(reference) = reference {
				ensure!(T::StorageLedger::contains(reference), Error::<T>::InvalidStorageReference);
			}
			Ok(())
		}

		fn ensure_controller(
			drive_id: T::Hash,
			owner: &T::AccountId,
			caller: &T::AccountId,
		) -> DispatchResult {
			ensure!(
				caller == owner || DriveControllers::<T>::get(drive_id).contains(caller),
				Error::<T>::NotDriveController
			);
			Ok(())
		}
	}
}
