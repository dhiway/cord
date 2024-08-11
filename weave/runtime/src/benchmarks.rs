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

frame_benchmarking::define_benchmarks!(
	[frame_system, SystemBench::<Runtime>]
	[pallet_balances, Balances]
	[pallet_session, SessionBench::<Runtime>]
	[pallet_timestamp, Timestamp]
	[pallet_message_queue, MessageQueue]
	[pallet_sudo, Sudo]
	[pallet_collator_selection, CollatorSelection]
	[cumulus_pallet_parachain_system, ParachainSystem]
	[cumulus_pallet_xcmp_queue, XcmpQueue]
);
