// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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
//
//! # Membership Module
//!
//! Allows control of membership of a set of `AccountId`s, useful for managing
//! membership of a collective. A prime member may be set
//! Modified to work with CORD governance modules.
// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	traits::{ChangeMembers, Contains, Get, InitializeMembers, SortedMembers},
	BoundedVec,
};
use sp_runtime::traits::{IsMember, StaticLookup, UniqueSaturatedInto};
use sp_std::prelude::*;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
pub mod tests;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type IsMember: IsMember<Self::AccountId>;
		/// Required origin for adding a member (though can always be Root).
		type AddOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Required origin for removing a member (though can always be Root).
		type RemoveOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Required origin for adding and removing a member in a single action.
		type SwapOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Required origin for resetting membership.
		type ResetOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Required origin for setting or resetting the prime member.
		type PrimeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The receiver of the signal for when the membership has been
		/// initialized. This happens pre-genesis and will usually be the same
		/// as `MembershipChanged`. If you need to do something different on
		/// initialization, then you can change this accordingly.
		type MembershipInitialized: InitializeMembers<Self::AccountId>;

		/// The receiver of the signal for when the membership has changed.
		type MembershipChanged: ChangeMembers<Self::AccountId>;

		/// The maximum number of members that this membership can have.
		///
		/// This is used for benchmarking. Re-run the benchmarks if this
		/// changes.
		///
		/// This is enforced in the code; the membership size can not exceed
		/// this limit.
		type MaxMembers: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// The current membership, stored as an ordered Vec.
	#[pallet::storage]
	pub type Members<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BoundedVec<T::AccountId, T::MaxMembers>, ValueQuery>;

	/// The current prime member, if one exists.
	#[pallet::storage]
	pub type Prime<T: Config<I>, I: 'static = ()> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		pub members: BoundedVec<T::AccountId, T::MaxMembers>,
		#[serde(skip)]
		pub phantom: PhantomData<I>,
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
		fn build(&self) {
			use sp_std::collections::btree_set::BTreeSet;
			let members_set: BTreeSet<_> = self.members.iter().collect();
			assert_eq!(
				members_set.len(),
				self.members.len(),
				"Members cannot contain duplicate accounts."
			);

			let mut members = self.members.clone();
			members.sort();
			T::MembershipInitialized::initialize_members(&members);
			<Members<T, I>>::put(members);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// The given member was added; see the transaction for who.
		MemberAdded,
		/// The given member was removed; see the transaction for who.
		MemberRemoved,
		/// Two members were swapped; see the transaction for who.
		MembersSwapped,
		/// The membership was reset; see the transaction for who the new set
		/// is.
		MembersReset,
		/// One of the members' keys changed.
		KeyChanged,
		/// Phantom member, never used.
		Dummy { _phantom_data: PhantomData<(T::AccountId, <T as Config<I>>::RuntimeEvent)> },
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Already a member.
		AlreadyMember,
		/// Not a member.
		NotMember,
		/// Too many members.
		TooManyMembers,
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Add a member `who` to the set.
		///
		/// May only be called from `T::AddOrigin`.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_member(T::MaxMembers::get()))]
		pub fn add_member(
			origin: OriginFor<T>,
			who: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::AddOrigin::ensure_origin(origin)?;
			let who = T::Lookup::lookup(who)?;

			ensure!(T::IsMember::is_member(&who), Error::<T, I>::NotMember);

			let mut members = <Members<T, I>>::get();
			let init_length = members.len();
			let location = members.binary_search(&who).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members
				.try_insert(location, who.clone())
				.map_err(|_| Error::<T, I>::TooManyMembers)?;

			<Members<T, I>>::put(&members);

			T::MembershipChanged::change_members_sorted(&[who], &[], &members[..]);

			Self::deposit_event(Event::MemberAdded);

			Ok(Some(T::WeightInfo::add_member(init_length as u32)).into())
		}

		/// Remove a member `who` from the set.
		///
		/// May only be called from `T::RemoveOrigin`.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::remove_member(T::MaxMembers::get()))]
		pub fn remove_member(
			origin: OriginFor<T>,
			who: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::RemoveOrigin::ensure_origin(origin)?;
			let who = T::Lookup::lookup(who)?;

			let mut members = <Members<T, I>>::get();
			let init_length = members.len();
			let location = members.binary_search(&who).ok().ok_or(Error::<T, I>::NotMember)?;
			members.remove(location);

			<Members<T, I>>::put(&members);

			T::MembershipChanged::change_members_sorted(&[], &[who], &members[..]);
			Self::rejig_prime(&members);

			Self::deposit_event(Event::MemberRemoved);
			Ok(Some(T::WeightInfo::remove_member(init_length as u32)).into())
		}

		/// Swap out one member `remove` for another `add`.
		///
		/// May only be called from `T::SwapOrigin`.
		///
		/// Prime membership is *not* passed from `remove` to `add`, if extant.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::swap_member(T::MaxMembers::get()))]
		pub fn swap_member(
			origin: OriginFor<T>,
			remove: AccountIdLookupOf<T>,
			add: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::SwapOrigin::ensure_origin(origin)?;
			let remove = T::Lookup::lookup(remove)?;
			let add = T::Lookup::lookup(add)?;

			if remove == add {
				return Ok(().into());
			}
			ensure!(T::IsMember::is_member(&add), Error::<T, I>::NotMember);

			let mut members = <Members<T, I>>::get();
			let location = members.binary_search(&remove).ok().ok_or(Error::<T, I>::NotMember)?;
			let _ = members.binary_search(&add).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members[location] = add.clone();
			members.sort();

			<Members<T, I>>::put(&members);

			T::MembershipChanged::change_members_sorted(&[add], &[remove], &members[..]);
			Self::rejig_prime(&members);

			Self::deposit_event(Event::MembersSwapped);
			Ok(Some(T::WeightInfo::swap_member(members.len() as u32)).into())
		}

		/// Change the membership to a new set, disregarding the existing
		/// membership. Be nice and pass `members` pre-sorted.
		///
		/// May only be called from `T::ResetOrigin`.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::reset_members(members.len().unique_saturated_into()))]
		pub fn reset_members(origin: OriginFor<T>, members: Vec<T::AccountId>) -> DispatchResult {
			T::ResetOrigin::ensure_origin(origin)?;

			let mut members: BoundedVec<T::AccountId, T::MaxMembers> =
				BoundedVec::try_from(members).map_err(|_| Error::<T, I>::TooManyMembers)?;
			members.sort();
			<Members<T, I>>::mutate(|m| {
				T::MembershipChanged::set_members_sorted(&members[..], m);
				Self::rejig_prime(&members);
				*m = members;
			});

			Self::deposit_event(Event::MembersReset);
			Ok(())
		}

		/// Swap out the sending member for some other key `new`.
		///
		/// May only be called from `Signed` origin of a current member.
		///
		/// Prime membership is passed from the origin account to `new`, if
		/// extant.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::change_key(T::MaxMembers::get()))]
		pub fn change_key(
			origin: OriginFor<T>,
			new: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			let remove = ensure_signed(origin)?;
			let new = T::Lookup::lookup(new)?;

			ensure!(T::IsMember::is_member(&new), Error::<T, I>::NotMember);
			if remove == new {
				return Ok(().into());
			}

			let mut members = <Members<T, I>>::get();
			let members_length = members.len() as u32;
			let location = members.binary_search(&remove).ok().ok_or(Error::<T, I>::NotMember)?;
			let _ = members.binary_search(&new).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members[location] = new.clone();
			members.sort();

			<Members<T, I>>::put(&members);

			T::MembershipChanged::change_members_sorted(
				&[new.clone()],
				&[remove.clone()],
				&members[..],
			);

			if Prime::<T, I>::get() == Some(remove) {
				Prime::<T, I>::put(&new);
				T::MembershipChanged::set_prime(Some(new));
			}

			Self::deposit_event(Event::KeyChanged);
			Ok(Some(T::WeightInfo::change_key(members_length)).into())
		}

		/// Set the prime member. Must be a current member.
		///
		/// May only be called from `T::PrimeOrigin`.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::set_prime(T::MaxMembers::get()))]
		pub fn set_prime(
			origin: OriginFor<T>,
			who: AccountIdLookupOf<T>,
		) -> DispatchResultWithPostInfo {
			T::PrimeOrigin::ensure_origin(origin)?;
			let who = T::Lookup::lookup(who)?;
			let members = Members::<T, I>::get();
			members.binary_search(&who).ok().ok_or(Error::<T, I>::NotMember)?;
			Prime::<T, I>::put(&who);
			T::MembershipChanged::set_prime(Some(who));
			Ok(Some(T::WeightInfo::set_prime(members.len() as u32)).into())
		}

		/// Remove the prime member if it exists.
		///
		/// May only be called from `T::PrimeOrigin`.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::clear_prime())]
		pub fn clear_prime(origin: OriginFor<T>) -> DispatchResult {
			T::PrimeOrigin::ensure_origin(origin)?;
			Prime::<T, I>::kill();
			T::MembershipChanged::set_prime(None);
			Ok(())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	fn rejig_prime(members: &[T::AccountId]) {
		if let Some(prime) = Prime::<T, I>::get() {
			match members.binary_search(&prime) {
				Ok(_) => T::MembershipChanged::set_prime(Some(prime)),
				Err(_) => Prime::<T, I>::kill(),
			}
		}
	}
}

impl<T: Config<I>, I: 'static> Contains<T::AccountId> for Pallet<T, I> {
	fn contains(t: &T::AccountId) -> bool {
		Members::<T, I>::get().binary_search(t).is_ok()
	}
}

impl<T: Config<I>, I: 'static> SortedMembers<T::AccountId> for Pallet<T, I> {
	fn sorted_members() -> Vec<T::AccountId> {
		Members::<T, I>::get().to_vec()
	}

	fn count() -> usize {
		Members::<T, I>::decode_len().unwrap_or(0)
	}
}
