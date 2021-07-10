// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Mark: Handles #MARKs on chain,
//! adding and revoking #MARKs.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod marks;
pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::marks::*;
pub use pallet::*;

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

	/// Type of an issuer identifier.
	pub type MarkIssuerOf<T> = pallet_mtype::MtypeOwnerOf<T>;

	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	/// CID Information
	pub type CidOf = Vec<u8>;

	/// Type of a digest hash.
	pub type DigestHashOf<T> = <T as frame_system::Config>::Hash;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_mtype::Config {
		type EnsureOrigin: EnsureOrigin<Success = MarkIssuerOf<Self>, <Self as frame_system::Config>::Origin>;
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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new mark has been created.
		/// \[issuer ID, stream hash, MTYPE hash\]
		MarkAnchored(MarkIssuerOf<T>, StreamHashOf<T>),
		/// A Mark has been revoked.
		/// \[revoker ID, stream hash\]
		MarkRevoked(MarkIssuerOf<T>, StreamHashOf<T>),
		/// A Mark has been revoked.
		/// \[restorer ID, stream hash\]
		MarkRestored(MarkIssuerOf<T>, StreamHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a MARK with the same content hash stored on
		/// chain.
		AlreadyAnchored,
		/// The mark has already been revoked.
		AlreadyRevoked,
		/// No mark on chain matching the content hash.
		MarkNotFound,
		/// The mark MTYPE does not match the MTYPE specified
		MTypeMismatch,
		/// The MARK is not under the control of the revoker, or it
		/// is but it has been revoked. Only when the revoker is not the
		/// original issuer.
		UnauthorizedRevocation,
		/// the mark cannot be restored by the issuer.
		UnauthorizedRestore,
		/// the mark is active.
		/// only when trying to restore an active mark.
		MarkStillActive,
		/// Invalid StreamCid encoding.
		InvalidStreamCidEncoding,
		/// Invalid ParentCid encoding.
		InvalidParentCidEncoding,
		/// MTYPE not authorised.
		MTypeNotDelegatedToIssuer,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new mark.
		///
		///
		/// * origin: the identifier of the issuer
		/// * stream_hash: the hash of the conten to attest. It has to be unique
		/// * mtype_hash: the hash of the MTYPE used for this mark
		/// * stream_cid: CID of the MARK content
		/// * parent_cid: CID of the parent Mark
		/// * digest_hash: \[OPTIONAL\] digest hash of the presentation format
		#[pallet::weight(<T as pallet::Config>::WeightInfo::anchor())]
		pub fn anchor(
			origin: OriginFor<T>,
			stream_hash: StreamHashOf<T>,
			mtype_hash: MtypeHashOf<T>,
			stream_cid: CidOf,
			parent_cid: Option<CidOf>,
			digest_hash: Option<DigestHashOf<T>>,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// //TODO - Fix MType Ownership check
			let mtype = <pallet_mtype::Mtypes<T>>::get(mtype_hash).ok_or(pallet_mtype::Error::<T>::MTypeNotFound)?;
			ensure!(mtype.owner == issuer, Error::<T>::MTypeNotDelegatedToIssuer);

			ensure!(!<Marks<T>>::contains_key(&stream_hash), Error::<T>::AlreadyAnchored);
			// TODO - Change this to length check
			let cid_base = str::from_utf8(&stream_cid).unwrap();
			ensure!(
				pallet_mtype::utils::is_base_32(cid_base) || pallet_mtype::utils::is_base_58(cid_base),
				Error::<T>::InvalidStreamCidEncoding
			);

			// TODO - Change this to length check
			if let Some(ref parent_cid) = parent_cid {
				let pcid_base = str::from_utf8(&parent_cid).unwrap();
				ensure!(
					pallet_mtype::utils::is_base_32(pcid_base) || pallet_mtype::utils::is_base_58(pcid_base),
					Error::<T>::InvalidParentCidEncoding
				);
			}

			log::debug!("Anchor Mark");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Marks<T>>::insert(
				&stream_hash,
				MarkDetails {
					mtype_hash,
					issuer: issuer.clone(),
					stream_cid,
					parent_cid,
					digest_hash,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::MarkAnchored(issuer, stream_hash));

			Ok(())
		}

		/// Revoke an existing mark.
		///
		/// The revoker must be the creator of the mark
		/// * origin: the identifier of the revoker
		/// * stream_hash: the hash of the content to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, stream_hash: StreamHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mark = <Marks<T>>::get(&stream_hash).ok_or(Error::<T>::MarkNotFound)?;
			ensure!(mark.issuer == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!mark.revoked, Error::<T>::AlreadyRevoked);

			log::debug!("Revoking Mark");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Marks<T>>::insert(
				&stream_hash,
				MarkDetails {
					block_number,
					revoked: true,
					..mark
				},
			);
			Self::deposit_event(Event::MarkRevoked(revoker, stream_hash));

			Ok(())
		}
		// Restore a revoked mark.
		///
		/// The restorer must be the creator of the mark being restored
		/// * origin: the identifier of the restorer
		/// * stream_hash: the hash of the content to restore
		#[pallet::weight(0)]
		pub fn restore(origin: OriginFor<T>, stream_hash: StreamHashOf<T>) -> DispatchResult {
			let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mark = <Marks<T>>::get(&stream_hash).ok_or(Error::<T>::MarkNotFound)?;
			ensure!(mark.revoked, Error::<T>::MarkStillActive);
			ensure!(mark.issuer == restorer, Error::<T>::UnauthorizedRestore);

			log::debug!("Restoring Mark");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Marks<T>>::insert(
				&stream_hash,
				MarkDetails {
					block_number,
					revoked: false,
					..mark
				},
			);
			Self::deposit_event(Event::MarkRestored(restorer, stream_hash));

			Ok(())
		}
	}
}
