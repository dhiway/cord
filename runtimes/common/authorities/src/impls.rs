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
//

//! Implementation of the offence handling logic.
//!
//! Offences are sorted in the `offences` pallet.
//! The offences are executed here based. The offenders are disconnected and
//! can be added to a blacklist to avoid futur connection.

#![allow(clippy::type_complexity)]

use super::pallet::*;
use frame_support::{pallet_prelude::Weight, traits::Get};
use pallet_cord_offences::{traits::OnOffenceHandler, SlashStrategy};
use sp_runtime::traits::Convert;
use sp_staking::{offence::OffenceDetails, SessionIndex};

impl<T: Config>
	OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>, Weight>
	for Pallet<T>
where
	T: pallet_session::Config<ValidatorId = <T as frame_system::Config>::AccountId>,
	T::ValidatorIdOf: Convert<
		<T as frame_system::Config>::AccountId,
		Option<<T as frame_system::Config>::AccountId>,
	>,
{
	fn on_offence(
		offenders: &[OffenceDetails<
			T::AccountId,
			pallet_session::historical::IdentificationTuple<T>,
		>],
		strategy: SlashStrategy,
		_slash_session: SessionIndex,
	) -> Weight {
		let mut consumed_weight = Weight::from_parts(0, 0);
		let mut add_db_reads_writes = |reads, writes| {
			consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
		};

		match strategy {
			SlashStrategy::BlackList =>
				for offender in offenders {
					BlackList::<T>::mutate(|blacklist| {
						if let Some(member) = T::ValidatorIdOf::convert(offender.offender.0.clone())
						{
							if !blacklist.contains(&member) {
								blacklist.push(member.clone());
								add_db_reads_writes(0, 1);
							}
							Self::mark_for_blacklist_and_removal(member);
							add_db_reads_writes(2, 1);
						}
					})
				},
			SlashStrategy::Disconnect =>
				for offender in offenders {
					if let Some(member) = T::ValidatorIdOf::convert(offender.offender.0.clone()) {
						Self::mark_for_disconnect(member);
						add_db_reads_writes(1, 1);
					}
				},
		}
		consumed_weight
	}
}
