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

use core::marker::PhantomData;
use cumulus_pallet_xcmp_queue::weights::SubstrateWeight;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> cumulus_pallet_xcmp_queue::WeightInfo for WeightInfo<T> {
	fn set_config_with_u32() -> Weight {
		SubstrateWeight::<T>::set_config_with_u32()
	}
	fn enqueue_n_bytes_xcmp_message(n: u32) -> Weight {
		SubstrateWeight::<T>::enqueue_n_bytes_xcmp_message(n)
	}
	fn enqueue_n_empty_xcmp_messages(n: u32) -> Weight {
		SubstrateWeight::<T>::enqueue_n_empty_xcmp_messages(n)
	}
	fn enqueue_empty_xcmp_message_at(n: u32) -> Weight {
		SubstrateWeight::<T>::enqueue_empty_xcmp_message_at(n)
	}
	fn enqueue_n_full_pages(n: u32) -> Weight {
		SubstrateWeight::<T>::enqueue_n_full_pages(n)
	}
	fn enqueue_1000_small_xcmp_messages() -> Weight {
		SubstrateWeight::<T>::enqueue_1000_small_xcmp_messages()
	}
	fn suspend_channel() -> Weight {
		SubstrateWeight::<T>::suspend_channel()
	}
	fn resume_channel() -> Weight {
		SubstrateWeight::<T>::resume_channel()
	}
	fn take_first_concatenated_xcm(n: u32) -> Weight {
		SubstrateWeight::<T>::take_first_concatenated_xcm(n)
	}
	fn on_idle_good_msg() -> Weight {
		SubstrateWeight::<T>::on_idle_good_msg()
	}
	fn on_idle_large_msg() -> Weight {
		SubstrateWeight::<T>::on_idle_large_msg()
	}
}
