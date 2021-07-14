// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Stream: Handles Streams on chain,
//! adding and revoking Streams.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use sp_std::str;
use sp_std::vec::Vec;

pub mod creds;
pub mod weights;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

// #[cfg(test)]
// mod tests;

// pub use crate::{marks::*, pallet::*, weights::WeightInfo};

pub use crate::creds::*;
pub use pallet::*;

use crate::weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Type of a content hash.
	pub type CredHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a schema hash.
	pub type SchemaHashOf<T> = pallet_schema::SchemaHashOf<T>;

	/// Type of cred parent stream identifier.
	pub type StreamHashOf<T> = pallet_stream::StreamHashOf<T>;

	/// Type of cred owner identifier.
	pub type CredIssuerOf<T> = pallet_schema::SchemaOwnerOf<T>;

	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	/// CID Information
	pub type CredCidOf = Vec<u8>;

	// /// Type of a linked credential hash.
	// pub type CredentialHashOf<T> = <T as frame_system::Config>::Hash;
	// /// Type of an incoming stream hash.
	// pub type LinkedStreamHashOf<T> = <T as frame_system::Config>::Hash;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_stream::Config {
		type EnsureOrigin: EnsureOrigin<Success = CredIssuerOf<Self>, <Self as frame_system::Config>::Origin>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Creds stored on chain.
	/// It maps from a cred hash to the details.
	#[pallet::storage]
	#[pallet::getter(fn creds)]
	pub type Creds<T> = StorageMap<_, Blake2_128Concat, CredHashOf<T>, CredDetails<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new credential has been created.
		/// \[issuer identifier, credential hash, credential cid\]
		CredentialAnchored(CredIssuerOf<T>, CredHashOf<T>, CredCidOf),
		/// A credential has been revoked.
		/// \[revoker identifier, credential hash\]
		CredentialRevoked(CredIssuerOf<T>, CredHashOf<T>),
		/// A credential has been restored.
		/// \[restorer identifier, credential hash\]
		CredentialRestored(CredIssuerOf<T>, CredHashOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is already a credential with the same hash stored on
		/// chain.
		CredentialAlreadyAnchored,
		/// The stream has already been revoked.
		CredentialAlreadyRevoked,
		/// No credential on chain matching the content hash.
		CredentialNotFound,
		/// The schema hash does not match the schema specified
		SchemaMismatch,
		/// The credential is not under the control of the revoker, or it
		/// is but it has been revoked. Only when the revoker is not the
		/// owner.
		UnauthorizedRevocation,
		/// the credential cannot be restored by anoyone other than the owner.
		UnauthorizedRestore,
		/// the credential is active.
		/// only when trying to restore an active credential.
		CredentialStillActive,
		/// Invalid StreamCid encoding.
		InvalidCredCidEncoding,
		/// schema not authorised.
		SchemaNotDelegated,
		/// credential revoked
		CredentialRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new credential.
		///
		///
		/// * origin: the identifier of the issuer
		/// * credential_hash: the hash of the conten to attest. It has to be unique
		/// * schema_hash: the hash of the schema used for this credential
		/// * credential_cid: CID of the credential content
		/// * parent_cid: CID of the parent credential
		#[pallet::weight(0)]
		pub fn anchor(
			origin: OriginFor<T>,
			cred_hash: CredHashOf<T>,
			schema_hash: SchemaHashOf<T>,
			cred_cid: CredCidOf,
			stream_hash: Option<StreamHashOf<T>>,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<Creds<T>>::contains_key(&cred_hash),
				Error::<T>::CredentialAlreadyAnchored
			);

			let schema =
				<pallet_schema::Schemas<T>>::get(schema_hash).ok_or(pallet_schema::Error::<T>::SchemaNotFound)?;
			ensure!(schema.owner == issuer, pallet_schema::Error::<T>::SchemaNotDelegated);
			if let Some(stream_hash) = stream_hash {
				let stream =
					<pallet_stream::Streams<T>>::get(stream_hash).ok_or(pallet_stream::Error::<T>::StreamNotFound)?;
				ensure!(!stream.revoked, pallet_stream::Error::<T>::StreamRevoked);
			}
			// TODO - Change this to length check
			let cid_base = str::from_utf8(&cred_cid).unwrap();
			ensure!(
				pallet_schema::utils::is_base_32(cid_base) || pallet_schema::utils::is_base_58(cid_base),
				Error::<T>::InvalidCredCidEncoding
			);
			log::debug!("Anchor Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Creds<T>>::insert(
				&cred_hash,
				CredDetails {
					schema_hash,
					issuer: issuer.clone(),
					cred_cid: cred_cid.clone(),
					stream_hash,
					block_number,
					revoked: false,
				},
			);

			Self::deposit_event(Event::CredentialAnchored(issuer, cred_hash, cred_cid));

			Ok(())
		}

		/// Revoke an existing credential
		///
		/// The revoker must be the creator of the credential
		/// * origin: the identifier of the revoker
		/// * cred_hash: the hash of the credential to revoke
		#[pallet::weight(0)]
		pub fn revoke(origin: OriginFor<T>, cred_hash: CredHashOf<T>) -> DispatchResult {
			let revoker = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let cred = <Creds<T>>::get(&cred_hash).ok_or(Error::<T>::CredentialNotFound)?;
			ensure!(cred.issuer == revoker, Error::<T>::UnauthorizedRevocation);
			ensure!(!cred.revoked, Error::<T>::CredentialAlreadyRevoked);

			log::debug!("Revoking Credential");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Creds<T>>::insert(
				&cred_hash,
				CredDetails {
					block_number,
					revoked: true,
					..cred
				},
			);
			Self::deposit_event(Event::CredentialRevoked(revoker, cred_hash));

			Ok(())
		}

		// Restore a revoked credential.
		///
		/// The restorer must be the creator of the credential being restored
		/// * origin: the identifier of the restorer
		/// * cred_hash: the hash of the credential to restore
		#[pallet::weight(0)]
		pub fn restore(origin: OriginFor<T>, cred_hash: CredHashOf<T>) -> DispatchResult {
			let restorer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let cred = <Creds<T>>::get(&cred_hash).ok_or(Error::<T>::CredentialNotFound)?;
			ensure!(cred.revoked, Error::<T>::CredentialStillActive);
			ensure!(cred.issuer == restorer, Error::<T>::UnauthorizedRestore);

			log::debug!("Restoring Stream");
			let block_number = <frame_system::Pallet<T>>::block_number();

			<Creds<T>>::insert(
				&cred_hash,
				CredDetails {
					block_number,
					revoked: false,
					..cred
				},
			);
			Self::deposit_event(Event::CredentialRestored(restorer, cred_hash));

			Ok(())
		}
	}
}
