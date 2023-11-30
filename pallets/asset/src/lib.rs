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
use frame_support::{ensure, traits::Get};
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{curi::Ss58Identifier, CountOf, RatingOf};
	use frame_support::{pallet_prelude::*, Twox64Concat};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{Hash, IdentifyAccount, Verify},
		BoundedVec,
	};
	use sp_std::{prelude::Clone, str};

	///SS58 Asset Identifier
	pub type AssetIdOf = Ss58Identifier;

	///SS58 Asset Identifier
	pub type AssetInstanceIdOf = Ss58Identifier;

	/// Type of an account identifier.
	pub type CordAccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Type of an account identifier.
	pub type SignatureOf<T> = <T as Config>::Signature;

	/// Hash of the Rating.
	pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;

	pub type AssetDescriptionOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetTagOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	pub type AssetInputEntryOf<T> =
		AssetInputEntry<AssetDescriptionOf<T>, AssetTypeOf, AssetTagOf<T>, AssetMetadataOf<T>>;

	pub type AssetEntryOf<T> = AssetEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		AssetStatusOf,
		CordAccountIdOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
		BlockNumberFor<T>,
	>;

	pub type AssetDistributionEntryOf<T> = AssetDistributionEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		AssetStatusOf,
		CordAccountIdOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
		BlockNumberFor<T>,
		AssetIdOf,
	>;

	pub type AssetTransferEntryOf<T> =
		AssetTransferEntry<AssetIdOf, AssetInstanceIdOf, CordAccountIdOf<T>>;

	pub type AssetIssuanceEntryOf<T> = AssetIssuanceEntry<AssetIdOf, CordAccountIdOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + identifier::Config {
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

		#[pallet::constant]
		type MaxAssetDistribution: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// asset entry identifiers with  details stored on chain.
	#[pallet::storage]
	#[pallet::getter(fn assets)]
	pub type Assets<T> = StorageMap<_, Blake2_128Concat, AssetIdOf, AssetEntryOf<T>, OptionQuery>;

	/// asset entry identifiers with  details stored on chain.
	#[pallet::storage]
	#[pallet::getter(fn distribution)]
	pub type Distribution<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		AssetIdOf,
		BoundedVec<AssetInstanceIdOf, T::MaxAssetDistribution>,
		OptionQuery,
	>;

	/// asset entry identifiers with  details stored on chain.
	#[pallet::storage]
	#[pallet::getter(fn issued)]
	pub type Issuance<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AssetIdOf,
		Blake2_128Concat,
		AssetInstanceIdOf,
		AssetDistributionEntryOf<T>,
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
		/// \[asset entry identifier, issuer\]
		Create { identifier: AssetIdOf, issuer: CordAccountIdOf<T> },
		/// A new asset entry has been added.
		/// \[asset entry identifier, instance identifier\]
		Issue { identifier: AssetIdOf, instance: AssetInstanceIdOf },
		/// A asset has been transfered.
		/// \[asset entry identifier, instance identifier, owner, beneficiary,
		/// \]
		Transfer {
			identifier: AssetIdOf,
			instance: AssetInstanceIdOf,
			from: CordAccountIdOf<T>,
			to: CordAccountIdOf<T>,
		},
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
		/// Not enough balance
		InsufficientBalance,
		/// distribution limit exceeded
		DistributionLimitExceeded,
		/// asset instance not found
		AssetInstanceNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			entry: AssetInputEntryOf<T>,
			signature: SignatureOf<T>,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(entry.asset_qty > 0 && entry.asset_value > 0, Error::<T>::InvalidAssetValue);
			ensure!(entry.asset_type.is_valid_asset_type(), Error::<T>::InvalidAssetType);

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&issuer.encode()[..],
					&entry.asset_qty.encode()[..],
					&entry.asset_value.encode()[..],
					&entry.asset_tag.encode()[..],
					&entry.asset_meta.encode()[..],
					&entry.asset_desc.encode()[..],
					&entry.asset_type.encode()[..],
				]
				.concat()[..],
			);

			ensure!(
				signature.verify(&(digest).encode()[..], &issuer),
				Error::<T>::InvalidSignature
			);

			let identifier = Ss58Identifier::to_asset_id(&(digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Assets<T>>::contains_key(&identifier), Error::<T>::AssetIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

			<AssetLookup<T>>::insert(digest, &identifier);

			<Assets<T>>::insert(
				&identifier,
				AssetEntryOf::<T> {
					asset_detail: entry,
					asset_status: AssetStatusOf::ACTIVE,
					asset_issuer: issuer.clone(),
					created_at: block_number,
				},
			);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Create { identifier, issuer });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn issue(
			origin: OriginFor<T>,
			entry: AssetIssuanceEntryOf<T>,
			signature: SignatureOf<T>,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let mut asset = <Assets<T>>::get(&entry.asset_id).ok_or(Error::<T>::AssetIdNotFound)?;

			ensure!(asset.asset_issuer == issuer, Error::<T>::UnauthorizedOperation);

			ensure!(AssetStatusOf::ACTIVE == asset.asset_status, Error::<T>::AssetNotActive);

			let distribued_qty = Self::get_distributed_qty(&entry.asset_id);
			let issuance_qty = entry.asset_issuance_qty.unwrap_or(1);

			ensure!(
				distribued_qty + issuance_qty <= asset.asset_detail.asset_qty,
				Error::<T>::InsufficientBalance
			);

			asset.asset_detail.asset_qty = issuance_qty;

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&entry.asset_id.encode()[..],
					&entry.new_asset_owner.encode()[..],
					&issuer.encode()[..],
					&issuance_qty.encode()[..],
				]
				.concat()[..],
			);

			ensure!(
				signature.verify(&(digest).encode()[..], &issuer),
				Error::<T>::InvalidSignature
			);

			let instance_id = Ss58Identifier::to_asset_id(&(digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let block_number = frame_system::Pallet::<T>::block_number();

			Distribution::<T>::try_mutate(&entry.asset_id, |dist_option| {
				let dist = dist_option.get_or_insert_with(BoundedVec::default);
				dist.try_push(instance_id.clone())
					.map_err(|_| Error::<T>::DistributionLimitExceeded)
			})?;

			<AssetLookup<T>>::insert(digest, &entry.asset_id);

			<Issuance<T>>::insert(
				&entry.asset_id,
				&instance_id,
				AssetDistributionEntryOf::<T> {
					asset_instance_detail: asset.asset_detail,
					asset_instance_parent: entry.asset_id.clone(),
					asset_instance_status: AssetStatusOf::ACTIVE,
					asset_instance_issuer: issuer,
					asset_instance_owner: entry.new_asset_owner,
					created_at: block_number,
				},
			);

			Self::update_activity(&instance_id, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Issue { identifier: entry.asset_id, instance: instance_id });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn transfer(
			origin: OriginFor<T>,
			entry: AssetTransferEntryOf<T>,
			signature: SignatureOf<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let instance = <Issuance<T>>::get(&entry.asset_id, &entry.asset_instance_id)
				.ok_or(Error::<T>::AssetInstanceNotFound)?;

			ensure!(instance.asset_instance_owner == owner, Error::<T>::UnauthorizedOperation);

			ensure!(
				AssetStatusOf::ACTIVE == instance.asset_instance_status,
				Error::<T>::AssetNotActive
			);

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&entry.asset_id.encode()[..],
					&entry.asset_instance_id.encode()[..],
					&entry.new_asset_owner.encode()[..],
					&owner.encode()[..],
				]
				.concat()[..],
			);

			ensure!(signature.verify(&(digest).encode()[..], &owner), Error::<T>::InvalidSignature);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Issuance<T>>::insert(
				&entry.asset_id,
				&entry.asset_instance_id,
				AssetDistributionEntryOf::<T> {
					asset_instance_owner: entry.new_asset_owner.clone(),
					created_at: block_number,
					..instance
				},
			);

			Self::update_activity(&entry.asset_instance_id, CallTypeOf::Transfer)
				.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Transfer {
				identifier: entry.asset_id,
				instance: entry.asset_instance_id,
				from: owner,
				to: entry.new_asset_owner,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_distributed_qty(asset_id: &AssetIdOf) -> u32 {
		<Distribution<T>>::get(asset_id)
			.map(|bounded_vec| bounded_vec.len() as u32)
			.unwrap_or(0)
	}

	pub fn update_activity(tx_id: &AssetIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ = identifier::Pallet::<T>::update_timeline(tx_id, IdentifierTypeOf::Asset, tx_entry);
		Ok(())
	}

	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
