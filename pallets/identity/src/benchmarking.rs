// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

//! Identity pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Pallet as Identity;
use frame_benchmarking::v1::{account, benchmarks, whitelisted_caller, BenchmarkError};
use frame_support::{
	ensure,
	traits::{EnsureOrigin, Get},
};
use frame_system::RawOrigin;

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

// Adds `r` registrars to the Identity Pallet. These registrars will have set
// fees and fields.
fn add_registrars<T: Config>(r: u32) -> Result<(), &'static str> {
	for i in 0..r {
		let registrar: T::AccountId = account("registrar", i, SEED);
		let registrar_lookup = T::Lookup::unlookup(registrar.clone());
		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, registrar_lookup)?;
	}
	assert_eq!(Registrars::<T>::get().len(), r as usize);
	Ok(())
}

// This creates an `IdentityInfo` object with `num_fields` extra fields.
// All data is pre-populated with some arbitrary bytes.
fn create_identity_info<T: Config>(num_fields: u32) -> IdentityInfo<T::MaxAdditionalFields> {
	let data = Data::Raw(vec![0; 64].try_into().unwrap());

	IdentityInfo {
		additional: vec![(data.clone(), data.clone()); num_fields as usize].try_into().unwrap(),
		display: data.clone(),
		legal: data.clone(),
		web: data.clone(),
		email: data,
	}
}

benchmarks! {
	add_registrar {
		let r in 1 .. T::MaxRegistrars::get() - 1 => add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r + 1, SEED));
	}: _<T::RuntimeOrigin>(origin, account)
	verify {
		ensure!(Registrars::<T>::get().len() as u32 == r + 1, "Registrars not added.");
	}
	remove_registrar {
		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin = T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r - 1, SEED));
	}: _<T::RuntimeOrigin>(origin, account)
	verify {
		ensure!(Registrars::<T>::get().len() as u32 == r - 1, "Registrar not removed.");
	}
	set_identity {
		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get();
		let caller = {
			// The target user
			let caller: T::AccountId = whitelisted_caller();
			let caller_lookup = T::Lookup::unlookup(caller.clone());
			let caller_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(caller.clone()).into();
			caller
		};
	}: _(RawOrigin::Signed(caller.clone()), Box::new(create_identity_info::<T>(x)))
	verify {
		assert_last_event::<T>(Event::<T>::IdentitySet { who: caller }.into());
	}

	clear_identity {
		let caller: T::AccountId = whitelisted_caller();
		let caller_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		let caller_lookup = <T::Lookup as StaticLookup>::unlookup(caller);

		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get();

		// Create their main identity with x additional fields
		let info = create_identity_info::<T>(x);
		let caller: T::AccountId = whitelisted_caller();
		let caller_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info.clone()))?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(caller_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				caller_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&info),
			)?;
		}
		ensure!(IdentityOf::<T>::contains_key(&caller), "Identity does not exist.");
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		ensure!(!IdentityOf::<T>::contains_key(&caller), "Identity not cleared.");
	}

	request_judgement {
		let caller: T::AccountId = whitelisted_caller();

		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get() => {
			// Create their main identity with x additional fields
			let info = create_identity_info::<T>(x);
			let digest = T::Hashing::hash_of(&info);
			let caller: T::AccountId = whitelisted_caller();
			let caller_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller));
			Identity::<T>::set_identity(caller_origin, Box::new(info))?;
		};
		let registrar = Registrars::<T>::get()[r as usize - 1].clone();
		let digest_info = create_identity_info::<T>(x);
		let digest = T::Hashing::hash_of(&digest_info);

	}: _(RawOrigin::Signed(caller.clone()), registrar.clone())
	verify {
		assert_last_event::<T>(Event::<T>::JudgementRequested { who: caller, registrar, digest}.into());
	}

	cancel_request {
		let caller: T::AccountId = whitelisted_caller();
		let caller_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));

		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get() => {
			// Create their main identity with x additional fields
			let info = create_identity_info::<T>(x);
			let caller: T::AccountId = whitelisted_caller();
			let caller_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller));
			Identity::<T>::set_identity(caller_origin, Box::new(info))?;
		};

		let registrar = Registrars::<T>::get()[r as usize - 1].clone();
		Identity::<T>::request_judgement(caller_origin,registrar.clone(), )?;
	}: _(RawOrigin::Signed(caller.clone()), registrar.clone())
	verify {
		assert_last_event::<T>(Event::<T>::JudgementUnrequested { who: caller, registrar}.into());
	}


	provide_judgement {
		// The user
		let user: T::AccountId = account("user", r, SEED);
		let user_origin = <T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(user.clone()));
		let user_lookup = <T::Lookup as StaticLookup>::unlookup(user.clone());

		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		let r in 1 .. T::MaxRegistrars::get() - 1 => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get();

		let info = create_identity_info::<T>(x);
		let info_hash = T::Hashing::hash_of(&info);
		Identity::<T>::set_identity(user_origin.clone(), Box::new(info))?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;
		Identity::<T>::request_judgement(user_origin, caller.clone(), )?;
	}: _(RawOrigin::Signed(caller.clone()), user_lookup, Judgement::Reasonable, info_hash)
	verify {
		assert_last_event::<T>(Event::<T>::JudgementGiven { target: user, registrar: caller}.into())
	}

	kill_identity {
		let r in 1 .. T::MaxRegistrars::get() => add_registrars::<T>(r)?;
		let x in 0 .. T::MaxAdditionalFields::get();

		let target: T::AccountId = account("target", 0, SEED);
		let target_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(target.clone()).into();
		let target_lookup = T::Lookup::unlookup(target.clone());

		let info = create_identity_info::<T>(x);
		Identity::<T>::set_identity(target_origin.clone(), Box::new(info.clone()))?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(target_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				target_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&info),
			)?;
		}
		ensure!(IdentityOf::<T>::contains_key(&target), "Identity not set");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(origin, target_lookup)
	verify {
		ensure!(!IdentityOf::<T>::contains_key(&target), "Identity not removed");
	}

	impl_benchmark_test_suite!(Identity, crate::tests::new_test_ext(), crate::tests::Test);
}
