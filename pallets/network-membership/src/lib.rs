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
use frame_support::dispatch::DispatchInfo;
pub use pallet::*;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
pub mod tests;

use frame_support::{dispatch::Weight, traits::Get};
use network_membership::MemberData;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*};

pub use weights::WeightInfo;
// pub mod types;
// pub use crate::types::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::StorageVersion};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type NetworkMembershipOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		#[pallet::constant]
		/// Maximum life span of a non-renewable membership (in number of
		/// blocks)
		type MembershipPeriod: Get<Self::BlockNumber>;
		#[pallet::constant]
		type MaxMembersPerBlock: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	// maps author identity with expire block
	#[pallet::storage]
	#[pallet::getter(fn members)]
	pub(super) type Members<T: Config> = CountedStorageMap<
		_,
		Blake2_128Concat,
		CordAccountOf<T>,
		MemberData<BlockNumberOf<T>>,
		OptionQuery,
	>;

	/// maps block number to the list of authors set to expire at this block
	#[pallet::storage]
	pub type MembershipsExpiresOn<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberOf<T>,
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
		fn on_initialize(n: T::BlockNumber) -> Weight {
			if n > T::BlockNumber::zero() {
				Self::renew_or_expire_memberships(n)
			} else {
				Weight::zero()
			}
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub members: BTreeMap<T::AccountId, MemberData<T::BlockNumber>>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { members: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for (member, member_data) in &self.members {
				Members::<T>::insert(member, member_data);
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

			if expires {
				Self::insert_membership_and_schedule_expiry(member.clone())
			} else {
				let expire_on = T::BlockNumber::zero();
				Members::<T>::insert(&member, MemberData { expire_on });
			}

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

			Self::deposit_event(Event::MembershipRevoked { member });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn insert_membership_and_schedule_expiry(member: CordAccountOf<T>) {
		let block_number = frame_system::pallet::Pallet::<T>::block_number();
		let expire_on = block_number + T::MembershipPeriod::get();
		Members::<T>::insert(&member, MemberData { expire_on });

		let _ = MembershipsExpiresOn::<T>::try_mutate(expire_on, |members| {
			members.try_push(member).map_err(|_| Error::<T>::MaxMembersExceededForTheBlock)
		});
	}

	fn renew_membership_and_schedule_expiry(member: CordAccountOf<T>, expire_on: BlockNumberOf<T>) {
		let schedule_expiry = expire_on + T::MembershipPeriod::get();
		Members::<T>::insert(&member, MemberData { expire_on: schedule_expiry });
		let _ = MembershipsExpiresOn::<T>::try_mutate(schedule_expiry, |members| {
			members.try_push(member).map_err(|_| Error::<T>::MaxMembersExceededForTheBlock)
		});
	}

	/// perform membership renewal or expiration
	fn do_expire_or_renew_membership(
		member: CordAccountOf<T>,
		expire_on: BlockNumberOf<T>,
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
	fn renew_or_expire_memberships(block_number: BlockNumberOf<T>) -> Weight {
		let mut total_weight: Weight = Weight::zero();

		for member in MembershipsExpiresOn::<T>::take(block_number) {
			total_weight += Self::do_expire_or_renew_membership(member, block_number);
		}

		total_weight
	}
	/// check if identity is member
	pub fn is_member_inner(member: &CordAccountOf<T>) -> bool {
		Members::<T>::contains_key(member)
	}
}

impl<T: Config> network_membership::traits::IsMember<T::AccountId> for Pallet<T> {
	fn is_member(member: &CordAccountOf<T>) -> bool {
		Self::is_member_inner(member)
	}
}

impl<T: Config> sp_runtime::traits::IsMember<T::AccountId> for Pallet<T> {
	fn is_member(member: &CordAccountOf<T>) -> bool {
		Self::is_member_inner(member)
	}
}

impl<T: Config> network_membership::traits::MembersCount for Pallet<T> {
	fn members_count() -> u32 {
		Members::<T>::count()
	}
}

/// The `CheckNetworkMembership` struct.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Default, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckNetworkMembership<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckNetworkMembership<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckNetworkMembership")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> CheckNetworkMembership<T> {
	/// Create new `SignedExtension` to check author permission.
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

/// Implementation of the `SignedExtension` trait for the
/// `CheckNetworkMembership` struct.
impl<T: Config + Send + Sync> SignedExtension for CheckNetworkMembership<T>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
{
	type AccountId = T::AccountId;
	type Call = T::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckNetworkMembership";

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		self.validate(who, call, info, len).map(|_| ())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		if <Members<T>>::contains_key(who) {
			Ok(ValidTransaction {
				priority: 0,
				longevity: TransactionLongevity::max_value(),
				propagate: true,
				..Default::default()
			})
		} else {
			Err(InvalidTransaction::Call.into())
		}
	}
}
