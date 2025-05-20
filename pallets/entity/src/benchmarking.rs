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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::types::Data;
use crate::Event as EntityEvent;
use crate::Pallet as EntityPallet;
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_system::{Pallet as System, RawOrigin};
use pallet_identifier::Identifier;
use sp_runtime::traits::Hash;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	let events = System::<T>::events();
	let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
	let last = &events[events.len() - 1];
	assert_eq!(&last.event, &system_event);
}

#[benchmarks]
mod benchmarks {
	use super::*;

	/// 1) set_info
	#[benchmark]
	fn set_info() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let info = T::EntityInformation::create_entity_info();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), Box::new(info.clone()));

		// rebuild the id exactly as the pallet does:
		let digest = <T as frame_system::Config>::Hashing::hash(
			&(info.clone(), b"IdentityInfoSet".to_vec()).encode(),
		);
		let id = <pallet_identifier::Pallet<T> as Identifier<T>>::build(
			digest.as_ref(),
			EntityPallet::<T>::name(),
		)
		.unwrap();

		assert_last_event::<T>(EntityEvent::<T>::EntityInfoSet { who: caller.clone(), id }.into());
		Ok(())
	}

	/// 2) update_info
	#[benchmark]
	fn update_info() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		// seed
		let info = T::EntityInformation::create_entity_info();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(info.clone()),
		)
		.unwrap();

		let ops = vec![(b"display".to_vec(), Data::None)];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ops.clone());

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		assert_last_event::<T>(
			EntityEvent::<T>::EntityInfoUpdated { who: caller.clone(), id }.into(),
		);
		Ok(())
	}

	/// 3) add_attributes
	#[benchmark]
	fn add_attributes() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInformation as Default>::default()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		let key = b"k".to_vec();
		let val = Data::Raw(b"v".to_vec().try_into().unwrap());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), vec![(key.clone(), val.clone())]);

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeUpdated { who: caller.clone(), id }.into(),
		);
		Ok(())
	}

	/// 4) remove_attribute
	#[benchmark]
	fn remove_attribute() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInformation as Default>::default()),
		)
		.unwrap();
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		// add the attr first
		EntityPallet::<T>::add_attributes(
			RawOrigin::Signed(caller.clone()).into(),
			vec![(b"x".to_vec(), Data::None)],
		)
		.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), b"x".to_vec());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeRemoved {
				who: caller.clone(),
				id,
				attr: b"x".to_vec().try_into().unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	/// 5) rotate_attribute
	#[benchmark]
	fn rotate_attribute() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInformation as Default>::default()),
		)
		.unwrap();
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		// add attribute so rotation can happen
		EntityPallet::<T>::add_attributes(
			RawOrigin::Signed(caller.clone()).into(),
			vec![(b"r".to_vec(), Data::None)],
		)
		.unwrap();

		let new_val = Data::Raw(b"z".to_vec().try_into().unwrap());
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), b"r".to_vec(), new_val.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeRotated {
				who: caller.clone(),
				id,
				attr: b"r".to_vec().try_into().unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	/// 6) set_sub_account
	#[benchmark]
	fn set_sub_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntitySubAccountAdded { sub, id }.into());
		Ok(())
	}

	/// 7) revoke_sub_account
	#[benchmark]
	fn revoke_sub_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_sub_account(RawOrigin::Signed(caller.clone()).into(), sub.clone())
			.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntitySubAccountRevoked { sub, id }.into());
		Ok(())
	}

	/// 8) revoke_sub_account_for
	#[benchmark]
	fn revoke_sub_account_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_sub_account(RawOrigin::Signed(caller.clone()).into(), sub.clone())
			.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, id.clone(), sub.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntitySubAccountRevoked { sub, id }.into());
		Ok(())
	}

	/// 9) rotate_controller
	#[benchmark]
	fn rotate_controller() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc: T::AccountId = account("n", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone(), newc.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityControllerRotated { id, new: newc }.into());
		Ok(())
	}

	/// 10) rotate_controller_for
	#[benchmark]
	fn rotate_controller_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc: T::AccountId = account("n", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, id.clone(), newc.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityControllerRotated { id, new: newc }.into());
		Ok(())
	}

	/// 11) clear_everything
	#[benchmark]
	fn clear_everything() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityInfoCleared { id }.into());
		Ok(())
	}

	/// 12) clear_everything_for
	#[benchmark]
	fn clear_everything_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, id.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityInfoCleared { id }.into());
		Ok(())
	}

	/// 13) set_id_name
	#[benchmark]
	fn set_id_name() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let prefix = b"bench".to_vec();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), prefix.clone());

		let mut uname = prefix.clone();
		uname.extend(b".myn.social");
		let uname: Vec<u8> = uname;
		let uname = uname.try_into().unwrap();

		assert_last_event::<T>(EntityEvent::<T>::Ss58IdNameAdded { id, name: uname }.into());
		Ok(())
	}

	/// 14) remove_id_name
	#[benchmark]
	fn remove_id_name() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInformation::create_entity_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_id_name(RawOrigin::Signed(caller.clone()).into(), b"bench".to_vec())
			.unwrap();

		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone());

		assert_last_event::<T>(EntityEvent::<T>::Ss58IdNameRemoved { id }.into());
		Ok(())
	}

	impl_benchmark_test_suite!(EntityPallet, crate::mock::new_test_ext(), crate::mock::Test);
}
