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

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]
use codec::Decode;
use sp_std::{prelude::*, vec};

use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use pallet_session::*;

pub struct Pallet<T: Config>(pallet_session::Pallet<T>);
pub trait Config: pallet_session::Config {}

benchmarks! {
	set_keys {
		let caller: T::AccountId = whitelisted_caller();
		frame_system::Pallet::<T>::inc_providers(&caller);
		let keys = T::Keys::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes()).unwrap();
		let proof: Vec<u8> = vec![0,1,2,3];
	}: _(RawOrigin::Signed(caller), keys, proof)

	purge_keys {
		let caller: T::AccountId = whitelisted_caller();
		frame_system::Pallet::<T>::inc_providers(&caller);
		let keys = T::Keys::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes()).unwrap();
		let proof: Vec<u8> = vec![0,1,2,3];
		let _t = pallet_session::Pallet::<T>::set_keys(RawOrigin::Signed(caller.clone()).into(), keys, proof);
	}: _(RawOrigin::Signed(caller))
}
