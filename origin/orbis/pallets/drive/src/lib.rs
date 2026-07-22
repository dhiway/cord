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

//! Canonical Drive/File System control state for Origin Commons.
//!
//! Drive stores only bounded tree metadata and commitments. Application bytes and manifests remain
//! in the canonical storage plane selected by the runtime's [`CanonicalStorageControl`] adapter.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

pub use pallet::*;
pub use pallet_orbis_storage_control_primitives::{
	CanonicalStorageControl, Commitment, CommitmentState,
};
pub use weights::WeightInfo;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

pub type DrivePath = BoundedVec<u8, ConstU32<4096>>;
pub type MetadataKey = BoundedVec<u8, ConstU32<64>>;
pub type MetadataValue = BoundedVec<u8, ConstU32<256>>;
pub type Metadata = BoundedVec<MetadataEntry, ConstU32<64>>;
pub type DriveHistory = BoundedVec<RootVersion, ConstU32<64>>;

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct MetadataEntry {
	pub key: MetadataKey,
	pub value: MetadataValue,
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
pub enum NodeKind {
	Directory,
	File,
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
pub enum DriveRole {
	Reader,
	Writer,
	Admin,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DriveGrant<AccountId> {
	pub subject: AccountId,
	pub role: DriveRole,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DriveRecord<AccountId, Name, BlockNumber> {
	pub owner: AccountId,
	pub name: Name,
	pub root_manifest: Option<Commitment>,
	pub root_provider_commitment: Option<Commitment>,
	pub version: u64,
	pub status: DriveStatus,
	pub created_at: BlockNumber,
	pub updated_at: BlockNumber,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct RootVersion {
	pub manifest: Option<Commitment>,
	pub provider_commitment: Option<Commitment>,
	pub version: u64,
}

#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct DriveNode<AccountId, BlockNumber> {
	pub kind: NodeKind,
	pub manifest: Option<Commitment>,
	pub provider_commitment: Option<Commitment>,
	pub metadata: Metadata,
	pub version: u64,
	pub updated_by: AccountId,
	pub updated_at: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, transactional};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash as HashT;
	use unicode_normalization::UnicodeNormalization;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	const DRIVE_ID_DOMAIN: &[u8] = b"cord:commons:drive:v1";

	pub type DriveNameOf<T> = BoundedVec<u8, <T as Config>::MaxDriveNameBytes>;
	pub type DriveRecordOf<T> =
		DriveRecord<<T as frame_system::Config>::AccountId, DriveNameOf<T>, BlockNumberFor<T>>;
	pub type DriveNodeOf<T> = DriveNode<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;
	pub type DriveGrantsOf<T> = BoundedVec<
		DriveGrant<<T as frame_system::Config>::AccountId>,
		<T as Config>::MaxControllersPerDrive,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type StorageControl: CanonicalStorageControl;
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper;
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
	pub type DriveGrants<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, DriveGrantsOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type DriveNonce<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	#[pallet::storage]
	pub type RootHistory<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, DriveHistory, ValueQuery>;

	/// O(1) active-root reference count used by guarded S3 history pruning.
	#[pallet::storage]
	pub type ActiveRootReferences<T: Config> =
		StorageMap<_, Blake2_128Concat, Commitment, u32, ValueQuery>;

	/// O(1) active file-node reference count used by guarded S3 history pruning.
	#[pallet::storage]
	pub type ActiveFileReferences<T: Config> =
		StorageMap<_, Blake2_128Concat, Commitment, u32, ValueQuery>;

	#[pallet::storage]
	pub type DriveNodeCount<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, u32, ValueQuery>;

	#[pallet::storage]
	pub type DriveChildCount<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		DrivePath,
		u32,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type DriveNodes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		DrivePath,
		DriveNodeOf<T>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DriveCreated {
			drive_id: T::Hash,
			owner: T::AccountId,
			version: u64,
		},
		DriveRootUpdated {
			drive_id: T::Hash,
			previous_root: Option<Commitment>,
			new_root: Commitment,
			previous_version: u64,
			version: u64,
		},
		GrantChanged {
			drive_id: T::Hash,
			subject: T::AccountId,
			role: Option<DriveRole>,
			previous_version: u64,
			version: u64,
		},
		DriveTransferred {
			drive_id: T::Hash,
			old_owner: T::AccountId,
			new_owner: T::AccountId,
			previous_version: u64,
			version: u64,
		},
		DriveArchived {
			drive_id: T::Hash,
			previous_version: u64,
			version: u64,
		},
		NodeWritten {
			drive_id: T::Hash,
			path: DrivePath,
			kind: NodeKind,
			previous_version: u64,
			version: u64,
		},
		NodeRemoved {
			drive_id: T::Hash,
			path: DrivePath,
			previous_version: u64,
			version: u64,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		DriveNameInvalid,
		DriveNotFound,
		DriveAlreadyExists,
		DriveNotActive,
		NotDriveOwner,
		NotDriveWriter,
		DriveVersionConflict,
		PreviousRootMismatch,
		ManifestMissing,
		ManifestPending,
		ManifestTombstoned,
		ProviderCommitmentInvalid,
		OwnerDriveLimitReached,
		GrantLimitReached,
		GrantAlreadyExists,
		GrantNotFound,
		PathInvalid,
		PathTooLong,
		DepthExceeded,
		ParentNotFound,
		ParentNotDirectory,
		ChildLimitReached,
		NodeLimitReached,
		NodeNotFound,
		DirectoryNotEmpty,
		DriveNotEmpty,
		MetadataInvalid,
		MetadataOrderInvalid,
		RootHistoryFull,
		VersionOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_drive(name.len() as u32))]
		#[transactional]
		pub fn create_drive(origin: OriginFor<T>, name: DriveNameOf<T>) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::validate_name(&name)?;
			let nonce = DriveNonce::<T>::get(&owner);
			let genesis = frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::default());
			let drive_id = T::Hashing::hash_of(&(DRIVE_ID_DOMAIN, genesis, &owner, nonce));
			ensure!(!Drives::<T>::contains_key(drive_id), Error::<T>::DriveAlreadyExists);
			OwnerDrives::<T>::try_mutate(&owner, |ids| ids.try_push(drive_id))
				.map_err(|_| Error::<T>::OwnerDriveLimitReached)?;
			let now = frame_system::Pallet::<T>::block_number();
			Drives::<T>::insert(
				drive_id,
				DriveRecord {
					owner: owner.clone(),
					name,
					root_manifest: None,
					root_provider_commitment: None,
					version: 1,
					status: DriveStatus::Active,
					created_at: now,
					updated_at: now,
				},
			);
			DriveNonce::<T>::insert(&owner, nonce.saturating_add(1));
			Self::deposit_event(Event::DriveCreated { drive_id, owner, version: 1 });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_root(RootHistory::<T>::decode_len(drive_id).unwrap_or(0) as u32))]
		#[transactional]
		pub fn update_root(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			previous_root: Option<Commitment>,
			new_root: Commitment,
			provider_commitment: Commitment,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			Self::validate_commitment(&new_root, &provider_commitment)?;
			Drives::<T>::try_mutate(drive_id, |maybe| -> DispatchResult {
				let drive = maybe.as_mut().ok_or(Error::<T>::DriveNotFound)?;
				Self::ensure_active(drive)?;
				Self::ensure_writer(drive_id, &drive.owner, &caller)?;
				Self::ensure_version(drive, expected_version)?;
				ensure!(drive.root_manifest == previous_root, Error::<T>::PreviousRootMismatch);
				RootHistory::<T>::try_mutate(drive_id, |history| {
					history
						.try_push(RootVersion {
							manifest: drive.root_manifest,
							provider_commitment: drive.root_provider_commitment,
							version: drive.version,
						})
						.map_err(|_| Error::<T>::RootHistoryFull)
				})?;
				let prior = drive.version;
				if let Some(previous) = drive.root_manifest {
					ActiveRootReferences::<T>::mutate(previous, |count| {
						*count = count.saturating_sub(1)
					});
				}
				ActiveRootReferences::<T>::mutate(new_root, |count| {
					*count = count.saturating_add(1)
				});
				drive.root_manifest = Some(new_root);
				drive.root_provider_commitment = Some(provider_commitment);
				let version = Self::bump(drive)?;
				Self::deposit_event(Event::DriveRootUpdated {
					drive_id,
					previous_root,
					new_root,
					previous_version: prior,
					version,
				});
				Ok(())
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_grant())]
		pub fn set_grant(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			subject: T::AccountId,
			role: Option<DriveRole>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			Self::ensure_active(&drive)?;
			ensure!(drive.owner == owner, Error::<T>::NotDriveOwner);
			Self::ensure_version(&drive, expected_version)?;
			ensure!(subject != owner, Error::<T>::GrantAlreadyExists);
			DriveGrants::<T>::try_mutate(drive_id, |grants| -> DispatchResult {
				let encoded = subject.encode();
				let position = grants.binary_search_by(|item| item.subject.encode().cmp(&encoded));
				match (position, role) {
					(Ok(_), Some(_)) => return Err(Error::<T>::GrantAlreadyExists.into()),
					(Err(_), None) => return Err(Error::<T>::GrantNotFound.into()),
					(Ok(index), None) => {
						grants.remove(index);
					},
					(Err(index), Some(role)) => grants
						.try_insert(index, DriveGrant { subject: subject.clone(), role })
						.map_err(|_| Error::<T>::GrantLimitReached)?,
				}
				Ok(())
			})?;
			let prior = drive.version;
			let version = Self::bump(&mut drive)?;
			Drives::<T>::insert(drive_id, drive);
			Self::deposit_event(Event::GrantChanged {
				drive_id,
				subject,
				role,
				previous_version: prior,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::transfer_drive())]
		#[transactional]
		pub fn transfer_drive(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let old_owner = ensure_signed(origin)?;
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			Self::ensure_active(&drive)?;
			ensure!(drive.owner == old_owner, Error::<T>::NotDriveOwner);
			Self::ensure_version(&drive, expected_version)?;
			ensure!(old_owner != new_owner, Error::<T>::NotDriveOwner);
			OwnerDrives::<T>::try_mutate(&new_owner, |ids| ids.try_push(drive_id))
				.map_err(|_| Error::<T>::OwnerDriveLimitReached)?;
			OwnerDrives::<T>::mutate(&old_owner, |ids| {
				if let Some(index) = ids.iter().position(|id| id == &drive_id) {
					ids.remove(index);
				}
			});
			let prior = drive.version;
			drive.owner = new_owner.clone();
			let version = Self::bump(&mut drive)?;
			Drives::<T>::insert(drive_id, drive);
			DriveGrants::<T>::remove(drive_id);
			Self::deposit_event(Event::DriveTransferred {
				drive_id,
				old_owner,
				new_owner,
				previous_version: prior,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::archive_drive())]
		pub fn archive_drive(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			Self::ensure_active(&drive)?;
			ensure!(drive.owner == owner, Error::<T>::NotDriveOwner);
			Self::ensure_version(&drive, expected_version)?;
			ensure!(DriveNodeCount::<T>::get(drive_id) == 0, Error::<T>::DriveNotEmpty);
			let prior = drive.version;
			if let Some(root) = drive.root_manifest {
				ActiveRootReferences::<T>::mutate(root, |count| *count = count.saturating_sub(1));
			}
			drive.status = DriveStatus::Archived;
			let version = Self::bump(&mut drive)?;
			Drives::<T>::insert(drive_id, drive);
			DriveGrants::<T>::remove(drive_id);
			Self::deposit_event(Event::DriveArchived {
				drive_id,
				previous_version: prior,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(Pallet::<T>::write_node_weight(
			path.len() as u32,
			metadata.len() as u32,
		))]
		#[transactional]
		pub fn write_node(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			path: DrivePath,
			kind: NodeKind,
			manifest: Option<Commitment>,
			provider_commitment: Option<Commitment>,
			metadata: Metadata,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::validate_path(&path)?;
			Self::validate_metadata(&metadata)?;
			Self::validate_optional_commitment(manifest.as_ref(), provider_commitment.as_ref())?;
			ensure!(kind == NodeKind::File || manifest.is_none(), Error::<T>::ManifestMissing);
			ensure!(kind == NodeKind::Directory || manifest.is_some(), Error::<T>::ManifestMissing);
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			Self::ensure_active(&drive)?;
			Self::ensure_writer(drive_id, &drive.owner, &who)?;
			Self::ensure_version(&drive, expected_version)?;
			Self::ensure_parent(drive_id, &path)?;
			let current = DriveNodes::<T>::get(drive_id, &path);
			if current.is_none() {
				Self::ensure_child_capacity(drive_id, &path)?;
				DriveNodeCount::<T>::try_mutate(drive_id, |count| -> DispatchResult {
					ensure!(*count < 4096, Error::<T>::NodeLimitReached);
					*count = count.saturating_add(1);
					Ok(())
				})?;
				if let Some(parent) = Self::parent_path(&path) {
					DriveChildCount::<T>::mutate(drive_id, parent, |count| {
						*count = count.saturating_add(1)
					});
				}
			}
			let prior = drive.version;
			if let Some(previous) = current.as_ref().and_then(|node| node.manifest) {
				ActiveFileReferences::<T>::mutate(previous, |count| {
					*count = count.saturating_sub(1)
				});
			}
			if let Some(next) = manifest {
				ActiveFileReferences::<T>::mutate(next, |count| *count = count.saturating_add(1));
			}
			let version = Self::bump(&mut drive)?;
			DriveNodes::<T>::insert(
				drive_id,
				&path,
				DriveNode {
					kind,
					manifest,
					provider_commitment,
					metadata,
					version,
					updated_by: who,
					updated_at: frame_system::Pallet::<T>::block_number(),
				},
			);
			Drives::<T>::insert(drive_id, drive);
			Self::deposit_event(Event::NodeWritten {
				drive_id,
				path,
				kind,
				previous_version: prior,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::remove_node())]
		#[transactional]
		pub fn remove_node(
			origin: OriginFor<T>,
			drive_id: T::Hash,
			expected_version: u64,
			path: DrivePath,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::validate_path(&path)?;
			let mut drive = Drives::<T>::get(drive_id).ok_or(Error::<T>::DriveNotFound)?;
			Self::ensure_active(&drive)?;
			Self::ensure_writer(drive_id, &drive.owner, &who)?;
			Self::ensure_version(&drive, expected_version)?;
			let node = DriveNodes::<T>::get(drive_id, &path).ok_or(Error::<T>::NodeNotFound)?;
			ensure!(DriveChildCount::<T>::get(drive_id, &path) == 0, Error::<T>::DirectoryNotEmpty);
			DriveNodes::<T>::remove(drive_id, &path);
			if let Some(manifest) = node.manifest {
				ActiveFileReferences::<T>::mutate(manifest, |count| {
					*count = count.saturating_sub(1)
				});
			}
			DriveNodeCount::<T>::mutate(drive_id, |count| *count = count.saturating_sub(1));
			if let Some(parent) = Self::parent_path(&path) {
				DriveChildCount::<T>::mutate(drive_id, parent, |count| {
					*count = count.saturating_sub(1)
				});
			}
			DriveChildCount::<T>::remove(drive_id, &path);
			let prior = drive.version;
			let version = Self::bump(&mut drive)?;
			Drives::<T>::insert(drive_id, drive);
			Self::deposit_event(Event::NodeRemoved {
				drive_id,
				path,
				previous_version: prior,
				version,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn write_node_weight(
			path_bytes: u32,
			metadata_items: u32,
		) -> frame_support::weights::Weight {
			T::WeightInfo::write_node_create(path_bytes, metadata_items)
				.max(T::WeightInfo::write_node_update_file(path_bytes, metadata_items))
		}

		pub fn active_root_references(manifest: &Commitment) -> u32 {
			ActiveRootReferences::<T>::get(manifest)
		}

		pub fn active_manifest_references(manifest: &Commitment) -> u32 {
			ActiveRootReferences::<T>::get(manifest)
				.saturating_add(ActiveFileReferences::<T>::get(manifest))
		}

		pub fn validate_name(name: &[u8]) -> DispatchResult {
			ensure!(!name.is_empty() && name.len() <= 256, Error::<T>::DriveNameInvalid);
			ensure!(!name.contains(&b'/') && !name.contains(&0), Error::<T>::DriveNameInvalid);
			ensure!(name != b"." && name != b"..", Error::<T>::DriveNameInvalid);
			let text = core::str::from_utf8(name).map_err(|_| Error::<T>::DriveNameInvalid)?;
			ensure!(text.nfc().eq(text.chars()), Error::<T>::DriveNameInvalid);
			Ok(())
		}

		pub fn validate_path(path: &[u8]) -> DispatchResult {
			let bytes = path;
			ensure!(!bytes.is_empty(), Error::<T>::PathInvalid);
			ensure!(bytes.len() <= 4096, Error::<T>::PathTooLong);
			ensure!(bytes[0] == b'/', Error::<T>::PathInvalid);
			if bytes == b"/" {
				return Ok(());
			}
			ensure!(bytes.last() != Some(&b'/'), Error::<T>::PathInvalid);
			let mut depth = 0usize;
			for component in bytes[1..].split(|byte| *byte == b'/') {
				ensure!(!component.is_empty(), Error::<T>::PathInvalid);
				Self::validate_name(component).map_err(|_| Error::<T>::PathInvalid)?;
				depth += 1;
			}
			ensure!(depth <= 64, Error::<T>::DepthExceeded);
			Ok(())
		}

		pub fn validate_metadata(metadata: &Metadata) -> DispatchResult {
			let mut previous: Option<&[u8]> = None;
			for entry in metadata {
				ensure!(!entry.key.is_empty(), Error::<T>::MetadataInvalid);
				ensure!(!entry.key.contains(&0), Error::<T>::MetadataInvalid);
				if let Some(previous) = previous {
					ensure!(previous < entry.key.as_slice(), Error::<T>::MetadataOrderInvalid);
				}
				previous = Some(entry.key.as_slice());
			}
			Ok(())
		}

		fn validate_optional_commitment(
			manifest: Option<&Commitment>,
			provider: Option<&Commitment>,
		) -> DispatchResult {
			match (manifest, provider) {
				(None, None) => Ok(()),
				(Some(manifest), Some(provider)) => Self::validate_commitment(manifest, provider),
				_ => Err(Error::<T>::ProviderCommitmentInvalid.into()),
			}
		}

		fn validate_commitment(
			manifest: &Commitment,
			provider_commitment: &Commitment,
		) -> DispatchResult {
			match T::StorageControl::manifest_state(manifest) {
				CommitmentState::Publishable => {},
				CommitmentState::Pending => return Err(Error::<T>::ManifestPending.into()),
				CommitmentState::Tombstoned => return Err(Error::<T>::ManifestTombstoned.into()),
				CommitmentState::Missing => return Err(Error::<T>::ManifestMissing.into()),
			}
			ensure!(
				T::StorageControl::provider_commitment_matches(manifest, provider_commitment),
				Error::<T>::ProviderCommitmentInvalid
			);
			Ok(())
		}

		fn ensure_active(drive: &DriveRecordOf<T>) -> DispatchResult {
			ensure!(drive.status == DriveStatus::Active, Error::<T>::DriveNotActive);
			Ok(())
		}

		fn ensure_version(drive: &DriveRecordOf<T>, expected: u64) -> DispatchResult {
			ensure!(drive.version == expected, Error::<T>::DriveVersionConflict);
			Ok(())
		}

		fn ensure_writer(
			drive_id: T::Hash,
			owner: &T::AccountId,
			caller: &T::AccountId,
		) -> DispatchResult {
			if caller == owner {
				return Ok(());
			}
			let authorized = DriveGrants::<T>::get(drive_id).iter().any(|grant| {
				grant.subject == *caller
					&& matches!(grant.role, DriveRole::Writer | DriveRole::Admin)
			});
			ensure!(authorized, Error::<T>::NotDriveWriter);
			Ok(())
		}

		fn parent_path(path: &DrivePath) -> Option<DrivePath> {
			if path.as_slice() == b"/" {
				return None;
			}
			let last = path.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
			if last == 0 {
				Some(b"/".to_vec().try_into().expect("root is bounded"))
			} else {
				Some(path[..last].to_vec().try_into().expect("parent shorter than path"))
			}
		}

		fn ensure_parent(drive_id: T::Hash, path: &DrivePath) -> DispatchResult {
			if let Some(parent) = Self::parent_path(path) {
				if parent.as_slice() == b"/" && !DriveNodes::<T>::contains_key(drive_id, &parent) {
					return Ok(());
				}
				let node =
					DriveNodes::<T>::get(drive_id, parent).ok_or(Error::<T>::ParentNotFound)?;
				ensure!(node.kind == NodeKind::Directory, Error::<T>::ParentNotDirectory);
			}
			Ok(())
		}

		fn ensure_child_capacity(drive_id: T::Hash, path: &DrivePath) -> DispatchResult {
			let Some(parent) = Self::parent_path(path) else { return Ok(()) };
			ensure!(
				DriveChildCount::<T>::get(drive_id, parent) < 1024,
				Error::<T>::ChildLimitReached
			);
			Ok(())
		}

		fn bump(drive: &mut DriveRecordOf<T>) -> Result<u64, DispatchError> {
			drive.version = drive.version.checked_add(1).ok_or(Error::<T>::VersionOverflow)?;
			drive.updated_at = frame_system::Pallet::<T>::block_number();
			Ok(drive.version)
		}
	}
}
