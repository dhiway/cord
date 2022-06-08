// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Mark: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

// use codec::{Decode, Encode};
// use frame_system::Config;
use pallet::{CordAccountOf, HashOf};
use sp_std::prelude::{Clone, PartialEq};
pub mod mark;
pub use crate::mark::*;
pub use pallet::*;
pub mod weights;
use crate::weights::WeightInfo;
use frame_support::{ensure, storage::types::StorageMap};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, BoundedVec};
	use frame_system::pallet_prelude::*;

	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a delegation node identifier.
	pub type DelegationNodeIdOf<T> = <T as Config>::DelegationNodeId;

	/// The #MARK trait
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_mtype::Config {
		type DelegationNodeId: Parameter + Copy + MaxEncodedLen;
		/// #MARK specific event type
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		#[pallet::constant]
		type MaxDelegatedAttestations: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::storage]
	#[pallet::getter(fn marks)]
	pub type Marks<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, Mark<T>>;

	#[pallet::storage]
	#[pallet::getter(fn delegated_marks)]
	pub type DelegatedMarks<T> = StorageMap<
		_,
		Blake2_128Concat,
		DelegationNodeIdOf<T>,
		BoundedVec<HashOf<T>, <T as Config>::MaxDelegatedAttestations>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new #MARK has been anchored
		Anchored(CordAccountOf<T>, HashOf<T>, HashOf<T>, Option<DelegationNodeIdOf<T>>),
		/// A #MARK has been revoked
		Revoked(CordAccountOf<T>, HashOf<T>),
		/// A #MARK has been restored (previously revoked)
		Restored(CordAccountOf<T>, HashOf<T>),
	}

	// The pallet's errors
	#[pallet::error]
	pub enum Error<T> {
		AlreadyAnchoredMark,
		AlreadyRevoked,
		MarkNotFound,
		MTypeMismatch,
		DelegationUnauthorisedToAnchor,
		DelegationRevoked,
		NotDelegatedToMarker,
		UnauthorizedRevocation,
		UnauthorizedRestore,
		MarkStillActive,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Anchors a new #MARK on chain
		///, where, originis the signed sender account,
		/// content_hash is the hash of the content stream,
		/// mtype_hash is the hash of the #MARK TYPE,
		/// and delegation_id refers to a #MARK TYPE delegation.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(3, 2))]
		pub fn anchor(
			origin: OriginFor<T>,
			content_hash: HashOf<T>,
			mtype_hash: HashOf<T>,
			delegation_id: Option<DelegationNodeIdOf<T>>,
		) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// check if the #MARK TYPE exists
			ensure!(
				<pallet_mtype::Mtypes<T>>::contains_key(mtype_hash),
				pallet_mtype::Error::<T>::NotFound
			);

			// check if the #MARK already exists
			ensure!(!<Marks<T>>::contains_key(content_hash), Error::<T>::AlreadyAnchoredMark);

			// insert #MARK
			<Marks<T>>::insert(
				content_hash,
				Mark { mtype_hash, marker: creator.clone(), delegation_id, revoked: false },
			);

			if let Some(d) = delegation_id {
				// if the #MARK is based on a delegation, store seperately

				let mut delegated_marks = <DelegatedMarks<T>>::get(d).unwrap_or_default();
				delegated_marks
					.try_push(content_hash)
					.expect("delegates length is less than T::MaxSchemaDelegates; qed");
				<DelegatedMarks<T>>::insert(d, delegated_marks);
			}

			// deposit event that mark has beed added
			Self::deposit_event(Event::Anchored(creator, content_hash, mtype_hash, delegation_id));
			Ok(())
		}

		/// Revokes a #MARK
		/// where, origin is the signed sender account,
		/// and content_hash is the hash of the anchored #MARK.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn revoke(
			origin: OriginFor<T>,
			content_hash: HashOf<T>,
			_max_depth: u32,
		) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// lookup #MARK & check if it exists
			let Mark { mtype_hash, marker, delegation_id, revoked, .. } =
				<Marks<T>>::get(content_hash).ok_or(Error::<T>::MarkNotFound)?;

			// check if the #MARK has already been revoked
			ensure!(!revoked, Error::<T>::AlreadyRevoked);

			// revoke #MARK
			<Marks<T>>::insert(
				content_hash,
				Mark { mtype_hash, marker, delegation_id, revoked: true },
			);
			// deposit event that the #MARK has been revoked
			Self::deposit_event(Event::Revoked(sender, content_hash));
			Ok(())
		}

		/// Restores a #MARK
		/// where, origin is the signed sender account,
		/// content_hash is the revoked #MARK.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn restore(
			origin: OriginFor<T>,
			content_hash: HashOf<T>,
			_max_depth: u32,
		) -> DispatchResult {
			// origin of the transaction needs to be a signed sender account
			let sender = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// lookup #MARK & check if it exists
			let Mark { mtype_hash, marker, delegation_id, revoked, .. } =
				<Marks<T>>::get(content_hash).ok_or(Error::<T>::MarkNotFound)?;

			// check if the #MARK has already been revoked
			ensure!(revoked, Error::<T>::MarkStillActive);

			// restore #MARK
			<Marks<T>>::insert(
				content_hash,
				Mark { mtype_hash, marker, delegation_id, revoked: false },
			);

			// deposit event that the #MARK has been restored
			Self::deposit_event(Event::Restored(sender, content_hash));
			Ok(())
		}
	}
}
