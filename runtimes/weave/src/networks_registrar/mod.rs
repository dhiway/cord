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

//! Pallet to handle network registration.

extern crate alloc;
pub mod primitives;
pub mod traits;

pub use primitives::{Id as NetworkId, NetworkToken, LOWEST_PUBLIC_ID};
pub use traits::Registrar;

use alloc::{fmt, vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::StatusOf;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::{PhantomData, Weight},
	traits::{
		fungible::{Balanced, Credit, Inspect},
		tokens::{Fortitude, Precision, Preservation},
		Get, OnUnbalanced, StorageVersion,
	},
};
use frame_system::{self, ensure_root, ensure_signed, pallet_prelude::BlockNumberFor};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{Hash, Zero},
	};

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Debug,
	PartialEq,
	Eq,
	Clone,
)]
pub struct NetworkInfo<Account, Hash, NetworkToken, StatusOf> {
	pub(crate) manager: Account,
	pub genesis_head: Hash,
	pub token: NetworkToken,
	pub active: StatusOf,
}

pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
pub type HashOf<T> = <T as frame_system::Config>::Hash;

pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Inspect<CordAccountOf<T>>>::Balance;
type CreditOf<T> = Credit<<T as frame_system::Config>::AccountId, <T as Config>::Currency>;

pub use pallet::*;

pub trait WeightInfo {
	fn reserve() -> Weight;
	fn register() -> Weight;
	fn deregister() -> Weight;
	fn renew() -> Weight;
	fn expire() -> Weight;
}

pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
	fn reserve() -> Weight {
		Weight::zero()
	}
	fn register() -> Weight {
		Weight::zero()
	}
	fn deregister() -> Weight {
		Weight::zero()
	}
	fn renew() -> Weight {
		Weight::zero()
	}
	fn expire() -> Weight {
		Weight::zero()
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The in-code storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Balanced<CordAccountOf<Self>>;

		#[pallet::constant]
		type RegistrationFee: Get<BalanceOf<Self>>;

		#[pallet::constant]
		type RegistrationPeriod: Get<BlockNumberFor<Self>>;

		#[pallet::constant]
		type MaxEntriesPerBlock: Get<u32>;

		type FeeCollector: OnUnbalanced<CreditOf<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Registered { network_id: NetworkId, manager: CordAccountOf<T>, token: NetworkToken<T> },
		Deregistered { network_id: NetworkId },
		Reserved { network_id: NetworkId, who: CordAccountOf<T>, token: NetworkToken<T> },
		RenewalScheduled { network_id: NetworkId },
		Renewed { network_id: NetworkId },
		Expired { network_id: NetworkId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The ID is not registered in the network.
		NotRegistered,
		/// The ID is already registered and cannot be registered again.
		AlreadyRegistered,
		/// The caller is not the owner of the specified ID.
		NotOwner,
		/// The ID provided for registration has not been reserved beforehand.
		NotReserved,
		/// The maximum number of entries supported for a block has been exceeded.
		MaxEntriesExceededForTheBlock,
		/// The network manager does not have sufficient balance to pay fees.
		UnableToPayFees,
		/// The network registration has been successfully renewed.
		RegistrationRenewed,
		/// The network registration is currently active.
		ActiveRegistration,
		/// The network registration has expired and is no longer active.
		InActiveRegistration,
		/// The provided genesis hash or head is invalid.
		InvalidGenesisHash,
		/// The provided token is invalid.
		InvalidToken,
		/// The prefix in the provided token is invalid.
		InvalidPrefix,
		/// The cord genesis head is invalid or corrupted.
		InvalidCordGenesisHead,
		/// The network genesis head is invalid or corrupted.
		InvalidNetworkGenesisHead,
		/// The provided account ID is invalid or not recognized.
		InvalidAccountId,
		/// The checksum of the token is invalid or does not match.
		InvalidChecksum,
		/// The cord genesis hash does not match the expected value.
		CordGenesisMismatch,
		/// The origin of the operation is not authorized or invalid.
		BadOrigin,
		/// The token generation process failed.
		TokenGenerationFailed,
		/// The provided token does not match the expected value.
		TokenMismatch,
		/// The provided network id does not match the expected value.
		InvalidNetworkId,
	}

	/// Netoworks - maps a network to it's associated properties.
	#[pallet::storage]
	pub type Networks<T: Config> = StorageMap<
		_,
		Twox64Concat,
		NetworkId,
		NetworkInfo<CordAccountOf<T>, HashOf<T>, NetworkToken<T>, StatusOf>,
	>;

	/// Track the network IDs.
	#[pallet::storage]
	pub type NextFreeNetworkId<T> = StorageValue<_, NetworkId, ValueQuery>;

	/// Stores the expiry information indexed on block
	#[pallet::storage]
	pub type ExpiresOn<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<NetworkId, T::MaxEntriesPerBlock>,
		ValueQuery,
	>;

	/// Scheduled renewals.
	#[pallet::storage]
	pub type RenewsOn<T: Config> = StorageMap<_, Blake2_128Concat, NetworkId, (), OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _config: core::marker::PhantomData<T>,
		pub next_free_network_id: NetworkId,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig { next_free_network_id: LOWEST_PUBLIC_ID, _config: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			NextFreeNetworkId::<T>::put(self.next_free_network_id);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			if n > BlockNumberFor::<T>::zero() {
				Self::renew_or_expire_network_registrations(n)
			} else {
				Weight::zero()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register network genesis hash for a reserved Id.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			token: NetworkToken<T>,
			network_genesis_head: HashOf<T>,
		) -> DispatchResult {
			let (reserve_id, cord_genesis_hash, network_genesis_hash_from_token, account_id) =
				token.resolve().map_err(|_| Error::<T>::InvalidToken)?;

			let caller = if account_id == ensure_signed(origin.clone())? {
				account_id.clone()
			} else {
				ensure_root(origin)?;
				account_id.clone()
			};

			let network_data = Networks::<T>::get(reserve_id).ok_or(Error::<T>::NotReserved)?;

			ensure!(
				network_genesis_head != network_data.genesis_head,
				Error::<T>::AlreadyRegistered
			);

			let genesis_hash = <frame_system::Pallet<T>>::block_hash(BlockNumberFor::<T>::zero());
			ensure!(genesis_hash == cord_genesis_hash, Error::<T>::CordGenesisMismatch);

			let reserve_genesis_hash = <T as frame_system::Config>::Hashing::hash(
				&[&reserve_id.encode()[..], &caller.encode()[..]].concat()[..],
			);
			ensure!(reserve_genesis_hash == network_genesis_hash_from_token, Error::<T>::BadOrigin);

			ensure!(network_data.token == token, Error::<T>::TokenMismatch);
			ensure!(network_genesis_head != T::Hash::default(), Error::<T>::InvalidGenesisHash);
			ensure!(account_id == network_data.manager, Error::<T>::BadOrigin);

			let network_token = NetworkToken::generate(
				reserve_id,
				&cord_genesis_hash,
				&network_genesis_head,
				&caller,
				false,
			)
			.map_err(|_| Error::<T>::TokenGenerationFailed)?;

			let updated_info = NetworkInfo {
				manager: network_data.manager,
				genesis_head: network_genesis_head,
				token: network_token.clone(),
				active: true,
			};

			Networks::<T>::insert(reserve_id, updated_info);

			Self::deposit_event(Event::<T>::Registered {
				network_id: reserve_id,
				manager: caller,
				token: network_token,
			});

			Ok(())
		}

		/// Deregister a network Id.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::deregister())]
		pub fn deregister(
			origin: OriginFor<T>,
			who: Option<CordAccountOf<T>>,
			id: NetworkId,
		) -> DispatchResult {
			match who {
				Some(_owner) => {
					ensure_root(origin)?;

					ensure!(Networks::<T>::contains_key(&id), Error::<T>::NotRegistered);

					Networks::<T>::take(&id);
				},
				None => {
					let caller = ensure_signed(origin)?;

					let network_data = Networks::<T>::get(&id).ok_or(Error::<T>::NotReserved)?;
					ensure!(network_data.manager == caller, Error::<T>::NotOwner);

					Networks::<T>::take(&id);
				},
			}

			Self::deposit_event(Event::<T>::Deregistered { network_id: id });

			Ok(())
		}

		/// Reserve a Network Id on CORD.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::reserve())]
		pub fn reserve(origin: OriginFor<T>, who: Option<CordAccountOf<T>>) -> DispatchResult {
			let (who, id) = match who {
				Some(owner) => {
					ensure_root(origin)?;
					let id = NextFreeNetworkId::<T>::get().max(LOWEST_PUBLIC_ID);
					(owner, id)
				},
				None => {
					let caller = ensure_signed(origin)?;
					let id = NextFreeNetworkId::<T>::get().max(LOWEST_PUBLIC_ID);
					(caller, id)
				},
			};

			// Reserve the ID and update the next free ID
			Self::do_reserve(who, id)?;

			NextFreeNetworkId::<T>::set(id + 1);
			Ok(())
		}

		/// Schedule renewal of a network registration
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::renew())]
		pub fn renew(
			origin: OriginFor<T>,
			who: Option<CordAccountOf<T>>,
			id: NetworkId,
		) -> DispatchResult {
			match who {
				Some(owner) => {
					ensure_root(origin)?;
					Self::do_renew(owner, id)?;
				},
				None => {
					let caller = ensure_signed(origin)?;
					Self::do_renew(caller, id)?;
				},
			}

			Ok(())
		}

		/// Renew an expired network registration
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::renew())]
		pub fn renew_now(
			origin: OriginFor<T>,
			who: Option<CordAccountOf<T>>,
			id: NetworkId,
		) -> DispatchResult {
			match who {
				Some(owner) => {
					ensure_root(origin)?;
					Self::do_renew_now(owner, id)?;
				},
				None => {
					let caller = ensure_signed(origin)?;
					Self::do_renew_now(caller, id)?;
				},
			}

			Ok(())
		}

		// [TODO]
		// transafer network
		// update genesis-head
	}
}

