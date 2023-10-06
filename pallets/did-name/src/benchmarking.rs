// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
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
#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::{
	pallet_prelude::EnsureOrigin, sp_runtime::SaturatedConversion, traits::Get, BoundedVec,
};
use frame_system::RawOrigin;
use sp_runtime::app_crypto::sr25519;
use sp_std::{vec, vec::Vec};

use cord_utilities::traits::GenerateBenchmarkOrigin;

const CALLER_SEED: u32 = 0;
const OWNER_SEED: u32 = 1;

fn generate_did_name_input(length: usize) -> Vec<u8> {
	let max_length = length.saturating_sub(10_usize);
	let ones_vec = vec![b'a'; max_length];
	let cord_vec = "@cord".as_bytes().to_vec(); // Convert the string "@cord" to a byte vector
	let mut name_vec = ones_vec;
	name_vec.extend(cord_vec);
	name_vec
}

benchmarks! {
	where_clause {
		where
		T::AccountId: From<sr25519::Public>,
		T::DidNameOwner: From<T::AccountId>,
		T::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::DidNameOwner>,
		T::BanOrigin: EnsureOrigin<T::RuntimeOrigin>,
	}

	register {
		let n in (T::MinNameLength::get().saturating_add(10)) .. (T::MaxNameLength::get());
		let caller: AccountIdOf<T> = account("caller", 0, CALLER_SEED);
		let owner: DidNameOwnerOf<T> = account("owner", 0, OWNER_SEED);
		let did_name_input: BoundedVec<u8, T::MaxNameLength> = BoundedVec::try_from(generate_did_name_input(n.saturated_into())).expect("BoundedVec creation should not fail.");
		let did_name_input_clone = did_name_input.clone();
		let origin = T::EnsureOrigin::generate_origin(caller, owner.clone());

	}: _<T::RuntimeOrigin>(origin, did_name_input_clone)
	verify {
		let did_name = DidNameOf::<T>::try_from(did_name_input.to_vec()).unwrap();
		assert!(Names::<T>::get(&owner).is_some());
		assert!(Owner::<T>::get(&did_name).is_some());
	}

	release {
		let caller: AccountIdOf<T> = account("caller", 0, CALLER_SEED);
		let owner: DidNameOwnerOf<T> = account("owner", 0, OWNER_SEED);
		let did_name_input: BoundedVec<u8, T::MaxNameLength> = BoundedVec::try_from(generate_did_name_input(T::MaxNameLength::get().saturated_into())).expect("BoundedVec creation should not fail.");
		let origin = T::EnsureOrigin::generate_origin(caller, owner.clone());

		Pallet::<T>::register(origin.clone(), did_name_input.clone()).expect("Should register the did name.");
	}: _<T::RuntimeOrigin>(origin)
	verify {
		let did_name = DidNameOf::<T>::try_from(did_name_input.to_vec()).unwrap();
		assert!(Names::<T>::get(&owner).is_none());
		assert!(Owner::<T>::get(&did_name).is_none());
	}

	ban {
		let n in (T::MinNameLength::get().saturating_add(10)) .. (T::MaxNameLength::get());
		let caller: AccountIdOf<T> = account("caller", 0, CALLER_SEED);
		let owner: DidNameOwnerOf<T> = account("owner", 0, OWNER_SEED);
		let did_name_input: BoundedVec<u8, T::MaxNameLength> = BoundedVec::try_from(generate_did_name_input(n.saturated_into())).expect("BoundedVec creation should not fail.");
		let did_name_input_clone = did_name_input.clone();
		let did_origin = T::EnsureOrigin::generate_origin(caller, owner.clone());
		let ban_origin = RawOrigin::Root;

		Pallet::<T>::register(did_origin, did_name_input.clone()).expect("Should register the did name.");
	}: _(ban_origin, did_name_input_clone)
	verify {
		let did_name = DidNameOf::<T>::try_from(did_name_input.to_vec()).unwrap();
		assert!(Names::<T>::get(&owner).is_none());
		assert!(Owner::<T>::get(&did_name).is_none());
		assert!(Banned::<T>::get(&did_name).is_some());
	}

	unban {
		let n in (T::MinNameLength::get().saturating_add(10)) .. (T::MaxNameLength::get());
		let caller: AccountIdOf<T> = account("caller", 0, CALLER_SEED);
		let owner: DidNameOwnerOf<T> = account("owner", 0, OWNER_SEED);
		let did_name_input: BoundedVec<u8, T::MaxNameLength> = BoundedVec::try_from(generate_did_name_input(n.saturated_into())).expect("BoundedVec creation should not fail.");
		let did_name_input_clone = did_name_input.clone();
		let ban_origin = RawOrigin::Root;

		Pallet::<T>::ban(ban_origin.clone().into(), did_name_input.clone()).expect("Should ban the did name.");
	}: _(ban_origin, did_name_input_clone)
	verify {
		let did_name = DidNameOf::<T>::try_from(did_name_input.to_vec()).unwrap();
		assert!(Names::<T>::get(&owner).is_none());
		assert!(Owner::<T>::get(&did_name).is_none());
		assert!(Banned::<T>::get(&did_name).is_none());
	}

	impl_benchmark_test_suite! (
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	)
}
