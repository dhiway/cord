// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Benchmarking of Dhi-Treasury

#![cfg(feature = "runtime-benchmarks")]

use super::{Pallet as Treasury, *};

use frame_benchmarking::{account, benchmarks_instance_pallet};
use frame_support::{ensure, traits::OnInitialize};
use frame_system::RawOrigin;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a treasury `propose_spend`.
fn setup_proposal<T: Config<I>, I: 'static>(
	u: u32,
) -> (T::AccountId, BalanceOf<T, I>, <T::Lookup as StaticLookup>::Source) {
	let caller = account("caller", u, SEED);
	let value: BalanceOf<T, I> = T::ProposalBondMinimum::get().saturating_mul(100u32.into());
	let _ = T::Currency::make_free_balance_be(&caller, value);
	let beneficiary = account("beneficiary", u, SEED);
	let beneficiary_lookup = T::Lookup::unlookup(beneficiary);
	(caller, value, beneficiary_lookup)
}

// Create proposals that are approved for use in `on_initialize`.
fn create_approved_proposals<T: Config<I>, I: 'static>(n: u32) -> Result<(), &'static str> {
	for i in 0..n {
		let (caller, value, lookup) = setup_proposal::<T, I>(i);
		Treasury::<T, I>::propose_spend(RawOrigin::Signed(caller).into(), value, lookup)?;
		let proposal_id = <ProposalCount<T, I>>::get() - 1;
		Treasury::<T, I>::approve_proposal(RawOrigin::Root.into(), proposal_id)?;
	}
	ensure!(<Approvals<T, I>>::get().len() == n as usize, "Not all approved");
	Ok(())
}

fn setup_pot_account<T: Config<I>, I: 'static>() {
	let pot_account = Treasury::<T, I>::account_id();
	let value = T::Currency::minimum_balance().saturating_mul(1_000_000_000u32.into());
	let _ = T::Currency::make_free_balance_be(&pot_account, value);
}

benchmarks_instance_pallet! {
	propose_spend {
		let (caller, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		// Whitelist caller account from further DB operations.
		let caller_key = frame_system::Account::<T>::hashed_key_for(&caller);
		frame_benchmarking::benchmarking::add_to_whitelist(caller_key.into());
	}: _(RawOrigin::Signed(caller), value, beneficiary_lookup)

	reject_proposal {
		let (caller, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		Treasury::<T, _>::propose_spend(
			RawOrigin::Signed(caller).into(),
			value,
			beneficiary_lookup
		)?;
		let proposal_id = Treasury::<T, _>::proposal_count() - 1;
	}: _(RawOrigin::Root, proposal_id)

	approve_proposal {
		let p in 0 .. T::MaxApprovals::get() - 1;
		create_approved_proposals::<T, _>(p)?;
		let (caller, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		Treasury::<T, _>::propose_spend(
			RawOrigin::Signed(caller).into(),
			value,
			beneficiary_lookup
		)?;
		let proposal_id = Treasury::<T, _>::proposal_count() - 1;
	}: _(RawOrigin::Root, proposal_id)

	on_initialize_proposals {
		let p in 0 .. T::MaxApprovals::get();
		setup_pot_account::<T, _>();
		create_approved_proposals::<T, _>(p)?;
	}: {
		Treasury::<T, _>::on_initialize(T::BlockNumber::zero());
	}

	impl_benchmark_test_suite!(Treasury, crate::tests::new_test_ext(), crate::tests::Test);
}
