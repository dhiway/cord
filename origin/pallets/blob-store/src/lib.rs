// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, StorageVersion, WithdrawReasons},
	BoundedVec,
};
use frame_system::{ensure_root, pallet_prelude::*};
use origin_primitives::{
	authorization::{authorization_signature_hash, ensure_authorization_ttl, extract_valid_until},
	Signature,
};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	traits::{UniqueSaturatedInto, Verify},
	AccountId32,
};

pub use pallet::*;

pub type BlobId = H256;
pub type RootHash = H256;
pub type Period = u32;

#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Debug,
)]
pub enum BlobStatus {
	Registered,
	Active,
	Grace,
	Expired,
	ArchiveAuthorized,
	Archived,
}

#[derive(
	Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug,
)]
pub struct BlobRecord<AccountId, Balance> {
	pub owner: AccountId,
	pub publisher: AccountId,
	pub root_hash: RootHash,
	pub size_bytes: u64,
	pub encoding: u8,
	pub status: BlobStatus,
	pub created_period: Period,
	pub next_charge_period: Period,
	pub grace_until_period: Option<Period>,
	pub checksum: Option<H256>,
	pub last_charge: Option<Balance>,
}

/// SCALE payload that must be signed by the blob owner and supplied by the publisher.
///
/// `valid_until` must be the final field (trailing `u32`) so pallets can apply the shared TTL
/// utilities from `origin-primitives`.
#[derive(
	Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Debug,
)]
pub struct RegisterBlobAuthorization<AccountId> {
	pub domain: [u8; 8],
	pub publisher: AccountId,
	pub blob_id: BlobId,
	pub root_hash: RootHash,
	pub size_bytes: u64,
	pub encoding: u8,
	pub nonce: u64,
	pub valid_until: u32,
}

