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

use super::*;
extern crate alloc;
use alloc::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;

benchmarks! {
	set_profile {
		let n in 1 .. T::MaxDataKeyLength::get();
		let creator: T::AccountId = account("creator", 0, 0);
		let mut data = vec![];
		for i in 0 .. n {
			let key = format!("pub_key{}", i).as_bytes().to_vec();
			let key_bounded: DataKeyOf<T> = key.try_into().expect("Key should fit within bounds");
			let value = vec![1u8; 100]; // Reasonable value size
			let value_bounded: DataValueOf<T> = value.try_into().expect("Value should fit within bounds");
			data.push((key_bounded, value_bounded));
		}
	}: _(RawOrigin::Signed(creator.clone()), data.clone())
	verify {
		let digest = T::Hashing::hash(&creator.encode());
		let pallet_name = <Pallet<T> as frame_support::traits::PalletInfoAccess>::name();
		let profile_id = pallet_doken::Pallet::<T>::build(&digest.encode()[..], pallet_name)
			.expect("Profile ID should be created");
		assert!(Profiles::<T>::contains_key(&profile_id), "Profile should exist");
		assert_eq!(AccountProfiles::<T>::get(&creator), Some(profile_id.clone()), "Account should be linked to profile");
		for (key, value) in data {
			assert_eq!(ProfileData::<T>::get(&profile_id, &key), Some(value), "Profile data should be stored");
		}
	}

	rotate_key {
		let creator: T::AccountId = account("creator", 0, 0);
		let new_key: T::AccountId = account("new_key", 1, 0);
		let digest = T::Hashing::hash(&creator.encode());
		let pallet_name = <Pallet<T> as frame_support::traits::PalletInfoAccess>::name();
		let profile_id = pallet_doken::Pallet::<T>::build(&digest.encode()[..], pallet_name)
			.expect("Profile ID should be created");
		let data = vec![(
			b"pub_key".to_vec().try_into().expect("Key should fit"),
			vec![1u8; 100].try_into().expect("Value should fit"),
		)];
		Pallet::<T>::set_profile(RawOrigin::Signed(creator.clone()).into(), data)
			.expect("Profile creation should succeed");
	}: _(RawOrigin::Signed(creator.clone()), new_key.clone())
	verify {
		assert_eq!(AccountProfiles::<T>::get(&creator), None, "Old account should no longer be linked");
		assert_eq!(AccountProfiles::<T>::get(&new_key), Some(profile_id.clone()), "New account should be linked");
		let profile = Profiles::<T>::get(&profile_id).expect("Profile should exist");
		assert_eq!(profile.latest_key, new_key, "Profile should have new key");
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
