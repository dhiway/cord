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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod types;

pub use crate::{pallet::*, types::*};
use frame_support::{
	ensure,
	traits::{BalanceStatus, Currency, Get, OnUnbalanced, ReservableCurrency},
};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{curi::Ss58Identifier, CountOf, RatingOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, Twox64Concat};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{Hash, IdentifyAccount, Saturating, Verify},
		SaturatedConversion,
	};
	use sp_std::{prelude::Clone, str};

	///SS58 Rating Identifier
	pub type AssetIdOf = Ss58Identifier;

	/// Type of an account identifier.
	pub type CordAccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Type of an account identifier.
	pub type SignatureOf<T> = <T as Config>::Signature;

	/// Hash of the Rating.
	pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type AssetDescriptionOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetTagOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	pub type AssetInputEntryOf<T> = AssetInputEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		CordAccountIdOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
	>;

	pub type AssetEntryOf<T> = AssetEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		AssetStatusOf,
		CordAccountIdOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
		BlockNumberFor<T>,
	>;

	pub type AssetTransferEntryOf<T> = AssetTransferEntry<AssetIdOf, CordAccountIdOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = CordAccountIdOf<Self>,
		>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountIdOf<Self>> + Parameter;

		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// asset entry identifiers with  details stored on chain.
	#[pallet::storage]
	#[pallet::getter(fn assets)]
	pub type Assets<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AssetIdOf,
		Blake2_128Concat,
		EntryHashOf<T>,
		AssetEntryOf<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn asset_lookup)]
	pub type AssetLookup<T> =
		StorageMap<_, Blake2_128Concat, EntryHashOf<T>, AssetIdOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new asset entry has been added.
		/// \[asset entry identifier, entity\]
		Issuance { identifier: AssetIdOf, issuer: CordAccountIdOf<T> },
		/// A asset has been transfered.
		/// \[asset entry identifier, owner, beneficiary, \]
		Transfer { identifier: AssetIdOf, from: CordAccountIdOf<T>, to: CordAccountIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unauthorized operation
		UnauthorizedOperation,
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Invalid digest
		InvalidDigest,
		/// Invalid creator signature
		InvalidSignature,
		/// Asset already added
		AssetIdAlreadyExists,
		/// Invalid asset value - should be greater than zero
		InvalidAssetValue,
		/// Invalid asset type
		InvalidAssetType,
		/// Asset identifier not found
		AssetIdNotFound,
		/// Asset is not active
		AssetNotActive,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn issue(
			origin: OriginFor<T>,
			entry: AssetInputEntryOf<T>,
			signature: SignatureOf<T>,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(entry.asset_qty > 0 && entry.asset_value > 0, Error::<T>::InvalidAssetValue);
			ensure!(entry.asset_type.is_valid_asset_type(), Error::<T>::InvalidAssetType);

			let digest = <T as frame_system::Config>::Hashing::hash(&entry.encode().as_slice());

			ensure!(
				signature.verify(&(&digest).encode()[..], &issuer),
				Error::<T>::InvalidSignature
			);

			// Id Digest = concat (H(<scale_encoded_digest>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &issuer.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::to_asset_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Assets<T>>::contains_key(&identifier, &digest),
				Error::<T>::AssetIdAlreadyExists
			);

			let block_number = frame_system::Pallet::<T>::block_number();

			<AssetLookup<T>>::insert(&digest, &identifier);

			<Assets<T>>::insert(
				&identifier,
				&digest,
				AssetEntryOf::<T> {
					asset_entry: entry,
					asset_status: AssetStatusOf::ACTIVE,
					asset_issuer: issuer.clone(),
					created_at: block_number,
				},
			);

			Self::deposit_event(Event::Issuance { identifier, issuer });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn transfer(
			origin: OriginFor<T>,
			entry: AssetTransferEntryOf<T>,
			signature: SignatureOf<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let digest = <T as frame_system::Config>::Hashing::hash(&entry.encode().as_slice());

			let asset_details =
				<Assets<T>>::get(&entry.asset_id, &digest).ok_or(Error::<T>::AssetIdNotFound);

			ensure!(entry.transfer_qty => asset_details.asset_qty> 0, Error::<T>::InsufficientBalance);
			let stored_status = asset_details.asset_status;
			ensure!(AssetStatusOf::ACTIVE == stored_status, Error::<T>::AssetNotActive);

			ensure!(
				signature.verify(&(&digest).encode()[..], &owner),
				Error::<T>::InvalidSignature
			);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Assets<T>>::insert(
				&entry.asset_id,
				&digest,
				AssetEntryOf::<T> {
					asset_issuer: owner.clone(),
					asset_entry: entry,
					created_at: block_number,
					..asset_details
				},
			);

			Self::deposit_event(Event::Issuance { identifier: asset_id, issuer: owner });

			Ok(())
		}
	}
}
