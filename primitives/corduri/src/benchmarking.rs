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
use crate::pallet;
use codec::TryFrom;
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use sp_std::prelude::*;

benchmarks! {
	get_or_add_pallet_index {
		let pallet_name: &str = "TestPallet";
	}: {
		let _ = Pallet::<T>::get_or_add_pallet_index(pallet_name)?;
	}
	verify {
		let bounded_name: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
			pallet_name.as_bytes().to_vec().try_into().expect("BoundedVec creation should not fail");
		assert!(pallet::PalletIndex::<T>::contains_key(&bounded_name));
	}

	resolve_pallet_name {
		let pallet_name: &str = "TestPallet";
		let index = Pallet::<T>::get_or_add_pallet_index(pallet_name)?;
	}: {
		let _ = Pallet::<T>::resolve_pallet_name(index)?;
	}

	record_activity {
		let raw_identifier = vec![0u8; 10];
		let identifier = Ss58Identifier::try_from(raw_identifier).unwrap();
		// Create a dummy entry (a bounded vector of 10 bytes).
		let entry: EntryTypeOf = vec![1u8; 10].try_into().unwrap();
		// Create a fixed event stamp.
		let stamp = EventStamp { height: 1, index: 0 };
	}: {
		Pallet::<T>::record_activity(&identifier, entry.clone(), stamp)?;
	}
}
