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

#![cfg(test)]

use crate as pallet_offences;
use crate::{Config, SlashStrategy};
use codec::Encode;
use frame_support::{derive_impl, parameter_types, weights::Weight};
use sp_runtime::{BuildStorage, Perbill};
use sp_staking::{
	offence::{Kind, OffenceDetails},
	SessionIndex,
};

pub struct OnOffenceHandler;

parameter_types! {
	pub static OnOffencePerbill: Vec<Perbill> = Default::default();
	pub static OffenceWeight: Weight = Default::default();
}

impl<Reporter, Offender> pallet_offences::OnOffenceHandler<Reporter, Offender, Weight>
	for OnOffenceHandler
{
	fn on_offence(
		_offenders: &[OffenceDetails<Reporter, Offender>],
		_strategy: SlashStrategy,
		_offence_session: SessionIndex,
	) -> Weight {
		OffenceWeight::get()
	}
}

type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub struct Runtime {
		System: frame_system,
		Offences: pallet_offences,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = Block;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IdentificationTuple = u64;
	type OnOffenceHandler = OnOffenceHandler;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub const KIND: [u8; 16] = *b"test_report_1234";

/// Returns all offence details for the specific `kind` happened at the specific
/// time slot.
pub fn offence_reports(kind: Kind, time_slot: u128) -> Vec<OffenceDetails<u64, u64>> {
	<crate::ConcurrentReportsIndex<Runtime>>::get(kind, time_slot.encode())
		.into_iter()
		.map(|report_id| {
			<crate::Reports<Runtime>>::get(report_id)
				.expect("dangling report id is found in ConcurrentReportsIndex")
		})
		.collect()
}

#[derive(Clone)]
pub struct Offence {
	pub validator_set_count: u32,
	pub offenders: Vec<u64>,
	pub time_slot: u128,
}

impl pallet_offences::Offence<u64> for Offence {
	const ID: pallet_offences::Kind = KIND;
	type TimeSlot = u128;

	fn offenders(&self) -> Vec<u64> {
		self.offenders.clone()
	}

	fn validator_set_count(&self) -> u32 {
		self.validator_set_count
	}

	fn time_slot(&self) -> u128 {
		self.time_slot
	}

	fn session_index(&self) -> SessionIndex {
		1
	}

	fn slash_fraction(&self, offenders_count: u32) -> Perbill {
		Perbill::from_percent(5 + offenders_count * 100 / self.validator_set_count)
	}
}
