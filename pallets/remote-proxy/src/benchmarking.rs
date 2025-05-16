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

// Benchmarks for Remote Proxy Pallet

use super::*;
use crate::Pallet as RemoteProxy;
use alloc::{boxed::Box, vec};
use frame_benchmarking::v2::{
	account, impl_test_function, instance_benchmarks, whitelisted_caller,
};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_runtime::{
	traits::{Bounded, StaticLookup},
	BoundedVec,
};

const SEED: u32 = 0;

type BalanceOf<T> = <<T as pallet_proxy::Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

fn assert_last_event<T: pallet_proxy::Config>(
	generic_event: <T as pallet_proxy::Config>::RuntimeEvent,
) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[instance_benchmarks]
mod benchmarks {
	use super::*;
	use frame_benchmarking::BenchmarkError;

	#[benchmark]
	fn remote_proxy() -> Result<(), BenchmarkError> {
		// In this case the caller is the "target" proxy
		let caller: T::AccountId = account("target", 0, SEED);
		<T as pallet_proxy::Config>::Currency::make_free_balance_be(
			&caller,
			BalanceOf::<T>::max_value() / 2u32.into(),
		);
		// ... and "real" is the traditional caller. This is not a typo.
		let real: T::AccountId = whitelisted_caller();
		let real_lookup = T::Lookup::unlookup(real.clone());
		let call: <T as pallet_proxy::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: vec![] }.into();
		let (proof, block_number, storage_root) =
			T::RemoteProxy::create_remote_proxy_proof(&caller, &real);
		BlockToRoot::<T, I>::set(BoundedVec::truncate_from(vec![(block_number, storage_root)]));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), real_lookup, None, Box::new(call), proof);

		assert_last_event::<T>(pallet_proxy::Event::ProxyExecuted { result: Ok(()) }.into());

		Ok(())
	}

	#[benchmark]
	fn register_remote_proxy_proof() -> Result<(), BenchmarkError> {
		// In this case the caller is the "target" proxy
		let caller: T::AccountId = account("target", 0, SEED);
		<T as pallet_proxy::Config>::Currency::make_free_balance_be(
			&caller,
			BalanceOf::<T>::max_value() / 2u32.into(),
		);
		// ... and "real" is the traditional caller. This is not a typo.
		let real: T::AccountId = whitelisted_caller();
		let (proof, block_number, storage_root) =
			T::RemoteProxy::create_remote_proxy_proof(&caller, &real);
		BlockToRoot::<T, I>::set(BoundedVec::truncate_from(vec![(block_number, storage_root)]));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), proof);

		Ok(())
	}

	#[benchmark]
	fn remote_proxy_with_registered_proof() -> Result<(), BenchmarkError> {
		// In this case the caller is the "target" proxy
		let caller: T::AccountId = account("target", 0, SEED);
		<T as pallet_proxy::Config>::Currency::make_free_balance_be(
			&caller,
			BalanceOf::<T>::max_value() / 2u32.into(),
		);
		// ... and "real" is the traditional caller. This is not a typo.
		let real: T::AccountId = whitelisted_caller();
		let real_lookup = T::Lookup::unlookup(real.clone());
		let call: <T as pallet_proxy::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: vec![] }.into();
		let (proof, block_number, storage_root) =
			T::RemoteProxy::create_remote_proxy_proof(&caller, &real);
		BlockToRoot::<T, I>::set(BoundedVec::truncate_from(vec![(block_number, storage_root)]));

		#[block]
		{
			frame_support::dispatch_context::run_in_context(|| {
				frame_support::dispatch_context::with_context::<
					crate::RemoteProxyContext<crate::RemoteBlockNumberOf<T, I>>,
					_,
				>(|context| {
					context.or_default().proofs.push(proof.clone());
				});

				RemoteProxy::<T, I>::remote_proxy_with_registered_proof(
					RawOrigin::Signed(caller).into(),
					real_lookup,
					None,
					Box::new(call),
				)
				.unwrap()
			})
		}

		assert_last_event::<T>(pallet_proxy::Event::ProxyExecuted { result: Ok(()) }.into());

		Ok(())
	}

	impl_benchmark_test_suite!(RemoteProxy, crate::tests::new_test_ext(), crate::tests::Test);
}