impl<T: Config> Pallet<T> {
	fn do_reserve(who: CordAccountOf<T>, id: NetworkId) -> DispatchResult {
		ensure!(!Networks::<T>::contains_key(id), Error::<T>::AlreadyRegistered);

		let block_number = frame_system::pallet::Pallet::<T>::block_number();
		let expire_on = block_number + T::RegistrationPeriod::get();

		let imbalance = <T::Currency as Balanced<CordAccountOf<T>>>::withdraw(
			&who,
			T::RegistrationFee::get(),
			Precision::Exact,
			Preservation::Protect,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::UnableToPayFees)?;

		T::FeeCollector::on_unbalanced(imbalance);

		let genesis_hash = <frame_system::Pallet<T>>::block_hash(BlockNumberFor::<T>::zero());
		// let reserve_genesis_hash = <T as
		// frame_system::Config>::Hashing::hash(&(&who.encode()[..]));
		let reserve_genesis_hash = <T as frame_system::Config>::Hashing::hash(
			&[&id.encode()[..], &who.encode()[..]].concat()[..],
		);
		let token =
			NetworkToken::<T>::generate(id, &genesis_hash, &reserve_genesis_hash, &who, true)
				.map_err(|_| Error::<T>::InvalidPrefix)?;

		let info = NetworkInfo {
			manager: who.clone(),
			genesis_head: reserve_genesis_hash,
			token: token.clone(),
			active: false,
		};

		let _ = ExpiresOn::<T>::try_mutate(expire_on, |networks| {
			networks
				.try_push(id.clone())
				.map_err(|_| Error::<T>::MaxEntriesExceededForTheBlock)
		});

		Networks::<T>::insert(id, info);
		Self::deposit_event(Event::<T>::Reserved { network_id: id, who, token });
		Ok(())
	}

