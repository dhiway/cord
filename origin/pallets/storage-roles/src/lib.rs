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

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{ensure, pallet_prelude::*, traits::StorageVersion};
use frame_system::{ensure_root, pallet_prelude::*};
use origin_primitives::identifier::Ss58Identifier;
use scale_info::TypeInfo;

pub use pallet::*;

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
pub enum StorageRole {
	Publisher,
	Aggregator,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type PublisherKeyByEntity<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type AggregatorKeyByEntity<T: Config> =
		StorageMap<_, Blake2_128Concat, Ss58Identifier, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type EntityByPublisherKey<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	#[pallet::storage]
	pub type EntityByAggregatorKey<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Ss58Identifier, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		PublisherRegistered { entity: Ss58Identifier, account: T::AccountId },
		AggregatorRegistered { entity: Ss58Identifier, account: T::AccountId },
		RoleKeyRotated { entity: Ss58Identifier, role: StorageRole, new_account: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		AccountAlreadyRegistered,
		EntityAlreadyRegistered,
		UnknownEntity,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())]
		pub fn register_publisher(
			origin: OriginFor<T>,
			entity: Ss58Identifier,
			account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_register(StorageRole::Publisher, entity.clone(), account.clone())?;
			Self::deposit_event(Event::PublisherRegistered { entity, account });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(Weight::zero())]
		pub fn register_aggregator(
			origin: OriginFor<T>,
			entity: Ss58Identifier,
			account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_register(StorageRole::Aggregator, entity.clone(), account.clone())?;
			Self::deposit_event(Event::AggregatorRegistered { entity, account });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(Weight::zero())]
		pub fn rotate_role_key(
			origin: OriginFor<T>,
			entity: Ss58Identifier,
			role: StorageRole,
			new_account: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_rotate(role, &entity, new_account.clone())?;
			Self::deposit_event(Event::RoleKeyRotated { entity, role, new_account });
			Ok(())
		}
	}

	#[pallet::view_functions]
	impl<T: Config> Pallet<T> {
		pub fn is_publisher_of(account: T::AccountId) -> bool {
			EntityByPublisherKey::<T>::contains_key(&account)
		}

		pub fn is_aggregator_of(account: T::AccountId) -> bool {
			EntityByAggregatorKey::<T>::contains_key(&account)
		}

		pub fn publisher_entity_of(account: T::AccountId) -> Option<Ss58Identifier> {
			EntityByPublisherKey::<T>::get(&account)
		}

		pub fn aggregator_entity_of(account: T::AccountId) -> Option<Ss58Identifier> {
			EntityByAggregatorKey::<T>::get(&account)
		}

		pub fn publisher_account_of(entity: Ss58Identifier) -> Option<T::AccountId> {
			PublisherKeyByEntity::<T>::get(entity)
		}

		pub fn aggregator_account_of(entity: Ss58Identifier) -> Option<T::AccountId> {
			AggregatorKeyByEntity::<T>::get(entity)
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn is_publisher(account: &T::AccountId) -> bool {
			EntityByPublisherKey::<T>::contains_key(account)
		}

		pub fn is_aggregator(account: &T::AccountId) -> bool {
			EntityByAggregatorKey::<T>::contains_key(account)
		}

		pub fn publisher_entity(account: &T::AccountId) -> Option<Ss58Identifier> {
			EntityByPublisherKey::<T>::get(account)
		}

		pub fn aggregator_entity(account: &T::AccountId) -> Option<Ss58Identifier> {
			EntityByAggregatorKey::<T>::get(account)
		}

		fn do_register(
			role: StorageRole,
			entity: Ss58Identifier,
			account: T::AccountId,
		) -> Result<(), Error<T>> {
			match role {
				StorageRole::Publisher => {
					ensure!(
						!EntityByPublisherKey::<T>::contains_key(&account),
						Error::<T>::AccountAlreadyRegistered
					);
					ensure!(
						!PublisherKeyByEntity::<T>::contains_key(&entity),
						Error::<T>::EntityAlreadyRegistered
					);
					EntityByPublisherKey::<T>::insert(&account, entity.clone());
					PublisherKeyByEntity::<T>::insert(&entity, &account);
				},
				StorageRole::Aggregator => {
					ensure!(
						!EntityByAggregatorKey::<T>::contains_key(&account),
						Error::<T>::AccountAlreadyRegistered
					);
					ensure!(
						!AggregatorKeyByEntity::<T>::contains_key(&entity),
						Error::<T>::EntityAlreadyRegistered
					);
					EntityByAggregatorKey::<T>::insert(&account, entity.clone());
					AggregatorKeyByEntity::<T>::insert(&entity, &account);
				},
			}
			Ok(())
		}

		fn do_rotate(
			role: StorageRole,
			entity: &Ss58Identifier,
			new_account: T::AccountId,
		) -> Result<(), Error<T>> {
			match role {
				StorageRole::Publisher => {
					let old =
						PublisherKeyByEntity::<T>::get(entity).ok_or(Error::<T>::UnknownEntity)?;
					if old != new_account {
						EntityByPublisherKey::<T>::remove(&old);
						ensure!(
							!EntityByPublisherKey::<T>::contains_key(&new_account),
							Error::<T>::AccountAlreadyRegistered
						);
						EntityByPublisherKey::<T>::insert(&new_account, entity.clone());
						PublisherKeyByEntity::<T>::insert(entity, &new_account);
					}
				},
				StorageRole::Aggregator => {
					let old =
						AggregatorKeyByEntity::<T>::get(entity).ok_or(Error::<T>::UnknownEntity)?;
					if old != new_account {
						EntityByAggregatorKey::<T>::remove(&old);
						ensure!(
							!EntityByAggregatorKey::<T>::contains_key(&new_account),
							Error::<T>::AccountAlreadyRegistered
						);
						EntityByAggregatorKey::<T>::insert(&new_account, entity.clone());
						AggregatorKeyByEntity::<T>::insert(entity, &new_account);
					}
				},
			}
			Ok(())
		}
	}
}
