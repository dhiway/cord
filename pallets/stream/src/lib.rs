// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use cord_primitives::{IdentifierOf, StatusOf};
use frame_support::traits::{Currency, OnUnbalanced, ReservableCurrency};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{fmt::Debug, prelude::Clone, str, vec::Vec};

pub mod streams;
pub mod weights;

pub use crate::streams::*;

use crate::weights::WeightInfo;
pub use pallet::*;
type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the Stream.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the controller.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	// stream identifier prefix.
	pub const STREAM_IDENTIFIER_PREFIX: u16 = 43;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Type for a block time.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The currency trait.
		type Currency: ReservableCurrency<Self::AccountId>;
		/// slashed funds.
		type Slashed: OnUnbalanced<NegativeImbalanceOf<Self>>;
		/// The amount held on deposit for a registered mark
		#[pallet::constant]
		type Deposit: Get<BalanceOf<Self>>;
		/// The origin which may forcibly remove a mark. Root can always do this.
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		type Signature: Verify<Signer = Self::Signer> + Parameter;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// streams stored on chain.
	/// It maps from stream Id to its details.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, StreamDetails<T>, OptionQuery>;

	/// stream commit details stored on chain.
	/// It maps from a stream Id to a vector of commit details.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, Vec<StreamCommit<T>>, OptionQuery>;

	/// stream deposit details stored on chain.
	/// It maps from a stream Id to the author and deposit.
	#[pallet::storage]
	#[pallet::getter(fn deposit_of)]
	pub type DepositOf<T> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		(CordAccountOf<T>, BalanceOf<T>),
		OptionQuery,
	>;
	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn hashes_of)]
	pub type HashesOf<T> = StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn expired_of)]
	pub type ExpiredOf<T> =
		StorageMap<_, Blake2_128Concat, SignatureOf<T>, IdentifierOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream identifier, controller\]
		Anchor(IdentifierOf, HashOf<T>, CordAccountOf<T>),
		/// A stream has been updated.
		/// \[stream identifier, controller\]
		Update(IdentifierOf, HashOf<T>, CordAccountOf<T>),
		/// A stream status has been changed.
		/// \[stream identifier\]
		Status(IdentifierOf, CordAccountOf<T>),
		/// A stream has been removed.
		/// \[stream identifier\]
		Remove(IdentifierOf, CordAccountOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stream idenfier is not unique
		StreamAlreadyAnchored,
		/// Stream idenfier not found
		StreamNotFound,
		/// Stream idenfier marked inactive
		StreamRevoked,
		/// No stream status change required
		StatusChangeNotRequired,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Stream link does not exist
		StreamLinkNotFound,
		/// Stream Link is revoked
		StreamLinkRevoked,
		// Unable to pay stream deposit
		UnableToPayDeposit,
		// Invalid creator signature
		InvalidSignature,
		// Author deposit details not found
		DepositDetailsNotFound,
		//Stream has is not unique
		HashAlreadyAnchored,
		// Expired Tx Signature
		ExpiredSignature,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its controller.
		///
		/// * origin: the identity of the Tx Author.
		/// * identifier: unique identifier of the incoming stream.
		/// * creator: creator (controller) of the stream.
		/// * stream_hash: hash of the incoming stream.
		/// * holder: \[OPTIONAL\] holder (recipient) of the stream.
		/// * schema: \[OPTIONAL\] stream schema identifier.
		/// * link: \[OPTIONAL\] stream link identifier.
		/// * tx_signature: creator signature.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(6, 4))]
		pub fn create(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			creator: CordAccountOf<T>,
			stream_hash: HashOf<T>,
			holder: Option<CordAccountOf<T>>,
			schema: Option<IdentifierOf>,
			link: Option<IdentifierOf>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&stream_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);
			pallet_schema::SchemaDetails::<T>::is_valid_identifier(
				&identifier,
				STREAM_IDENTIFIER_PREFIX,
			)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);
			ensure!(!<HashesOf<T>>::contains_key(&stream_hash), Error::<T>::HashAlreadyAnchored);

			if let Some(ref schema) = schema {
				pallet_schema::SchemaDetails::<T>::schema_status(schema, creator.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}

			if let Some(ref link) = link {
				let link_details =
					<Streams<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!link_details.revoked, Error::<T>::StreamLinkRevoked);
			}

			let now_block_number = frame_system::Pallet::<T>::block_number();
			let deposit = T::Deposit::get();
			T::Currency::reserve(&author, deposit).map_err(|_| Error::<T>::UnableToPayDeposit)?;
			<DepositOf<T>>::insert(&identifier, (author, deposit));
			<HashesOf<T>>::insert(&stream_hash, &identifier);

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit { block: now_block_number, commit: StreamCommitOf::Genesis },
			)?;

			<Streams<T>>::insert(
				&identifier,
				StreamDetails {
					stream_hash: stream_hash.clone(),
					controller: creator.clone(),
					holder,
					schema,
					link,
					revoked: false,
				},
			);
			<ExpiredOf<T>>::insert(tx_signature, &identifier);

			Self::deposit_event(Event::Anchor(identifier, stream_hash, creator));

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identity of the Tx Author.
		/// * identifier: unique identifier of the incoming stream.
		/// * stream_hash: hash of the incoming stream.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(2, 4))]
		pub fn update(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			stream_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<HashesOf<T>>::contains_key(&stream_hash), Error::<T>::HashAlreadyAnchored);

			let tx_prev_details =
				<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!tx_prev_details.revoked, Error::<T>::StreamRevoked);
			let updater = tx_prev_details.controller.clone();
			ensure!(
				tx_signature.verify(&(&stream_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			let now_block_number = frame_system::Pallet::<T>::block_number();

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit { block: now_block_number, commit: StreamCommitOf::Update },
			)?;
			<HashesOf<T>>::insert(&stream_hash, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamDetails { stream_hash: stream_hash.clone(), ..tx_prev_details },
			);
			<ExpiredOf<T>>::insert(tx_signature, &identifier);
			Self::deposit_event(Event::Update(identifier, stream_hash, updater));

			Ok(())
		}
		/// Update the status of the stream
		///
		/// * origin: the identity of the Tx Author.
		/// * identifier: unique identifier of the stream.
		/// * status: stream revocation status (bool).
		/// * tx_signature: signature of the contoller.
		#[pallet::weight(30_000 + T::DbWeight::get().reads_writes(2, 3))]
		pub fn status(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			status: StatusOf,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<ExpiredOf<T>>::contains_key(&tx_signature), Error::<T>::ExpiredSignature);

			let tx_status = <Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(tx_status.revoked != status, Error::<T>::StatusChangeNotRequired);
			let updater = tx_status.controller.clone();
			ensure!(
				tx_signature.verify(&(&tx_status.stream_hash).encode()[..], &updater),
				Error::<T>::InvalidSignature
			);

			let now_block_number = frame_system::Pallet::<T>::block_number();

			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit { block: now_block_number, commit: StreamCommitOf::Status },
			)?;
			<Streams<T>>::insert(&identifier, StreamDetails { revoked: status, ..tx_status });
			<ExpiredOf<T>>::insert(tx_signature, &identifier);
			Self::deposit_event(Event::Status(identifier, updater));

			Ok(())
		}
		///  Remove a stream from the chain.
		///
		/// * origin: the identity of the Tx Author.
		/// * identifier: unique identifier of the incoming stream.
		/// * tx_signature: signature of the contoller.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(3, 3))]
		pub fn remove(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(!<ExpiredOf<T>>::contains_key(&tx_signature), Error::<T>::ExpiredSignature);

			let tx_stream = <Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			let creator = tx_stream.controller.clone();
			ensure!(
				tx_signature.verify(&(&tx_stream.stream_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			let now_block_number = frame_system::Pallet::<T>::block_number();
			let (author, deposit) =
				<DepositOf<T>>::get(&identifier).ok_or(Error::<T>::DepositDetailsNotFound)?;

			<Streams<T>>::remove(&identifier);
			StreamCommit::<T>::store_tx(
				&identifier,
				StreamCommit { block: now_block_number, commit: StreamCommitOf::Remove },
			)?;
			T::Currency::unreserve(&author, deposit);
			<ExpiredOf<T>>::insert(tx_signature, &identifier);
			Self::deposit_event(Event::Remove(identifier, creator));

			Ok(())
		}
	}
}
