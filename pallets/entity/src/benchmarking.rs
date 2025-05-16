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
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_system::{Pallet as System, RawOrigin};
use sp_runtime::traits::Hash;

#[cfg(test)]
use crate::Pallet as EntityPallet;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	let events = System::<T>::events();
	let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
	let last = &events[events.len() - 1];
	assert_eq!(&last.event, &system_event);
}

#[benchmarks]
mod benchmarks {
	// 1) set_identity
	#[benchmark]
	fn set_identity() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let info = T::IdentityInformation::create_identity_info();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), Box::new(info.clone()));

		assert_last_event::<T>(
			Event::<T>::IdentityInfoSet {
				who: caller.clone(),
				id: {
					// recompute id the same way pallet does
					let digest = <T as frame_system::Config>::Hashing::hash(
						&(info.clone(), b"IdentityInfoSet".to_vec()).encode(),
					);
					<pallet_identifier::Pallet<T> as Identifier<T>>::build(
						digest.as_ref(),
						EntityPallet::<T>::name(),
					)
					.unwrap()
				},
			}
			.into(),
		);
		Ok(())
	}

	// 2) update_identity
	#[benchmark]
	fn update_identity() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		// first give them an identity
		let initial = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(initial.clone()),
		)?;

		// now prepare an update
		let new_display = Data::Raw(b"bench-upd".to_vec().try_into().unwrap());
		let ops = vec![IdentityUpdateOp::SetDisplay(new_display.clone())];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ops.clone());

		// verify event
		assert_last_event::<T>(
			Event::<T>::IdentityInfoUpdated {
				who: caller.clone(),
				id: EntityPallet::<T>::lookup_id_of(&caller).unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	// 3) add_attribute
	#[benchmark]
	fn add_attribute() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;

		let key = b"k".to_vec();
		let val = Data::Raw(b"val".to_vec().try_into().unwrap());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), key.clone(), val.clone());

		assert_last_event::<T>(
			Event::<T>::IdentityAttributeUpdated {
				who: caller.clone(),
				id: EntityPallet::<T>::lookup_id_of(&caller).unwrap(),
				attribute: key.clone().try_into().unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	// 4) set_sub_account
	#[benchmark]
	fn set_sub_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub = account("s", 0, 0);

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(
			Event::<T>::SubAccountAdded {
				sub: sub.clone(),
				id: EntityPallet::<T>::lookup_id_of(&caller).unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	// 5) revoke_sub_account
	#[benchmark]
	fn revoke_sub_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub = account("s", 0, 0);

		// seed identity + add sub
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		EntityPallet::<T>::set_sub_account(RawOrigin::Signed(caller.clone()).into(), sub.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(
			Event::<T>::SubAccountRevoked {
				sub: sub.clone(),
				id: EntityPallet::<T>::lookup_id_of(&caller).unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	// 6) revoke_sub_account_for
	#[benchmark]
	fn revoke_sub_account_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub = account("s", 0, 0);

		// seed identity + add sub
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		EntityPallet::<T>::set_sub_account(RawOrigin::Signed(caller.clone()).into(), sub.clone())?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, id.clone(), sub.clone());

		assert_last_event::<T>(
			Event::<T>::SubAccountRevoked { sub: sub.clone(), id: id.clone() }.into(),
		);
		Ok(())
	}

	// 7) rotate_controller
	#[benchmark]
	fn rotate_controller() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc = account("n", 0, 0);

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone(), newc.clone());

		assert_last_event::<T>(
			Event::<T>::ControllerRotated { id: id.clone(), new: newc.clone() }.into(),
		);
		Ok(())
	}

	// 8) rotate_controller_for
	#[benchmark]
	fn rotate_controller_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc = account("n", 0, 0);

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, id.clone(), newc.clone());

		assert_last_event::<T>(
			Event::<T>::ControllerRotated { id: id.clone(), new: newc.clone() }.into(),
		);
		Ok(())
	}

	// 9) clear_identity
	#[benchmark]
	fn clear_identity() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone());

		assert_last_event::<T>(Event::<T>::IdentityCleared { id: id.clone() }.into());
		Ok(())
	}

	// 10) clear_identity_for
	#[benchmark]
	fn clear_identity_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, id.clone());

		assert_last_event::<T>(Event::<T>::IdentityCleared { id: id.clone() }.into());
		Ok(())
	}

	// 11) set_username
	#[benchmark]
	fn set_username() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let prefix = b"bench".to_vec();

		// seed identity
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), prefix.clone());

		assert_last_event::<T>(
			Event::<T>::UserNameAdded {
				id: id.clone(),
				name: Username::<T>::try_from({
					let mut v = prefix.clone();
					v.extend(b".myn.social");
					v
				})
				.unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	// 12) remove_username
	#[benchmark]
	fn remove_username() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let prefix = b"bench".to_vec();

		// seed identity + username
		let info = T::IdentityInformation::create_identity_info();
		EntityPallet::<T>::set_identity(RawOrigin::Signed(caller.clone()).into(), Box::new(info))?;
		EntityPallet::<T>::set_username(RawOrigin::Signed(caller.clone()).into(), prefix.clone())?;
		let id = EntityPallet::<T>::lookup_id_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), id.clone());

		assert_last_event::<T>(Event::<T>::UserNameRemoved { id: id.clone() }.into());
		Ok(())
	}

	impl_benchmark_test_suite!(EntityPallet, crate::mock::new_test_ext(), crate::mock::Test,);
}
