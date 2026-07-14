// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native S3-style bucket and object metadata for Orbis.
//!
//! Object bodies remain exclusively in canonical TransactionStorage. This pallet stores no bytes,
//! CID, private data, or independent content availability state: every object version is only a
//! validated reference to a canonical TransactionStorage content hash.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::*, traits::StorageVersion, transactional, BoundedVec, CloneNoBound,
	DebugNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::Hash as HashT;
pub use weights::WeightInfo;

pub type ContentHash = [u8; 32];
const BUCKET_ID_DOMAIN: &[u8] = b"cord:orbis:s3:bucket:v1";
const OBJECT_ID_DOMAIN: &[u8] = b"cord:orbis:s3:object:v1";

/// Runtime adapter that validates a reference against canonical TransactionStorage state.
pub trait ContentHashValidator {
	fn exists(content_hash: &ContentHash) -> bool;
}

impl ContentHashValidator for () {
	fn exists(_: &ContentHash) -> bool {
		false
	}
}

pub type BucketNameOf<T> = BoundedVec<u8, <T as Config>::MaxBucketNameLen>;
pub type ObjectKeyOf<T> = BoundedVec<u8, <T as Config>::MaxObjectKeyLen>;
pub type ControllersOf<T> =
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxControllers>;
pub type OwnerBucketIndexOf<T> =
	BoundedVec<<T as frame_system::Config>::Hash, <T as Config>::MaxBucketsPerOwner>;
