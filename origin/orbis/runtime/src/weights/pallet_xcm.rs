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
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_xcm::WeightInfo for WeightInfo<T> {
	fn send() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn teleport_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn reserve_transfer_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn transfer_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn execute() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_xcm_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_default_xcm_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_subscribe_version_notify() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_unsubscribe_version_notify() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_suspension() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_supported_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_version_notifiers() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn already_notified_target() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn notify_current_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn notify_target_migration_fail() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_version_notify_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_and_notify_old_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn new_query() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn take_response() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn claim_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn add_authorized_alias() -> Weight {
		Weight::from_parts(100_000, 0)
	}
	fn remove_authorized_alias() -> Weight {
		Weight::from_parts(100_000, 0)
	}
	fn weigh_message() -> Weight {
		Weight::from_parts(100_000, 0)
	}
}
