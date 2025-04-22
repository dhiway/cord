// CORD Blockchain – https://dhiway.network
// Copyright (C) Dhiway Networks Pvt. Ltd.
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

//! # Network Membership Manager
#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;
use codec::{Decode, Encode};
pub use pallet::*;
use scale_info::TypeInfo;

#[cfg(test)]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
pub mod tests;

use frame_support::{
	pallet_prelude::TransactionSource,
	traits::{Get, OriginTrait},
	DefaultNoBound,
};
use sp_runtime::{
	impl_tx_ext_default,
	traits::{DispatchInfoOf, TransactionExtension, Zero},
	transaction_validity::{InvalidTransaction, ValidTransaction},
};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*};

pub use weights::WeightInfo;
pub mod types;
pub use crate::types::{MemberData, *};
use frame_support::pallet_prelude::Weight;
use frame_system::pallet_prelude::BlockNumberFor;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::StorageVersion};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type NetworkMembershipOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		#[pallet::constant]
		/// Maximum life span of a non-renewable membership (in number of
		/// blocks)
		type MembershipPeriod: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MaxMembersPerBlock: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	// maps author identity with expire block
	#[pallet::storage]
	pub(super) type Members<T: Config> = CountedStorageMap<
		_,
		Blake2_128Concat,
		CordAccountOf<T>,
		MemberData<BlockNumberFor<T>>,
		OptionQuery,
	>;

	/// maps block number to the list of authors set to expire at this block
	#[pallet::storage]
	pub type MembershipsExpiresOn<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<CordAccountOf<T>, T::MaxMembersPerBlock>,
		ValueQuery,
	>;

	/// maps block number to the list of authors set to renew
	#[pallet::storage]
	pub type MembershipsRenewsOn<T: Config> =
		StorageMap<_, Blake2_128Concat, CordAccountOf<T>, (), OptionQuery>;

	/// maps from a member identifier to a unit tuple
	#[pallet::storage]
	pub(crate) type MembershipBlacklist<T: Config> =
		StorageMap<_, Blake2_128Concat, CordAccountOf<T>, ()>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A membership was acquired
		MembershipAcquired { member: CordAccountOf<T> },
		/// A membership expired
		MembershipExpired { member: CordAccountOf<T> },
		/// A membership was renewed
		MembershipRenewed { member: CordAccountOf<T> },
		/// A membership was revoked
		MembershipRevoked { member: CordAccountOf<T> },
		/// A membership renew request
		MembershipRenewalRequested { member: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no member with the given ID.
		MembershipNotFound,
		/// Membership already acquired
		MembershipAlreadyAcquired,
		/// Membership Renewal already requested
		MembershipRenewalAlreadyRequested,
		/// Origin is not authorized
		OriginNotAuthorized,
		/// Rejects request if the member is added to the blacklist
		MembershipRequestRejected,
		/// Membership expired
		MembershipExpired,
		/// Max members limit exceeded
		MaxMembersExceededForTheBlock,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			if n > BlockNumberFor::<T>::zero() {
				Self::renew_or_expire_memberships(n)
			} else {
				Weight::zero()
			}
		}
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub members: BTreeMap<T::AccountId, bool>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			for (member, expires) in &self.members {
				Pallet::<T>::add_member_and_schedule_expiry(member, *expires)
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add an author. Only root or council origin can perform this
		/// action.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::nominate())]

		pub fn nominate(
			origin: OriginFor<T>,
			member: CordAccountOf<T>,
			expires: bool,
		) -> DispatchResult {
			T::NetworkMembershipOrigin::ensure_origin(origin)?;

			// Check if member already exist it should throw error
			// 'MembershipAlreadyAcquired'
			ensure!(!<Members<T>>::contains_key(&member), Error::<T>::MembershipAlreadyAcquired);

			Self::add_member_and_schedule_expiry(&member, expires);

			Self::deposit_event(Event::MembershipAcquired { member });

			Ok(())
		}

		/// Renew authorship. Only root or council orgin can perform this
		/// action.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::renew())]

		pub fn renew(origin: OriginFor<T>, member: CordAccountOf<T>) -> DispatchResult {
			T::NetworkMembershipOrigin::ensure_origin(origin)?;

			// Check if membership renewal request already exist it should throw error
			// 'MembershipRenewalAlreadyRequested'
			ensure!(
				!<MembershipsRenewsOn<T>>::contains_key(&member),
				Error::<T>::MembershipRenewalAlreadyRequested
			);

			MembershipsRenewsOn::<T>::insert(&member, ());

			Self::deposit_event(Event::MembershipRenewalRequested { member });

			Ok(())
		}

		/// Revoke a membership. Only root or council orgin can perform this
		/// action.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke())]
		pub fn revoke(origin: OriginFor<T>, member: CordAccountOf<T>) -> DispatchResult {
			T::NetworkMembershipOrigin::ensure_origin(origin)?;

			let member_details =
				<Members<T>>::get(&member).ok_or(Error::<T>::MembershipNotFound)?;

			// Remove the member from the Members storage.
			<Members<T>>::remove(&member);

			// Remove the member from the BoundedVec stored in MembershipsExpiresOn.
			MembershipsExpiresOn::<T>::try_mutate(member_details.expire_on, |members| {
				members
					.iter()
					.position(|x| x == &member)
					.map(|index| members.swap_remove(index))
					.ok_or(Error::<T>::MembershipNotFound)
			})?;

			// the membership was existing but is not anymore, decrement the provider
			let _ = frame_system::Pallet::<T>::dec_providers(&member);

			Self::deposit_event(Event::MembershipRevoked { member });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn add_member_and_schedule_expiry(member: &CordAccountOf<T>, expires: bool) {
		if expires {
			let block_number = frame_system::pallet::Pallet::<T>::block_number();
			let expire_on = block_number + T::MembershipPeriod::get();
			Members::<T>::insert(member, MemberData { expire_on });

			// the member has just been created, increment its provider
			let _ = frame_system::Pallet::<T>::inc_providers(member);

			let _ = MembershipsExpiresOn::<T>::try_mutate(expire_on, |members| {
				members
					.try_push(member.clone())
					.map_err(|_| Error::<T>::MaxMembersExceededForTheBlock)
			});
		} else {
			let expire_on = BlockNumberFor::<T>::zero();
			Members::<T>::insert(member, MemberData { expire_on });
			// the member has just been created, increment its provider
			let _ = frame_system::Pallet::<T>::inc_providers(member);
		}
	}

	fn renew_membership_and_schedule_expiry(
		member: CordAccountOf<T>,
		expire_on: BlockNumberFor<T>,
	) {
		let schedule_expiry = expire_on + T::MembershipPeriod::get();
		Members::<T>::insert(&member, MemberData { expire_on: schedule_expiry });
		let _ = MembershipsExpiresOn::<T>::try_mutate(schedule_expiry, |members| {
			members.try_push(member).map_err(|_| Error::<T>::MaxMembersExceededForTheBlock)
		});
	}

	/// perform membership renewal or expiration
	fn do_expire_or_renew_membership(
		member: CordAccountOf<T>,
		expire_on: BlockNumberFor<T>,
	) -> Weight {
		let mut call_weight: Weight = Weight::zero();

		if MembershipsRenewsOn::<T>::take(&member).is_some() {
			Self::renew_membership_and_schedule_expiry(member.clone(), expire_on);
			Self::deposit_event(Event::MembershipRenewed { member });
			call_weight += T::WeightInfo::renew();
		} else {
			Members::<T>::remove(&member);
			Self::deposit_event(Event::MembershipExpired { member });
			call_weight += T::WeightInfo::revoke();
		}

		call_weight
	}

	/// perform the membership expiry or renewal scheduled at given block
	fn renew_or_expire_memberships(block_number: BlockNumberFor<T>) -> Weight {
		let mut total_weight: Weight = Weight::zero();

		for member in MembershipsExpiresOn::<T>::take(block_number) {
			total_weight += Self::do_expire_or_renew_membership(member, block_number);
		}

		total_weight
	}
	/// check if identity is member
	pub fn is_member(member: &CordAccountOf<T>) -> bool {
		Members::<T>::contains_key(member)
	}
}

impl<T: Config> sp_runtime::traits::IsMember<T::AccountId> for Pallet<T> {
	fn is_member(member: &CordAccountOf<T>) -> bool {
		Self::is_member(member)
	}
}

impl<T: Config> network_membership::MembersCount for Pallet<T> {
	fn members_count() -> u32 {
		Members::<T>::count()
	}
}

/// The `CheckNetworkMembership` struct.
#[derive(Encode, Decode, DefaultNoBound, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckNetworkMembership<T>(PhantomData<T>);

impl<T: Config + Send + Sync> core::fmt::Debug for CheckNetworkMembership<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "CheckNetworkMembership")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> CheckNetworkMembership<T> {
	/// Create new `TransactionExtension` to check membership.
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T: Config + Send + Sync> TransactionExtension<T::RuntimeCall> for CheckNetworkMembership<T> {
	const IDENTIFIER: &'static str = "CheckNetworkMembership";
	type Implicit = ();
	type Val = ();
	type Pre = ();

	fn weight(&self, _: &T::RuntimeCall) -> sp_weights::Weight {
		<T::WeightInfo>::check_network_membership()
	}

	fn validate(
		&self,
		origin: <T as frame_system::Config>::RuntimeOrigin,
		_call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
		_: Self::Implicit,
		_inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> sp_runtime::traits::ValidateResult<Self::Val, T::RuntimeCall> {
		if let Some(who) = origin.as_signer() {
			if !<Members<T>>::contains_key(&who) {
				return Err(InvalidTransaction::Call.into());
			}
		}
		Ok((ValidTransaction::default(), (), origin))
	}

	impl_tx_ext_default!(T::RuntimeCall; prepare);
}