pub type BucketObjectIndexOf<T> = BoundedVec<ObjectKeyOf<T>, <T as Config>::MaxObjectsPerBucket>;
pub type ObjectHistoryOf<T> = BoundedVec<ObjectVersion<T>, <T as Config>::MaxObjectVersions>;

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Default,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum BucketStatus {
	#[default]
	Active,
	Archived,
	Deleted,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct BucketRecord<T: Config> {
	pub name: BucketNameOf<T>,
	pub owner: T::AccountId,
	pub controllers: ControllersOf<T>,
	pub status: BucketStatus,
	pub versioning_enabled: bool,
	pub version: u64,
	pub live_objects: u32,
	pub created_at: BlockNumberFor<T>,
	pub updated_at: BlockNumberFor<T>,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct ObjectRecord<T: Config> {
	pub object_id: T::Hash,
	pub content_hash: Option<ContentHash>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: T::AccountId,
	pub updated_at: BlockNumberFor<T>,
}

/// A bounded historical S3 version. Its content hash is a reference, never stored content.
#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct ObjectVersion<T: Config> {
	pub content_hash: Option<ContentHash>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: T::AccountId,
	pub updated_at: BlockNumberFor<T>,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Adapter to canonical TransactionStorage content-hash lookup.
		type ContentValidator: ContentHashValidator;

		#[pallet::constant]
		type MaxBucketNameLen: Get<u32>;
		#[pallet::constant]
		type MaxObjectKeyLen: Get<u32>;
		#[pallet::constant]
		type MaxControllers: Get<u32>;
		#[pallet::constant]
		type MaxBucketsPerOwner: Get<u32>;
		#[pallet::constant]
		type MaxObjectsPerBucket: Get<u32>;
		#[pallet::constant]
		type MaxObjectVersions: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Buckets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, BucketRecord<T>, OptionQuery>;

	/// Globally reserved validated name -> deterministic bucket ID. Deleted names remain reserved.
	#[pallet::storage]
	pub type BucketByName<T: Config> =
		StorageMap<_, Blake2_128Concat, BucketNameOf<T>, T::Hash, OptionQuery>;

	#[pallet::storage]
	pub type OwnerBuckets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, OwnerBucketIndexOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type BucketObjectKeys<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, BucketObjectIndexOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type Objects<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		ObjectKeyOf<T>,
		ObjectRecord<T>,
		OptionQuery,
	>;

	/// Bounded previous versions, populated only while bucket versioning is enabled.
	#[pallet::storage]
	pub type ObjectHistory<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		ObjectKeyOf<T>,
		ObjectHistoryOf<T>,
		ValueQuery,
	>;

	/// Empty clean-network genesis; only the storage-version marker is written.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _phantom: core::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { _phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			STORAGE_VERSION.put::<Pallet<T>>();
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BucketCreated {
			bucket: T::Hash,
			name: BucketNameOf<T>,
			owner: T::AccountId,
		},
		ControllerChanged {
			bucket: T::Hash,
			controller: T::AccountId,
			enabled: bool,
			version: u64,
		},
		BucketTransferred {
			bucket: T::Hash,
			from: T::AccountId,
			to: T::AccountId,
			version: u64,
		},
		BucketArchived {
			bucket: T::Hash,
			archived: bool,
			version: u64,
		},
		BucketVersioningChanged {
			bucket: T::Hash,
			enabled: bool,
			version: u64,
		},
		ObjectPut {
			bucket: T::Hash,
			object: T::Hash,
			key: ObjectKeyOf<T>,
			content_hash: ContentHash,
			version: u64,
		},
		ObjectDeleted {
			bucket: T::Hash,
			object: T::Hash,
			key: ObjectKeyOf<T>,
			version: u64,
		},
		BucketDeleted {
			bucket: T::Hash,
			name: BucketNameOf<T>,
			owner: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		EmptyBucketName,
		InvalidBucketName,
		BucketNameTaken,
		BucketAlreadyExists,
		BucketNotFound,
		BucketArchived,
		BucketDeleted,
		BucketNotEmpty,
		NotBucketOwner,
		NotBucketController,
		ControllerAlreadySet,
		ControllerNotFound,
		ControllerIndexFull,
		OwnerBucketIndexFull,
		BucketObjectIndexFull,
		EmptyObjectKey,
		ContentNotFound,
		ObjectNotFound,
		ObjectAlreadyExists,
		ObjectAlreadyDeleted,
		ObjectVersionMismatch,
		BucketVersionMismatch,
		ObjectVersionOverflow,
		BucketVersionOverflow,
		ObjectHistoryFull,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_bucket(name.len() as u32))]
		#[transactional]
		pub fn create_bucket(origin: OriginFor<T>, name: BucketNameOf<T>) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::validate_bucket_name(&name)?;
			ensure!(!BucketByName::<T>::contains_key(&name), Error::<T>::BucketNameTaken);
			let bucket = Self::bucket_id(&owner, &name);
			ensure!(!Buckets::<T>::contains_key(bucket), Error::<T>::BucketAlreadyExists);
			OwnerBuckets::<T>::try_mutate(&owner, |items| {
				items.try_push(bucket).map_err(|_| Error::<T>::OwnerBucketIndexFull)
			})?;
			let now = frame_system::Pallet::<T>::block_number();
			Buckets::<T>::insert(
				bucket,
				BucketRecord::<T> {
					name: name.clone(),
					owner: owner.clone(),
					controllers: Default::default(),
					status: BucketStatus::Active,
					versioning_enabled: false,
					version: 1,
					live_objects: 0,
					created_at: now,
					updated_at: now,
				},
			);
			BucketByName::<T>::insert(&name, bucket);
			Self::deposit_event(Event::BucketCreated { bucket, name, owner });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_controller())]
		#[transactional]
		pub fn set_controller(
			origin: OriginFor<T>,
			bucket: T::Hash,
			expected_bucket_version: u64,
			controller: T::AccountId,
			enabled: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let version =
				Buckets::<T>::try_mutate(bucket, |entry| -> Result<u64, DispatchError> {
					let record = entry.as_mut().ok_or(Error::<T>::BucketNotFound)?;
					Self::ensure_bucket_live(record)?;
					ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
					Self::ensure_bucket_version(record, expected_bucket_version)?;
					ensure!(controller != record.owner, Error::<T>::ControllerAlreadySet);
					if enabled {
						ensure!(
							!record.controllers.contains(&controller),
							Error::<T>::ControllerAlreadySet
						);
						record
							.controllers
							.try_push(controller.clone())
							.map_err(|_| Error::<T>::ControllerIndexFull)?;
					} else {
						let index = record
							.controllers
							.iter()
							.position(|item| item == &controller)
							.ok_or(Error::<T>::ControllerNotFound)?;
						record.controllers.swap_remove(index);
					}
					Self::bump_bucket(record)
				})?;
			Self::deposit_event(Event::ControllerChanged { bucket, controller, enabled, version });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::transfer_bucket())]
		#[transactional]
		pub fn transfer_bucket(
			origin: OriginFor<T>,
			bucket: T::Hash,
			expected_bucket_version: u64,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			ensure!(owner != new_owner, Error::<T>::NotBucketOwner);
			let mut record = Buckets::<T>::get(bucket).ok_or(Error::<T>::BucketNotFound)?;
			Self::ensure_bucket_live(&record)?;
			ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
			Self::ensure_bucket_version(&record, expected_bucket_version)?;
			OwnerBuckets::<T>::try_mutate(&new_owner, |items| {
				items.try_push(bucket).map_err(|_| Error::<T>::OwnerBucketIndexFull)
			})?;
			OwnerBuckets::<T>::mutate(&owner, |items| {
				if let Some(index) = items.iter().position(|id| id == &bucket) {
					items.swap_remove(index);
				}
			});
			record.owner = new_owner.clone();
			record.controllers = Default::default();
			let version = Self::bump_bucket(&mut record)?;
			Buckets::<T>::insert(bucket, record);
			Self::deposit_event(Event::BucketTransferred {
				bucket,
				from: owner,
				to: new_owner,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::set_archived())]
		pub fn set_archived(
			origin: OriginFor<T>,
			bucket: T::Hash,
			expected_bucket_version: u64,
			archived: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let version =
				Buckets::<T>::try_mutate(bucket, |entry| -> Result<u64, DispatchError> {
					let record = entry.as_mut().ok_or(Error::<T>::BucketNotFound)?;
					Self::ensure_bucket_live(record)?;
					ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
					Self::ensure_bucket_version(record, expected_bucket_version)?;
					record.status =
						if archived { BucketStatus::Archived } else { BucketStatus::Active };
					Self::bump_bucket(record)
				})?;
			Self::deposit_event(Event::BucketArchived { bucket, archived, version });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_versioning())]
		pub fn set_versioning(
			origin: OriginFor<T>,
			bucket: T::Hash,
			expected_bucket_version: u64,
			enabled: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let version =
				Buckets::<T>::try_mutate(bucket, |entry| -> Result<u64, DispatchError> {
					let record = entry.as_mut().ok_or(Error::<T>::BucketNotFound)?;
					Self::ensure_bucket_live(record)?;
					ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
					Self::ensure_bucket_version(record, expected_bucket_version)?;
					record.versioning_enabled = enabled;
					Self::bump_bucket(record)
				})?;
			Self::deposit_event(Event::BucketVersioningChanged { bucket, enabled, version });
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::put_object(key.len() as u32, ObjectHistory::<T>::decode_len(bucket, &key).unwrap_or(0) as u32))]
		#[transactional]
		pub fn put_object(
			origin: OriginFor<T>,
			bucket: T::Hash,
			key: ObjectKeyOf<T>,
			content_hash: ContentHash,
			expected_object_version: Option<u64>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!key.is_empty(), Error::<T>::EmptyObjectKey);
			ensure!(T::ContentValidator::exists(&content_hash), Error::<T>::ContentNotFound);
			let mut bucket_record = Buckets::<T>::get(bucket).ok_or(Error::<T>::BucketNotFound)?;
			Self::ensure_bucket_active(&bucket_record)?;
			Self::ensure_controller(&bucket_record, &who)?;
			let current = Objects::<T>::get(bucket, &key);
			match (&current, expected_object_version) {
				(None, None) => {},
				(None, Some(_)) => return Err(Error::<T>::ObjectNotFound.into()),
				(Some(_), None) => return Err(Error::<T>::ObjectAlreadyExists.into()),
				(Some(record), Some(expected)) => {
					ensure!(record.version == expected, Error::<T>::ObjectVersionMismatch)
				},
			}
			if bucket_record.versioning_enabled {
				if let Some(record) = current.as_ref() {
					ObjectHistory::<T>::try_mutate(bucket, &key, |history| {
						history
							.try_push(Self::history_entry(record))
							.map_err(|_| Error::<T>::ObjectHistoryFull)
					})?;
				}
			}
			let version = match current.as_ref() {
				Some(record) => {
					record.version.checked_add(1).ok_or(Error::<T>::ObjectVersionOverflow)?
				},
				None => 1,
			};
			if current.is_none() {
				BucketObjectKeys::<T>::try_mutate(bucket, |keys| {
					keys.try_push(key.clone()).map_err(|_| Error::<T>::BucketObjectIndexFull)
				})?;
				bucket_record.live_objects = bucket_record.live_objects.saturating_add(1);
			} else if current.as_ref().is_some_and(|record| record.deleted) {
				bucket_record.live_objects = bucket_record.live_objects.saturating_add(1);
			}
			let object = Self::object_id(bucket, &key);
			let now = frame_system::Pallet::<T>::block_number();
			Objects::<T>::insert(
				bucket,
				&key,
				ObjectRecord::<T> {
					object_id: object,
					content_hash: Some(content_hash),
					version,
					deleted: false,
					updated_by: who,
					updated_at: now,
				},
			);
			bucket_record.updated_at = now;
			Buckets::<T>::insert(bucket, bucket_record);
			Self::deposit_event(Event::ObjectPut { bucket, object, key, content_hash, version });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::delete_object(key.len() as u32, ObjectHistory::<T>::decode_len(bucket, &key).unwrap_or(0) as u32))]
		#[transactional]
		pub fn delete_object(
			origin: OriginFor<T>,
			bucket: T::Hash,
			key: ObjectKeyOf<T>,
			expected_object_version: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!key.is_empty(), Error::<T>::EmptyObjectKey);
			let mut bucket_record = Buckets::<T>::get(bucket).ok_or(Error::<T>::BucketNotFound)?;
			Self::ensure_bucket_active(&bucket_record)?;
			Self::ensure_controller(&bucket_record, &who)?;
			let current = Objects::<T>::get(bucket, &key).ok_or(Error::<T>::ObjectNotFound)?;
			ensure!(!current.deleted, Error::<T>::ObjectAlreadyDeleted);
			ensure!(current.version == expected_object_version, Error::<T>::ObjectVersionMismatch);
			if bucket_record.versioning_enabled {
				ObjectHistory::<T>::try_mutate(bucket, &key, |history| {
					history
						.try_push(Self::history_entry(&current))
						.map_err(|_| Error::<T>::ObjectHistoryFull)
				})?;
			}
			let version =
				current.version.checked_add(1).ok_or(Error::<T>::ObjectVersionOverflow)?;
			let now = frame_system::Pallet::<T>::block_number();
			Objects::<T>::insert(
				bucket,
				&key,
				ObjectRecord::<T> {
					object_id: current.object_id,
					content_hash: None,
					version,
					deleted: true,
					updated_by: who,
					updated_at: now,
				},
			);
			bucket_record.live_objects = bucket_record.live_objects.saturating_sub(1);
			bucket_record.updated_at = now;
			Buckets::<T>::insert(bucket, bucket_record);
			Self::deposit_event(Event::ObjectDeleted {
				bucket,
				object: current.object_id,
				key,
				version,
			});
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::delete_bucket(BucketObjectKeys::<T>::decode_len(bucket).unwrap_or(0) as u32))]
		#[transactional]
		pub fn delete_bucket(
			origin: OriginFor<T>,
			bucket: T::Hash,
			expected_bucket_version: u64,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let mut record = Buckets::<T>::get(bucket).ok_or(Error::<T>::BucketNotFound)?;
			Self::ensure_bucket_live(&record)?;
			ensure!(record.owner == owner, Error::<T>::NotBucketOwner);
			Self::ensure_bucket_version(&record, expected_bucket_version)?;
			ensure!(record.live_objects == 0, Error::<T>::BucketNotEmpty);
			for key in BucketObjectKeys::<T>::take(bucket) {
				Objects::<T>::remove(bucket, &key);
				ObjectHistory::<T>::remove(bucket, &key);
			}
			OwnerBuckets::<T>::mutate(&owner, |items| {
				if let Some(index) = items.iter().position(|id| id == &bucket) {
					items.swap_remove(index);
				}
			});
			record.status = BucketStatus::Deleted;
			record.controllers = Default::default();
			record.version =
				record.version.checked_add(1).ok_or(Error::<T>::BucketVersionOverflow)?;
			record.updated_at = frame_system::Pallet::<T>::block_number();
			let name = record.name.clone();
			Buckets::<T>::insert(bucket, record);
			Self::deposit_event(Event::BucketDeleted { bucket, name, owner });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn validate_bucket_name(name: &BucketNameOf<T>) -> DispatchResult {
			let bytes = name.as_slice();
			ensure!(!bytes.is_empty(), Error::<T>::EmptyBucketName);
			let is_alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
			ensure!(
				is_alphanumeric(bytes[0]) && is_alphanumeric(bytes[bytes.len() - 1]),
				Error::<T>::InvalidBucketName
			);
			let mut previous = 0;
			for byte in bytes.iter().copied() {
				ensure!(
					is_alphanumeric(byte) || byte == b'.' || byte == b'-',
					Error::<T>::InvalidBucketName
				);
				ensure!(!(byte == b'.' && previous == b'.'), Error::<T>::InvalidBucketName);
				ensure!(
					!((byte == b'.' && previous == b'-') || (byte == b'-' && previous == b'.')),
					Error::<T>::InvalidBucketName
				);
				previous = byte;
			}
			Ok(())
		}

		pub fn bucket_id(owner: &T::AccountId, name: &BucketNameOf<T>) -> T::Hash {
			T::Hashing::hash_of(&(BUCKET_ID_DOMAIN, owner, name))
		}

		pub fn object_id(bucket: T::Hash, key: &ObjectKeyOf<T>) -> T::Hash {
			T::Hashing::hash_of(&(OBJECT_ID_DOMAIN, bucket, key))
		}

		fn ensure_bucket_live(record: &BucketRecord<T>) -> DispatchResult {
			ensure!(record.status != BucketStatus::Deleted, Error::<T>::BucketDeleted);
			Ok(())
		}

		fn ensure_bucket_active(record: &BucketRecord<T>) -> DispatchResult {
			match record.status {
				BucketStatus::Active => Ok(()),
				BucketStatus::Archived => Err(Error::<T>::BucketArchived.into()),
				BucketStatus::Deleted => Err(Error::<T>::BucketDeleted.into()),
			}
		}

		fn ensure_controller(record: &BucketRecord<T>, who: &T::AccountId) -> DispatchResult {
			ensure!(
				record.owner == *who || record.controllers.contains(who),
				Error::<T>::NotBucketController
			);
			Ok(())
		}

		fn ensure_bucket_version(record: &BucketRecord<T>, expected: u64) -> DispatchResult {
			ensure!(record.version == expected, Error::<T>::BucketVersionMismatch);
			Ok(())
		}

		fn bump_bucket(record: &mut BucketRecord<T>) -> Result<u64, DispatchError> {
			record.version =
				record.version.checked_add(1).ok_or(Error::<T>::BucketVersionOverflow)?;
			record.updated_at = frame_system::Pallet::<T>::block_number();
			Ok(record.version)
		}

		fn history_entry(record: &ObjectRecord<T>) -> ObjectVersion<T> {
			ObjectVersion::<T> {
				content_hash: record.content_hash,
				version: record.version,
				deleted: record.deleted,
				updated_by: record.updated_by.clone(),
				updated_at: record.updated_at,
			}
		}
	}
}
