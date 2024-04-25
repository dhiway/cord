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

pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod tests;

pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_support::{ensure, traits::Get};
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use pallet_chain_space::AuthorizationIdOf;
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{CountOf, RatingOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, Twox64Concat};
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::{
		traits::{Hash, Zero},
		BoundedVec,
	};
	use sp_std::{prelude::Clone, str};

	///SS58 Asset Identifier
	pub type AssetIdOf = Ss58Identifier;

	///SS58 Asset Identifier
	pub type AssetInstanceIdOf = Ss58Identifier;

	/// Type of a creator identifier.
	pub type AssetCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of Asset quantity.
	pub type AssetQtyOf = u64;

	pub type AssetDescriptionOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetTagOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type AssetMetadataOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	pub type AssetInputEntryOf<T> =
		AssetInputEntry<AssetDescriptionOf<T>, AssetTypeOf, AssetTagOf<T>, AssetMetadataOf<T>>;

	pub type AssetEntryOf<T> = AssetEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		AssetStatusOf,
		AssetCreatorOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
		BlockNumberFor<T>,
	>;

	pub type VCAssetEntryOf<T> =
		VCAssetEntry<AssetStatusOf, AssetCreatorOf<T>, BlockNumberFor<T>, EntryHashOf<T>>;

	pub type VCAssetDistributionEntryOf<T> = VCAssetDistributionEntry<
		AssetStatusOf,
		AssetCreatorOf<T>,
		EntryHashOf<T>,
		BlockNumberFor<T>,
		AssetIdOf,
	>;

	pub type AssetDistributionEntryOf<T> = AssetDistributionEntry<
		AssetDescriptionOf<T>,
		AssetTypeOf,
		AssetStatusOf,
		AssetCreatorOf<T>,
		AssetTagOf<T>,
		AssetMetadataOf<T>,
		BlockNumberFor<T>,
		AssetIdOf,
	>;

	pub type AssetTransferEntryOf<T> =
		AssetTransferEntry<AssetIdOf, AssetInstanceIdOf, AssetCreatorOf<T>>;

	pub type AssetIssuanceEntryOf<T> = AssetIssuanceEntry<AssetIdOf, AssetCreatorOf<T>>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, AssetCreatorOf<Self>>;
		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;

		#[pallet::constant]
		type MaxAssetDistribution: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// asset entry identifiers with details stored on chain.
	#[pallet::storage]
	pub type Assets<T> = StorageMap<_, Blake2_128Concat, AssetIdOf, AssetEntryOf<T>, OptionQuery>;

	/// asset vc entry idenitfiers with details stored on chain.
	#[pallet::storage]
	pub type VCAssets<T> =
		StorageMap<_, Blake2_128Concat, AssetIdOf, VCAssetEntryOf<T>, OptionQuery>;

	/// asset entry identifiers with details stored on chain.
	#[pallet::storage]
	pub type Distribution<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		AssetIdOf,
		BoundedVec<AssetInstanceIdOf, T::MaxAssetDistribution>,
		OptionQuery,
	>;

	/// asset entry identifiers with  details stored on chain.
	#[pallet::storage]
	pub type Issuance<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AssetIdOf,
		Blake2_128Concat,
		AssetInstanceIdOf,
		AssetDistributionEntryOf<T>,
		OptionQuery,
	>;

	/// asset vc entry identifiers with details stored on chain.
	#[pallet::storage]
	pub type VCIssuance<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AssetIdOf,
		Blake2_128Concat,
		AssetInstanceIdOf,
		VCAssetDistributionEntryOf<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type AssetLookup<T> =
		StorageMap<_, Blake2_128Concat, EntryHashOf<T>, AssetIdOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new asset entry has been added.
		/// \[asset entry identifier, issuer\]
		Create { identifier: AssetIdOf, issuer: AssetCreatorOf<T> },
		/// A new asset entry has been added.
		/// \[asset entry identifier, instance identifier\]
		Issue { identifier: AssetIdOf, instance: AssetInstanceIdOf },
		/// A asset has been transfered.
		/// \[asset entry identifier, instance identifier, owner, beneficiary,
		/// \]
		Transfer {
			identifier: AssetIdOf,
			instance: AssetInstanceIdOf,
			from: AssetCreatorOf<T>,
			to: AssetCreatorOf<T>,
		},
		/// An asset (or instance) entry has a new Status now
		/// \[asset entry identifier, optional instance identifier, new status\]
		StatusChange {
			identifier: AssetIdOf,
			instance: Option<AssetInstanceIdOf>,
			status: AssetStatusOf,
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
		/// Invalid asset quantity - should be greater than zero
		InvalidAssetQty,
		/// Invalid asset type
		InvalidAssetType,
		/// Invalid Asset status type
		InvalidAssetStatus,
		/// Asset identifier not found
		AssetIdNotFound,
		/// Asset is not active
		AssetNotActive,
		/// Asset is not active
		InstanceNotActive,
		/// Not enough balance
		OverIssuanceLimit,
		/// distribution limit exceeded
		DistributionLimitExceeded,
		/// asset instance not found
		AssetInstanceNotFound,
		/// Asset is in same status as asked for
		AssetInSameState,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			entry: AssetInputEntryOf<T>,
			digest: EntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator.clone(),
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			ensure!(entry.asset_qty > 0 && entry.asset_value > 0, Error::<T>::InvalidAssetValue);
			ensure!(entry.asset_type.is_valid_asset_type(), Error::<T>::InvalidAssetType);

			// Id Digest = concat (H(<scale_encoded_entry_digest>,
			// <scale_encoded_space_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier =
				Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Asset)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Assets<T>>::contains_key(&identifier), Error::<T>::AssetIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

			<AssetLookup<T>>::insert(digest, &identifier);

			<Assets<T>>::insert(
				&identifier,
				AssetEntryOf::<T> {
					asset_detail: entry,
					asset_issuance: Zero::zero(),
					asset_status: AssetStatusOf::ACTIVE,
					asset_issuer: creator.clone(),
					created_at: block_number,
				},
			);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Create { identifier, issuer: creator });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::issue())]
		pub fn issue(
			origin: OriginFor<T>,
			entry: AssetIssuanceEntryOf<T>,
			digest: EntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&issuer,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let asset = <Assets<T>>::get(&entry.asset_id).ok_or(Error::<T>::AssetIdNotFound)?;

			ensure!(asset.asset_issuer == issuer, Error::<T>::UnauthorizedOperation);

			ensure!(AssetStatusOf::ACTIVE == asset.asset_status, Error::<T>::AssetNotActive);

			let issuance_qty = entry.asset_issuance_qty.unwrap_or(1);
			let overall_issuance = asset.asset_issuance.saturating_add(issuance_qty);

			ensure!(
				overall_issuance <= asset.asset_detail.asset_qty,
				Error::<T>::OverIssuanceLimit
			);
			let mut asset_instance = asset.clone();
			asset_instance.asset_detail.asset_qty = issuance_qty;

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&entry.asset_id.encode()[..],
					&entry.asset_owner.encode()[..],
					&space_id.encode()[..],
					&issuer.encode()[..],
					&digest.encode()[..],
				]
				.concat()[..],
			);

			let instance_id = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::AssetInstance,
			)
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
					asset_instance_detail: asset_instance.asset_detail,
					asset_instance_parent: entry.asset_id.clone(),
					asset_instance_status: AssetStatusOf::ACTIVE,
					asset_instance_issuer: issuer,
					asset_instance_owner: entry.asset_owner,
					created_at: block_number,
				},
			);

			<Assets<T>>::insert(
				&entry.asset_id,
				AssetEntryOf::<T> { asset_issuance: overall_issuance, ..asset },
			);

			Self::update_activity(&entry.asset_id, CallTypeOf::Issue).map_err(<Error<T>>::from)?;
			Self::update_activity(&instance_id, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Issue { identifier: entry.asset_id, instance: instance_id });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			entry: AssetTransferEntryOf<T>,
			_digest: EntryHashOf<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let asset = <Assets<T>>::get(&entry.asset_id).ok_or(Error::<T>::AssetIdNotFound)?;
			let instance = <Issuance<T>>::get(&entry.asset_id, &entry.asset_instance_id)
				.ok_or(Error::<T>::AssetInstanceNotFound)?;

			ensure!(instance.asset_instance_owner == owner, Error::<T>::UnauthorizedOperation);
			ensure!(
				instance.asset_instance_owner == entry.asset_owner,
				Error::<T>::UnauthorizedOperation
			);

			ensure!(AssetStatusOf::ACTIVE == asset.asset_status, Error::<T>::AssetNotActive);

			ensure!(
				AssetStatusOf::ACTIVE == instance.asset_instance_status,
				Error::<T>::InstanceNotActive
			);

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

		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::status_change())]
		pub fn status_change(
			origin: OriginFor<T>,
			asset_id: AssetIdOf,
			instance_id: Option<AssetInstanceIdOf>,
			new_status: AssetStatusOf,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let asset = <Assets<T>>::get(&asset_id).ok_or(Error::<T>::AssetIdNotFound)?;

			ensure!(asset.asset_issuer == issuer, Error::<T>::UnauthorizedOperation);

			ensure!(new_status.is_valid_status_type(), Error::<T>::InvalidAssetStatus);

			/* If instance ID is provided, only revoke the instance, not the asset */
			if let Some(ref inst_id) = instance_id {
				let instance = <Issuance<T>>::get(&asset_id, &inst_id)
					.ok_or(Error::<T>::AssetInstanceNotFound)?;
				ensure!(
					new_status.clone() != instance.asset_instance_status,
					Error::<T>::AssetInSameState
				);

				/* update the storage with new status */
				<Issuance<T>>::insert(
					&asset_id,
					&inst_id,
					AssetDistributionEntryOf::<T> {
						asset_instance_status: new_status.clone(),
						..instance
					},
				);

				Self::update_activity(&inst_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;
			} else {
				ensure!(new_status.clone() != asset.asset_status, Error::<T>::AssetInSameState);
				<Assets<T>>::insert(
					&asset_id,
					AssetEntryOf::<T> { asset_status: new_status.clone(), ..asset },
				);
				Self::update_activity(&asset_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;
			}
			Self::deposit_event(Event::StatusChange {
				identifier: asset_id,
				instance: instance_id,
				status: new_status,
			});

			Ok(())
		}

		// TODO: Set actual weights
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn vc_create(
			origin: OriginFor<T>,
			asset_qty: AssetQtyOf,
			digest: EntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator.clone(),
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			ensure!(asset_qty > 0, Error::<T>::InvalidAssetQty);

			// Id Digest = concat (H(<scale_encoded_entry_digest>,
			// <scale_encoded_space_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier =
				Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Asset)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<VCAssets<T>>::contains_key(&identifier), Error::<T>::AssetIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

			<AssetLookup<T>>::insert(digest, &identifier);

			<VCAssets<T>>::insert(
				&identifier,
				VCAssetEntryOf::<T> {
					asset_qty,
					digest,
					asset_issuance: Zero::zero(),
					asset_status: AssetStatusOf::ACTIVE,
					asset_issuer: creator.clone(),
					created_at: block_number,
				},
			);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Create { identifier, issuer: creator });

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn vc_issue(
			origin: OriginFor<T>,
			entry: AssetIssuanceEntryOf<T>,
			digest: EntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&issuer,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let asset = <VCAssets<T>>::get(&entry.asset_id).ok_or(Error::<T>::AssetIdNotFound)?;

			ensure!(asset.asset_issuer == issuer, Error::<T>::UnauthorizedOperation);

			ensure!(AssetStatusOf::ACTIVE == asset.asset_status, Error::<T>::AssetNotActive);

			let issuance_qty = entry.asset_issuance_qty.unwrap_or(1);
			let overall_issuance = asset.asset_issuance.saturating_add(issuance_qty);

			ensure!(overall_issuance <= asset.asset_qty, Error::<T>::OverIssuanceLimit);

			let mut asset_instance = asset.clone();
			asset_instance.asset_qty = issuance_qty;

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&entry.asset_id.encode()[..],
					&entry.asset_owner.encode()[..],
					&space_id.encode()[..],
					&issuer.encode()[..],
					&digest.encode()[..],
				]
				.concat()[..],
			);

			let instance_id = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::AssetInstance,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			let block_number = frame_system::Pallet::<T>::block_number();

			Distribution::<T>::try_mutate(&entry.asset_id, |dist_option| {
				let dist = dist_option.get_or_insert_with(BoundedVec::default);
				dist.try_push(instance_id.clone())
					.map_err(|_| Error::<T>::DistributionLimitExceeded)
			})?;

			<AssetLookup<T>>::insert(digest, &entry.asset_id);

			<VCIssuance<T>>::insert(
				&entry.asset_id,
				&instance_id,
				VCAssetDistributionEntryOf::<T> {
					asset_qty: issuance_qty,
					asset_instance_parent: entry.asset_id.clone(),
					digest,
					asset_instance_status: AssetStatusOf::ACTIVE,
					asset_instance_issuer: issuer,
					asset_instance_owner: entry.asset_owner,
					created_at: block_number,
				},
			);

			<VCAssets<T>>::insert(
				&entry.asset_id,
				VCAssetEntryOf::<T> { asset_issuance: overall_issuance, ..asset },
			);

			Self::update_activity(&entry.asset_id, CallTypeOf::Issue).map_err(<Error<T>>::from)?;
			Self::update_activity(&instance_id, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Issue { identifier: entry.asset_id, instance: instance_id });

			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn vc_transfer(
			origin: OriginFor<T>,
			entry: AssetTransferEntryOf<T>,
			digest: EntryHashOf<T>,
		) -> DispatchResult {
			let owner = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let asset = <VCAssets<T>>::get(&entry.asset_id).ok_or(Error::<T>::AssetIdNotFound)?;
			let instance = <VCIssuance<T>>::get(&entry.asset_id, &entry.asset_instance_id)
				.ok_or(Error::<T>::AssetInstanceNotFound)?;

			ensure!(instance.asset_instance_owner == owner, Error::<T>::UnauthorizedOperation);
			ensure!(
				instance.asset_instance_owner == entry.asset_owner,
				Error::<T>::UnauthorizedOperation
			);

			ensure!(AssetStatusOf::ACTIVE == asset.asset_status, Error::<T>::AssetNotActive);

			ensure!(
				AssetStatusOf::ACTIVE == instance.asset_instance_status,
				Error::<T>::InstanceNotActive
			);

			let block_number = frame_system::Pallet::<T>::block_number();

			<VCIssuance<T>>::insert(
				&entry.asset_id,
				&entry.asset_instance_id,
				VCAssetDistributionEntryOf::<T> {
					asset_instance_owner: entry.new_asset_owner.clone(),
					digest,
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

		#[pallet::call_index(7)]
		#[pallet::weight({0})]
		pub fn vc_status_change(
			origin: OriginFor<T>,
			asset_id: AssetIdOf,
			instance_id: Option<AssetInstanceIdOf>,
			new_status: AssetStatusOf,
		) -> DispatchResult {
			let issuer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let asset = <VCAssets<T>>::get(&asset_id).ok_or(Error::<T>::AssetIdNotFound)?;

			ensure!(asset.asset_issuer == issuer, Error::<T>::UnauthorizedOperation);

			ensure!(new_status.is_valid_status_type(), Error::<T>::InvalidAssetStatus);

			/* If instance ID is provided, only revoke the instance, not the asset */
			if let Some(ref inst_id) = instance_id {
				let instance = <VCIssuance<T>>::get(&asset_id, &inst_id)
					.ok_or(Error::<T>::AssetInstanceNotFound)?;
				ensure!(
					new_status.clone() != instance.asset_instance_status,
					Error::<T>::AssetInSameState
				);

				/* update the storage with new status */
				<VCIssuance<T>>::insert(
					&asset_id,
					&inst_id,
					VCAssetDistributionEntryOf::<T> {
						asset_instance_status: new_status.clone(),
						..instance
					},
				);

				Self::update_activity(&inst_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;
			} else {
				ensure!(new_status.clone() != asset.asset_status, Error::<T>::AssetInSameState);
				<VCAssets<T>>::insert(
					&asset_id,
					VCAssetEntryOf::<T> { asset_status: new_status.clone(), ..asset },
				);
				Self::update_activity(&asset_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;
			}
			Self::deposit_event(Event::StatusChange {
				identifier: asset_id,
				instance: instance_id,
				status: new_status,
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
		let _ = IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Asset, tx_entry);
		Ok(())
	}

	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