	/// Attempt to renew an expired network registration.
	fn do_renew(who: CordAccountOf<T>, id: NetworkId) -> DispatchResult {
		let network_data = Networks::<T>::get(id).ok_or(Error::<T>::NotReserved)?;
		ensure!(network_data.manager == who, Error::<T>::NotOwner);
		ensure!(network_data.active, Error::<T>::InActiveRegistration);

		let imbalance = <T::Currency as Balanced<CordAccountOf<T>>>::withdraw(
			&who,
			T::RegistrationFee::get(),
			Precision::Exact,
			Preservation::Protect,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::UnableToPayFees)?;

		T::FeeCollector::on_unbalanced(imbalance);

		RenewsOn::<T>::insert(&id, ());

		Self::deposit_event(Event::<T>::RenewalScheduled { network_id: id });

		Ok(())
	}

	/// Attempt to renew an expired network registration.
	fn do_renew_now(who: CordAccountOf<T>, id: NetworkId) -> DispatchResult {
		let network_data = Networks::<T>::get(id).ok_or(Error::<T>::NotReserved)?;
		ensure!(network_data.manager == who, Error::<T>::NotOwner);
		ensure!(!network_data.active, Error::<T>::ActiveRegistration);

		let imbalance = <T::Currency as Balanced<CordAccountOf<T>>>::withdraw(
			&who,
			T::RegistrationFee::get(),
			Precision::Exact,
			Preservation::Protect,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::UnableToPayFees)?;

		T::FeeCollector::on_unbalanced(imbalance);

		let info = NetworkInfo { active: true, ..network_data };

		Networks::<T>::insert(id, info);

		Self::deposit_event(Event::<T>::Renewed { network_id: id });
		Ok(())
	}

	fn renew_registration_and_schedule_expiry(network: NetworkId, expire_on: BlockNumberFor<T>) {
		let schedule_expiry = expire_on + T::RegistrationPeriod::get();

		if let Some(network_data) = Networks::<T>::get(network) {
			let info = NetworkInfo { active: true, ..network_data };
			Networks::<T>::insert(network, info);
		}
		let _ = ExpiresOn::<T>::try_mutate(schedule_expiry, |networks| {
			networks
				.try_push(network)
				.map_err(|_| Error::<T>::MaxEntriesExceededForTheBlock)
		});
	}

	fn do_expire_or_renew(network: NetworkId, expire_on: BlockNumberFor<T>) -> Weight {
		let mut call_weight: Weight = Weight::zero();

		if RenewsOn::<T>::take(&network).is_some() {
			Self::renew_registration_and_schedule_expiry(network.clone(), expire_on);
			Self::deposit_event(Event::<T>::Renewed { network_id: network });
			call_weight += T::WeightInfo::renew();
		} else {
			if let Some(network_data) = Networks::<T>::get(network) {
				let info = NetworkInfo { active: false, ..network_data };
				Networks::<T>::insert(network, info); // Update the network as inactive
				Self::deposit_event(Event::Expired { network_id: network });
				call_weight += T::WeightInfo::expire();
			}
		}
		call_weight
	}

	fn renew_or_expire_network_registrations(block_number: BlockNumberFor<T>) -> Weight {
		let mut total_weight: Weight = Weight::zero();

		for network in ExpiresOn::<T>::take(block_number) {
			total_weight += Self::do_expire_or_renew(network, block_number);
		}

		total_weight
	}
}

impl<T: Config> Registrar for Pallet<T> {
	type AccountId = CordAccountOf<T>;

	/// Return the manager `AccountId` of a network if one exists.
	fn manager_of(id: NetworkId) -> Option<CordAccountOf<T>> {
		Networks::<T>::get(id).map(|info| info.manager)
	}

	fn networks() -> Vec<NetworkId> {
		// Collect all keys from the storage map and return them
		Networks::<T>::iter_keys().collect()
	}

	fn is_network(id: NetworkId) -> bool {
		// Check if the given NetworkId exists in the Networks storage map
		Networks::<T>::contains_key(id)
	}

	fn is_registered(id: NetworkId) -> bool {
		// Return f the given NetworkId exists
		Self::is_network(id)
	}
}
