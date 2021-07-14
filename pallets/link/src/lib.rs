// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod links;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::links::*;
pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type LinkHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = pallet_schema::SchemaHashOf<T>;
	/// Type of link transaction owner identifier.
	pub type CredHashOf<T> = pallet_cred::CredHashOf<T>;
	/// Type of link owner identifier.
	pub type LinkOwnerOf<T> = pallet_schema::SchemaOwnerOf<T>;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// CID Information
	pub type LinkCidOf = Vec<u8>;

	// /// Type of a linked credential hash.
	// pub type CredentialHashOf<T> = <T as frame_system::Config>::Hash;
	// /// Type of an incoming stream hash.
	// pub type LinkedStreamHashOf<T> = <T as frame_system::Config>::Hash;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_cred::Config {
		type EnsureOrigin: EnsureOrigin<Success = LinkOwnerOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Linked streams stored on chain.
	/// It maps from a link hash to the details.
	#[pallet::storage]
	#[pallet::getter(fn links)]
	pub type Links<T> = StorageMap<_, Blake2_128Concat, LinkHashOf<T>, StreamLinkDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream link has been created.
		/// \[issuer identifier, link hash, link cid\]
		StreamLinkAnchored(LinkOwnerOf<T>, LinkHashOf<T>, LinkCidOf),
		/// A stream link has been revoked.
		/// \[revoker identifier, link hash\]
		StreamLinkRevoked(LinkOwnerOf<T>, LinkHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a credential with the same hash stored on
		/// chain.
		StreamLinkAlreadyAnchored,
		/// The stream has already been revoked.
		StreamLinkAlreadyRevoked,
		/// No credential on chain matching the content hash.
		StreamLinkNotFound,
		/// The credential is not under the control of the revoker, or it
		/// is but it has been revoked. Only when the revoker is not the
		/// owner.
		UnauthorizedRevocation,
		/// Invalid StreamLinkCid encoding.
		InvalidStreamLinkCid,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream link.
		///
		///
		/// * origin: the identifier of the owner
		/// * link_hash: the hash of the content to link. It has to be unique
		/// * schema_hash: the hash of the schema used for this credential
		/// * link_cid: CID of the credential content
		/// * cred_hash: hash of the linked credential
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			link_hash: LinkHashOf<T>,
			schema_hash: SchemaHashOf<T>,
			link_cid: LinkCidOf,
			cred_hash: CredHashOf<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Links<T>>::contains_key(&link_hash),
				Error::<T>::StreamLinkAlreadyAnchored
			);
			let schema =
				<pallet_schema::Schemas<T>>::get(schema_hash).ok_or(pallet_schema::Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == owner, pallet_schema::Error::<T>::SchemaNotDelegated);
			let cred = <pallet_cred::Creds<T>>::get(cred_hash).ok_or(pallet_cred::Error::<T>::CredentialNotFound)?;
			ensure!(!cred.revoked, pallet_cred::Error::<T>::CredentialRevoked);

			// TODO - Change this to length check
			let cid_base = str::from_utf8(&link_cid).unwrap();
			ensure!(
				pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base),
				Error::<T>::InvalidStreamLinkCid
			);
			log::debug!("Anchor Stream Link");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Links<T>>::insert(
				&link_hash,
				StreamLinkDetails {
					schema_hash,
					owner: owner.clone(),
					link_cid: link_cid.clone(),
					cred_hash,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::StreamLinkAnchored(owner, link_hash, link_cid));

			Ok(())
		}

		/// Revoke an existing stream link
		///
		/// The revoker must be the creator of the stream link
		/// * origin: the identifier of the revoker
		/// * link_hash: the hash of the stream link to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, link_hash: LinkHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let link = <Links<T>>::get(&link_hash).ok_or(Error::<T>::StreamLinkNotFound)?;
			ensure!(link.owner == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!link.revoked, Error::<T>::StreamLinkAlreadyRevoked);

			log::debug!("Revoking Stream Link");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Links<T>>::insert(
				&link_hash,
				StreamLinkDetails {
					block_number,
					revoked: true,
					..link
				},
			);
			Self::deposit_event(Event::StreamLinkRevoked(revoker, link_hash));

			Ok(())
		}
	}
}