const AUTH_DOMAIN: [u8; 8] = *b"ORIGBLOB";

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Authorization payload supplied for publisher-mediated blob registration.
	pub type AuthorizationPayloadOf<T> = BoundedVec<u8, <T as Config>::MaxAuthorizationLen>;

	/// Authorization structure.
	pub type AuthorizationOf<T> = origin_primitives::authorization::Authorization<
		<T as frame_system::Config>::AccountId,
		AuthorizationPayloadOf<T>,
		Signature,
	>;

	pub type BlobRecordOf<T> = BlobRecord<<T as frame_system::Config>::AccountId, BalanceOf<T>>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config<AccountId: Clone + Into<AccountId32>> + pallet_storage_roles::Config
	{
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId, Balance = u128>;

		/// Max length for authorization payloads.
		#[pallet::constant]
		type MaxAuthorizationLen: Get<u32>;

		/// Maximum number of blocks for which an authorization stays valid.
		#[pallet::constant]
		type MaxAuthorizationTTL: Get<u32>;

		/// Length of a renewal period in blocks (e.g. 432_000 for 30 days @ 6s).
		#[pallet::constant]
		type PeriodLengthBlocks: Get<BlockNumberFor<Self>>;

		/// Grace window in periods (e.g. 2 for 60 days with 30-day periods).
		#[pallet::constant]
		type GracePeriods: Get<u32>;

		/// Max blobs scheduled for a single period.
		#[pallet::constant]
		type MaxBlobsPerPeriod: Get<u32>;

		/// Max renewal work performed per on_initialize.
		#[pallet::constant]
		type MaxRenewalsPerBlock: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Blobs<T: Config> =
		StorageMap<_, Blake2_128Concat, BlobId, BlobRecordOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type BlobsByChargePeriod<T: Config> =
		StorageMap<_, Twox64Concat, Period, BoundedVec<BlobId, T::MaxBlobsPerPeriod>, ValueQuery>;

	/// If present and `current_period < until_period`, writes for the owner are blocked.
	#[pallet::storage]
	pub type OwnerWriteLockUntilPeriod<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Period, OptionQuery>;

	/// Replay protection for user-signed authorizations (hash of account+payload+signature).
	#[pallet::storage]
	pub type UsedAuthorizationHashes<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 16], (), OptionQuery>;

	/// Price per byte per period (charged at renewal boundaries).
	#[pallet::storage]
	pub type PricePerBytePerPeriod<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BlobRegistered {
			blob_id: BlobId,
			owner: T::AccountId,
			publisher: T::AccountId,
			size_bytes: u64,
			next_charge_period: Period,
		},
		BlobStored {
			blob_id: BlobId,
			checksum: H256,
		},
		OwnerEnteredGrace {
			owner: T::AccountId,
			until_period: Period,
		},
		OwnerGraceCleared {
			owner: T::AccountId,
		},
		BlobExpired {
			blob_id: BlobId,
		},
		ArchiveAuthorized {
			blob_id: BlobId,
		},
		BlobArchived {
			blob_id: BlobId,
		},
		PriceUpdated {
			price_per_byte_per_period: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		NotPublisher,
		NotAggregator,
		WritesBlocked,
		BlobAlreadyExists,
		BlobNotFound,
		InvalidState,
		InvalidAuthorizationPayload,
		AuthorizationExpired,
		AuthorizationReplay,
		Unauthorized,
		QueueFull,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let period_len: u32 = T::PeriodLengthBlocks::get().unique_saturated_into();
			if period_len == 0 {
				return Weight::zero();
			}
			let block: u32 = n.unique_saturated_into();
			if block == 0 || (block % period_len != 0) {
				return Weight::zero();
			}
			let current_period = block / period_len;
			Self::process_renewals_for_period(current_period);
			Weight::zero()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn set_price_per_byte_per_period(
			origin: OriginFor<T>,
			price: BalanceOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			PricePerBytePerPeriod::<T>::put(price);
			Self::deposit_event(Event::PriceUpdated { price_per_byte_per_period: price });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())]
		pub fn register_blob_by_publisher(
			origin: OriginFor<T>,
			owner: T::AccountId,
			blob_id: BlobId,
			root_hash: RootHash,
			size_bytes: u64,
			encoding: u8,
			auth: AuthorizationOf<T>,
		) -> DispatchResult {
			let publisher = ensure_signed(origin)?;
			ensure!(
				pallet_storage_roles::Pallet::<T>::is_publisher(&publisher),
				Error::<T>::NotPublisher
			);

			Self::ensure_writes_allowed(&owner)?;
			ensure!(!Blobs::<T>::contains_key(&blob_id), Error::<T>::BlobAlreadyExists);

			Self::verify_register_authorization(
				&owner, &publisher, blob_id, root_hash, size_bytes, encoding, &auth,
			)?;

			let current_period = Self::current_period();
			let next_charge_period = current_period.saturating_add(1);

			let record = BlobRecordOf::<T> {
				owner: owner.clone(),
				publisher: publisher.clone(),
				root_hash,
				size_bytes,
				encoding,
				status: BlobStatus::Registered,
				created_period: current_period,
				next_charge_period,
				grace_until_period: None,
				checksum: None,
				last_charge: None,
			};
			Blobs::<T>::insert(&blob_id, record);
			Self::enqueue_for_period(next_charge_period, blob_id)?;

			Self::deposit_event(Event::BlobRegistered {
				blob_id,
				owner,
				publisher,
				size_bytes,
				next_charge_period,
			});
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())]
		pub fn confirm_stored(
			origin: OriginFor<T>,
			blob_id: BlobId,
			checksum: H256,
		) -> DispatchResult {
			let publisher = ensure_signed(origin)?;
			ensure!(
				pallet_storage_roles::Pallet::<T>::is_publisher(&publisher),
				Error::<T>::NotPublisher
			);

			Blobs::<T>::try_mutate(blob_id, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::BlobNotFound)?;
				ensure!(record.publisher == publisher, Error::<T>::Unauthorized);
				ensure!(record.status == BlobStatus::Registered, Error::<T>::InvalidState);
				record.status = BlobStatus::Active;
				record.checksum = Some(checksum);
				Ok(())
			})?;

			Self::deposit_event(Event::BlobStored { blob_id, checksum });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero())]
		pub fn authorize_archive(origin: OriginFor<T>, blob_id: BlobId) -> DispatchResult {
			ensure_root(origin)?;
			Blobs::<T>::try_mutate(blob_id, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::BlobNotFound)?;
				ensure!(record.status == BlobStatus::Expired, Error::<T>::InvalidState);
				record.status = BlobStatus::ArchiveAuthorized;
				Ok(())
			})?;
			Self::deposit_event(Event::ArchiveAuthorized { blob_id });
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(Weight::zero())]
		pub fn mark_archived(origin: OriginFor<T>, blob_id: BlobId) -> DispatchResult {
			let aggregator = ensure_signed(origin)?;
			ensure!(
				pallet_storage_roles::Pallet::<T>::is_aggregator(&aggregator),
				Error::<T>::NotAggregator
			);
			Blobs::<T>::try_mutate(blob_id, |maybe| -> DispatchResult {
				let record = maybe.as_mut().ok_or(Error::<T>::BlobNotFound)?;
				ensure!(record.status == BlobStatus::ArchiveAuthorized, Error::<T>::InvalidState);
				record.status = BlobStatus::Archived;
				Ok(())
			})?;
			Self::deposit_event(Event::BlobArchived { blob_id });
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		pub fn blob(blob_id: BlobId) -> Option<BlobRecordOf<T>> {
			Blobs::<T>::get(blob_id)
		}

		pub fn owner_write_lock_until(owner: T::AccountId) -> Option<Period> {
			OwnerWriteLockUntilPeriod::<T>::get(owner)
		}

		pub fn current_period_view() -> Period {
			Self::current_period()
		}

		pub fn price_per_byte_per_period() -> BalanceOf<T> {
			PricePerBytePerPeriod::<T>::get()
		}
	}

	impl<T: Config> Pallet<T> {
		fn current_period() -> Period {
			let block: u32 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
			let period_len: u32 = T::PeriodLengthBlocks::get().unique_saturated_into();
			let period_len = period_len.max(1u32);
			block / period_len
		}

		fn ensure_writes_allowed(owner: &T::AccountId) -> Result<(), Error<T>> {
			let current_period = Self::current_period();
			if let Some(until_period) = OwnerWriteLockUntilPeriod::<T>::get(owner) {
				if current_period < until_period {
					return Err(Error::<T>::WritesBlocked);
				}
			}
			Ok(())
		}

		fn verify_register_authorization(
			owner: &T::AccountId,
			publisher: &T::AccountId,
			blob_id: BlobId,
			root_hash: RootHash,
			size_bytes: u64,
			encoding: u8,
			auth: &AuthorizationOf<T>,
		) -> Result<(), Error<T>> {
			let now: u32 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
			let issued_at = extract_valid_until(auth.payload.as_slice())
				.ok_or(Error::<T>::InvalidAuthorizationPayload)?;
			let ttl = T::MaxAuthorizationTTL::get();
			ensure_authorization_ttl(now, issued_at, ttl)
				.map_err(|_| Error::<T>::AuthorizationExpired)?;

			ensure!(&auth.account == owner, Error::<T>::InvalidAuthorizationPayload);
			let signer: AccountId32 = owner.clone().into();
			if !auth.signature.verify(auth.payload.as_slice(), &signer) {
				return Err(Error::<T>::Unauthorized);
			}

			let auth_hash = authorization_signature_hash(
				&auth.account,
				auth.payload.as_slice(),
				&auth.signature,
			);
			ensure!(
				!UsedAuthorizationHashes::<T>::contains_key(auth_hash),
				Error::<T>::AuthorizationReplay
			);

			let payload: RegisterBlobAuthorization<T::AccountId> =
				RegisterBlobAuthorization::decode(&mut &auth.payload.as_slice()[..])
					.map_err(|_| Error::<T>::InvalidAuthorizationPayload)?;
			ensure!(payload.domain == AUTH_DOMAIN, Error::<T>::InvalidAuthorizationPayload);
			ensure!(&payload.publisher == publisher, Error::<T>::InvalidAuthorizationPayload);
			ensure!(payload.blob_id == blob_id, Error::<T>::InvalidAuthorizationPayload);
			ensure!(payload.root_hash == root_hash, Error::<T>::InvalidAuthorizationPayload);
			ensure!(payload.size_bytes == size_bytes, Error::<T>::InvalidAuthorizationPayload);
			ensure!(payload.encoding == encoding, Error::<T>::InvalidAuthorizationPayload);

			UsedAuthorizationHashes::<T>::insert(auth_hash, ());
			Ok(())
		}

		fn enqueue_for_period(period: Period, blob_id: BlobId) -> Result<(), Error<T>> {
			BlobsByChargePeriod::<T>::try_mutate(period, |queue| {
				queue.try_push(blob_id).map_err(|_| Error::<T>::QueueFull)?;
				Ok(())
			})
		}

		fn process_renewals_for_period(period: Period) {
			let price = PricePerBytePerPeriod::<T>::get();
			let max = T::MaxRenewalsPerBlock::get().max(1) as usize;
			let mut processed: usize = 0;

			let mut remaining: Vec<BlobId> = Vec::new();
			let due = BlobsByChargePeriod::<T>::take(period);
			for blob_id in due.into_inner().into_iter() {
				if processed >= max {
					remaining.push(blob_id);
					continue;
				}
				processed = processed.saturating_add(1);

				let _ = Blobs::<T>::try_mutate(blob_id, |maybe| -> DispatchResult {
					let record = match maybe.as_mut() {
						Some(r) => r,
						None => return Ok(()),
					};
					if record.status == BlobStatus::Archived ||
						record.status == BlobStatus::ArchiveAuthorized
					{
						return Ok(());
					}
					if record.status == BlobStatus::Expired {
						return Ok(());
					}

					let current_period = period;
					let grace_periods = T::GracePeriods::get();
					let grace_until = OwnerWriteLockUntilPeriod::<T>::get(&record.owner);
					if let Some(until) = grace_until {
						if current_period >= until {
							record.status = BlobStatus::Expired;
							record.grace_until_period = Some(until);
							Self::deposit_event(Event::BlobExpired { blob_id });
							return Ok(());
						}
					}

					let charge: BalanceOf<T> = price.saturating_mul(record.size_bytes as u128);
					let withdraw = T::Currency::withdraw(
						&record.owner,
						charge,
						WithdrawReasons::FEE,
						ExistenceRequirement::KeepAlive,
					);

					match withdraw {
						Ok(_imbalance) => {
							record.last_charge = Some(charge);
							record.status = BlobStatus::Active;
							record.grace_until_period = None;
							if OwnerWriteLockUntilPeriod::<T>::contains_key(&record.owner) {
								OwnerWriteLockUntilPeriod::<T>::remove(&record.owner);
								Self::deposit_event(Event::OwnerGraceCleared {
									owner: record.owner.clone(),
								});
							}
						},
						Err(_) => {
							let until = current_period.saturating_add(grace_periods);
							OwnerWriteLockUntilPeriod::<T>::insert(&record.owner, until);
							record.status = BlobStatus::Grace;
							record.grace_until_period = Some(until);
							Self::deposit_event(Event::OwnerEnteredGrace {
								owner: record.owner.clone(),
								until_period: until,
							});
						},
					}

					record.next_charge_period = current_period.saturating_add(1);
					Ok(())
				});

				let next = Blobs::<T>::get(blob_id).map(|r| r.next_charge_period);
				if let Some(next_period) = next {
					let _ = Self::enqueue_for_period(next_period, blob_id);
				}
			}

			if !remaining.is_empty() {
				let mut queue: BoundedVec<BlobId, T::MaxBlobsPerPeriod> = Default::default();
				for id in remaining.into_iter() {
					let _ = queue.try_push(id);
				}
				BlobsByChargePeriod::<T>::insert(period, queue);
			}
		}
	}
}
