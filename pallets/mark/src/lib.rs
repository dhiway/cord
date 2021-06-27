// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Mark: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod marks;
pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

use sp_std::vec::Vec;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::marks::*;
pub use pallet::*;

use frame_support::traits::Get;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type StreamHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a MTYPE hash.
	pub type MtypeHashOf<T> = pallet_mtype::MtypeHashOf<T>;

	/// Type of an marker identifier.
	pub type MarkerOf<T> = pallet_delegation::DelegatorIdOf<T>;

	/// Type of a delegation identifier.
	pub type DelegationNodeIdOf<T> = pallet_delegation::DelegationNodeIdOf<T>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_mtype::Config + pallet_delegation::Config {
		type EnsureOrigin: EnsureOrigin<Success = MarkerOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Marks stored on chain.
	/// It maps from a content hash to the mark.
	#[pallet::storage]
	#[pallet::getter(fn marks)]
	pub type Marks<T> = StorageMap<_, Blake2_128Concat, StreamHashOf<T>, MarkDetails<T>>;

	/// Delegated marks stored on chain.
	/// It maps from a delegation ID to a vector of content hashes.
	#[pallet::storage]
	#[pallet::getter(fn delegated_marks)]
	pub type DelegatedMarks<T> = StorageMap<_, Blake2_128Concat, DelegationNodeIdOf<T>, Vec<StreamHashOf<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new mark has been created.
		/// \[markerer ID, content hash, MTYPE hash, (optional) delegation ID\]
		Anchored(
			MarkerOf<T>,
			StreamHashOf<T>,
			MtypeHashOf<T>,
			Option<DelegationNodeIdOf<T>>,
		),
		/// A Mark has been revoked.
		/// \[revoker ID, claim hash\]
		Revoked(MarkerOf<T>, StreamHashOf<T>),
		// / A #MARK has been restored (previously revoked)
		// Restored(MarkerOf<T>, StreamHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a mark with the same content hash stored on
		/// chain.
		AlreadyAnchored,
		/// The mark has already been revoked.
		AlreadyRevoked,
		/// No mark on chain matching the content hash.
		MarkNotFound,
		/// The mark MTYPE does not match the MTYPE specified in the
		/// delegation hierarchy root.
		MTypeMismatch,
		/// The delegation node does not include the permission to create new
		/// marks. Only when the revoker is not the original marker.
		DelegationUnauthorizedToAnchor,
		/// The delegation node has already been revoked.
		/// Only when the revoker is not the original marker.
		DelegationRevoked,
		/// The delegation node owner is different than the marker.
		/// Only when the revoker is not the original marker.
		NotDelegatedToMarker,
		/// The delegation node is not under the control of the revoker, or it
		/// is but it has been revoked. Only when the revoker is not the
		/// original marker.
		UnauthorizedRevocation,
		/// the mark cannot be restored by the marker.
		UnauthorizedRestore,
		/// the mark is active.
		/// only when trying to restore an active mark.
		MarkStillActive,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new mark.
		///
		/// The marker can optionally provide a reference to an existing
		/// delegation that will be saved along with the mark itself in
		/// the form of an attested delegation.
		///
		/// * origin: the identifier of the marker
		/// * stream_hash: the hash of the conten to attest. It has to be unique
		/// * mtype_hash: the hash of the MTYPE used for this mark
		/// * delegation_id: \[OPTIONAL\] the ID of the delegation node used to
		///   authorise the marker
		#[pallet::weight(<T as pallet::Config>::WeightInfo::anchor())]
		pub fn anchor(
			origin: OriginFor<T>,
			stream_hash: StreamHashOf<T>,
			mtype_hash: MtypeHashOf<T>,
			delegation_id: Option<DelegationNodeIdOf<T>>,
		) -> DispatchResult {
			let marker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				<pallet_mtype::Mtypes<T>>::contains_key(&mtype_hash),
				pallet_mtype::Error::<T>::MTypeNotFound
			);

			ensure!(!<Marks<T>>::contains_key(&stream_hash), Error::<T>::AlreadyAnchored);

			// Check for validity of the delegation node if specified.
			if let Some(delegation_id) = delegation_id {
				let delegation = <pallet_delegation::Delegations<T>>::get(delegation_id)
					.ok_or(pallet_delegation::Error::<T>::DelegationNotFound)?;

				ensure!(!delegation.revoked, Error::<T>::DelegationRevoked);

				ensure!(delegation.owner == marker, Error::<T>::NotDelegatedToMarker);

				ensure!(
					(delegation.permissions & pallet_delegation::Permissions::ANCHOR)
						== pallet_delegation::Permissions::ANCHOR,
					Error::<T>::DelegationUnauthorizedToAnchor
				);

				// Check if the MTYPE of the delegation is matching the MTYPE of the mark
				let root = <pallet_delegation::Roots<T>>::get(delegation.root_id)
					.ok_or(pallet_delegation::Error::<T>::RootNotFound)?;
				ensure!(root.mtype_hash == mtype_hash, Error::<T>::MTypeMismatch);

				// If the mark is based on a delegation, store separately
				let mut delegated_marks = <DelegatedMarks<T>>::get(delegation_id).unwrap_or_default();
				delegated_marks.push(stream_hash);
				<DelegatedMarks<T>>::insert(delegation_id, delegated_marks);
			}

			log::debug!("Anchor Mark");
			<Marks<T>>::insert(
				&stream_hash,
				MarkDetails {
					mtype_hash,
					marker: marker.clone(),
					delegation_id,
					revoked: false,
				},
			);

			Self::deposit_event(Event::Anchored(marker, stream_hash, mtype_hash, delegation_id));

			Ok(())
		}

		/// Revoke an existing mark.
		///
		/// The revoker must be either the creator of the mark being revoked
		/// or an entity that in the delegation tree is an ancestor of
		/// the marker, i.e., it was either the delegator of the marker or
		/// an ancestor thereof.
		///
		/// * origin: the identifier of the revoker
		/// * stream_hash: the hash of the content to revoke
		/// * max_parent_checks: for delegated marks, the number of
		///   delegation nodes to check up in the trust hierarchy (including the
		///   root node but excluding the provided node) to verify whether the
		///   caller is an ancestor of the mark marker and hence
		///   authorised to revoke the specified mark.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke(*max_parent_checks))]
		pub fn revoke(
			origin: OriginFor<T>,
			stream_hash: StreamHashOf<T>,
			max_parent_checks: u32,
		) -> DispatchResultWithPostInfo {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mark = <Marks<T>>::get(&stream_hash).ok_or(Error::<T>::MarkNotFound)?;

			ensure!(!mark.revoked, Error::<T>::AlreadyRevoked);

			// Check the delegation tree if the sender of the revocation operation is not
			// the original attester
			let revocations = if mark.marker != revoker {
				let delegation_id = mark.delegation_id.ok_or(Error::<T>::UnauthorizedRevocation)?;
				ensure!(
					max_parent_checks <= T::MaxParentChecks::get(),
					pallet_delegation::Error::<T>::MaxParentChecksTooLarge
				);
				// Check whether the sender of the revocation controls the delegation node
				// specified, and that its status has not been revoked
				let (is_delegating, revocations) =
					<pallet_delegation::Pallet<T>>::is_delegating(&revoker, &delegation_id, max_parent_checks)?;
				ensure!(is_delegating, Error::<T>::UnauthorizedRevocation);
				revocations
			} else {
				0u32
			};

			log::debug!("Revoking Mark");
			<Marks<T>>::insert(&stream_hash, MarkDetails { revoked: true, ..mark });
			Self::deposit_event(Event::Revoked(revoker, stream_hash));

			Ok(Some(<T as pallet::Config>::WeightInfo::revoke(revocations)).into())
		}
		// Restore a revoked mark.
		// /
		// / The restorer must be either the creator of the mark being restored
		// / or an entity that in the delegation tree is an ancestor.
		// / i.e., it was either the delegator of the marker or
		// / an ancestor thereof.
		// /
		// / * origin: the identifier of the restorer
		// / * stream_hash: the hash of the content to restore
		// / * max_parent_checks: for delegated marks, the number of
		// /   delegation nodes to check up in the trust hierarchy (including the
		// /   root node but excluding the provided node) to verify whether the
		// /   caller is an ancestor of the marker and hence authorised to
		// /   restore the specified mark.
		// #[pallet::weight(0)]
		// pub fn restore(
		// 	origin: OriginFor<T>,
		// 	stream_hash: StreamHashOf<T>,
		// 	max_parent_checks: u32,
		// ) -> DispatchResultWithPostInfo {
		// 	let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

		// 	let mark = <Marks<T>>::get(&stream_hash).ok_or(Error::<T>::MarkNotFound)?;

		// 	ensure!(mark.revoked, Error::<T>::MarkStillActive);

		// 	// Check the delegation tree if the sender of the restore operation is not
		// 	// the original attester
		// 	if mark.marker != restorer {
		// 		let delegation_id = mark.delegation_id.ok_or(Error::<T>::UnauthorizedRestore)?;
		// 		ensure!(
		// 			max_parent_checks <= T::MaxParentChecks::get(),
		// 			pallet_delegation::Error::<T>::MaxParentChecksTooLarge
		// 		);
		// 		// Check whether the sender of the restoration controls the delegation node
		// 		// specified, and that its status has not been revoked
		// 		ensure!(
		// 			<pallet_delegation::Pallet<T>>::is_delegating(&restorer, &delegation_id, max_parent_checks)?,
		// 			Error::<T>::UnauthorizedRestore
		// 		);
		// 	}

		// 	log::debug!("Restoring Mark");
		// 	<Marks<T>>::insert(&stream_hash, MarkDetails { revoked: false, ..mark });

		// 	Self::deposit_event(Event::Restored(restorer, stream_hash));

		// 	//TODO: Return actual weight used, which should be returned by
		// 	// delegation::is_actively_delegating
		// 	Ok(None.into())
		// }
	}
}
