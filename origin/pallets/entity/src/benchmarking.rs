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
use crate::{Event as EntityEvent, Pallet as EntityPallet};
use alloc::{vec, vec::Vec};
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_system::{Pallet as System, RawOrigin};
use origin_primitives::{attribute::Attribute, packet::Element};
use pallet_token::Token;
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
		let info = T::EntityInfoPacket::create_info();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), Box::new(info.clone()));

		// rebuild the token exactly as the pallet does:
		let digest = <T as frame_system::Config>::Hashing::hash(
			&(info.clone(), b"EntityInfoSet".to_vec()).encode(),
		);
		let token = T::Token::build(digest.as_ref(), EntityPallet::<T>::name())
			.map_err(|_| BenchmarkError::Stop("Token creation failed"))?;

		assert_last_event::<T>(
			EntityEvent::<T>::EntityInfoSet { who: caller.clone(), token }.into(),
		);
		Ok(())
	}

	/// 2) rotate_attributes
	#[benchmark]
	fn rotate_attributes() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInfoPacket as Default>::default()),
		)
		.unwrap();
		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();

		// add user attributes to rotate
		let k1: Attribute = b"custom".to_vec().try_into().unwrap();
		let k2: Attribute = b"custom2".to_vec().try_into().unwrap();
		EntityPallet::<T>::add_attributes(
			RawOrigin::Signed(caller.clone()).into(),
			vec![(k1.clone(), Element::None), (k2.clone(), Element::None)],
		)
		.unwrap();

		let new_display = Element::Raw(b"disp".to_vec().try_into().unwrap());
		let new_custom = Element::Raw(b"cust".to_vec().try_into().unwrap());
		let ops = vec![(k1.clone(), new_custom), (k2.clone(), new_display)];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), ops.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeRotated {
				who: caller.clone(),
				token,
				attr: k1,
			}
			.into(),
		);
		Ok(())
	}

	/// 3) add_attributes
	#[benchmark]
	fn add_attributes() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInfoPacket as Default>::default()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		let key: Attribute = b"k".to_vec().try_into().unwrap();
		let val = Element::Raw(b"v".to_vec().try_into().unwrap());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), vec![(key.clone(), val.clone())]);

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeUpdated { who: caller.clone(), token }.into(),
		);
		Ok(())
	}

	/// 4) remove_attribute
	#[benchmark]
	fn remove_attribute() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(<T::EntityInfoPacket as Default>::default()),
		)
		.unwrap();
		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();

		// add the attr first
		EntityPallet::<T>::add_attributes(
			RawOrigin::Signed(caller.clone()).into(),
			vec![(b"x".to_vec().try_into().unwrap(), Element::None)],
		)
		.unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), b"x".to_vec().try_into().unwrap());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeRemoved {
				who: caller.clone(),
				token,
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
			Box::new(<T::EntityInfoPacket as Default>::default()),
		)
		.unwrap();
		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();

		// add attribute so rotation can happen
		EntityPallet::<T>::add_attributes(
			RawOrigin::Signed(caller.clone()).into(),
			vec![(b"r".to_vec().try_into().unwrap(), Element::None)],
		)
		.unwrap();

		let new_val = Element::Raw(b"z".to_vec().try_into().unwrap());
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), b"r".to_vec().try_into().unwrap(), new_val.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityAttributeRotated {
				who: caller.clone(),
				token,
				attr: b"r".to_vec().try_into().unwrap(),
			}
			.into(),
		);
		Ok(())
	}

	/// 6) set_linked_account
	#[benchmark]
	fn set_linked_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);
		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();
		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityLinkedAccountAdded { account: sub, token }.into(),
		);
		Ok(())
	}

	/// 7) revoke_linked_account
	#[benchmark]
	fn revoke_linked_account() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_linked_account(
			RawOrigin::Signed(caller.clone()).into(),
			sub.clone(),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), sub.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityLinkedAccountRevoked { account: sub, token }.into(),
		);
		Ok(())
	}

	/// 8) revoke_linked_account_for
	#[benchmark]
	fn revoke_linked_account_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let sub: T::AccountId = account("s", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_linked_account(
			RawOrigin::Signed(caller.clone()).into(),
			sub.clone(),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, token.clone(), sub.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityLinkedAccountRevokedFor { account: sub, token }.into(),
		);
		Ok(())
	}

	/// 9) rotate_controller
	#[benchmark]
	fn rotate_controller() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc: T::AccountId = account("n", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), token.clone(), newc.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityControllerRotated { token, new: newc }.into(),
		);
		Ok(())
	}

	/// 10) rotate_controller_for
	#[benchmark]
	fn rotate_controller_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let newc: T::AccountId = account("n", 0, 0);

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, token.clone(), newc.clone());

		assert_last_event::<T>(
			EntityEvent::<T>::EntityControllerRotated { token, new: newc }.into(),
		);
		Ok(())
	}

	/// 11) clear_everything
	#[benchmark]
	fn clear_everything() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), token.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityInfoCleared { token }.into());
		Ok(())
	}

	/// 12) clear_everything_for
	#[benchmark]
	fn clear_everything_for() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Root, token.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityInfoCleared { token }.into());
		Ok(())
	}

	/// 13) set_entity_nym
	#[benchmark]
	fn set_entity_nym() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let prefix = b"bench".to_vec();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), prefix.clone());

		let mut uname = prefix.clone();
		uname.extend(b".nym.org.in");
		let uname: Vec<u8> = uname;
		let uname = uname.try_into().unwrap();

		assert_last_event::<T>(EntityEvent::<T>::EntityNymAdded { token, name: uname }.into());
		Ok(())
	}

	/// 14) remove_entity_nym
	#[benchmark]
	fn remove_entity_nym() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		EntityPallet::<T>::set_info(
			RawOrigin::Signed(caller.clone()).into(),
			Box::new(T::EntityInfoPacket::create_info()),
		)
		.unwrap();
		EntityPallet::<T>::set_entity_nym(
			RawOrigin::Signed(caller.clone()).into(),
			b"bench".to_vec(),
		)
		.unwrap();

		let token = EntityPallet::<T>::lookup_token_of(&caller).unwrap();
		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), token.clone());

		assert_last_event::<T>(EntityEvent::<T>::EntityNymRemoved { token }.into());
		Ok(())
	}

	impl_benchmark_test_suite!(EntityPallet, crate::mock::new_test_ext(), crate::mock::Test);
}
