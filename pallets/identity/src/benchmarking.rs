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
use enumflags2::BitFlag;
use frame_benchmarking::{
	account, impl_benchmark_test_suite, v2::*, whitelisted_caller, BenchmarkError,
};
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

		let fields = IdentityFields(
			<T::IdentityInformation as IdentityInformationProvider>::IdentityField::all(),
		);
		Identity::<T>::set_fields(RawOrigin::Signed(registrar.clone()).into(), fields)?;
	}

	assert_eq!(Registrars::<T>::get().len(), r as usize);
	Ok(())
}

#[benchmarks]
mod benchmarks {
	use super::*;
	#[benchmark]
	fn add_registrar(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r + 1, SEED));

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, account);

		ensure!(Registrars::<T>::get().len() as u32 == r + 1, "Registrars not added.");
		Ok(())
	}

	#[benchmark]
	fn remove_registrar(
		r: Linear<1, { T::MaxRegistrars::get() - 1 }>,
	) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;
		ensure!(Registrars::<T>::get().len() as u32 == r, "Registrars not set up correctly.");
		let origin =
			T::RegistrarOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let account = T::Lookup::unlookup(account("registrar", r - 1, SEED));
		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, account);

		ensure!(Registrars::<T>::get().len() as u32 == r - 1, "Registrar not removed.");

		Ok(())
	}

	#[benchmark]
	fn set_identity(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;

		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let caller_origin: <T as frame_system::Config>::RuntimeOrigin =
			RawOrigin::Signed(caller.clone()).into();

		// Add an initial identity
		let initial_info = T::IdentityInformation::create_identity_info(1);
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(initial_info.clone()))?;

		// User requests judgement from all the registrars, and they approve
		for i in 0..r {
			let registrar: T::AccountId = account("registrar", i, SEED);

			Identity::<T>::request_judgement(caller_origin.clone(), registrar.clone())?;
			Identity::<T>::provide_judgement(
				RawOrigin::Signed(registrar).into(),
				caller_lookup.clone(),
				Judgement::Reasonable,
				T::Hashing::hash_of(&initial_info),
			)?;
		}

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller.clone()),
			Box::new(T::IdentityInformation::create_identity_info(x)),
		);

		assert_last_event::<T>(Event::<T>::IdentitySet { who: caller }.into());
		Ok(())
	}

	#[benchmark]
	fn clear_identity(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		let caller_lookup = <T::Lookup as StaticLookup>::unlookup(caller.clone());

		// Register the registrars
		add_registrars::<T>(r)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info(x);
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

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		ensure!(!IdentityOf::<T>::contains_key(&caller), "Identity not cleared.");
		Ok(())
	}

	#[benchmark]
	fn request_judgement(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Register the registrars
		add_registrars::<T>(r)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info(x);
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info))?;

		let registrar: T::AccountId = account("registrar", 0, SEED);
		// let registrar = T::Lookup::unlookup(registrar.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), registrar.clone());

		assert_last_event::<T>(Event::<T>::JudgementRequested { who: caller, registrar }.into());

		Ok(())
	}

	#[benchmark]
	fn cancel_request(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();

		// Register the registrars
		add_registrars::<T>(r)?;

		// Create their main identity with x additional fields
		let info = T::IdentityInformation::create_identity_info(x);
		let caller_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(caller.clone()));
		Identity::<T>::set_identity(caller_origin.clone(), Box::new(info))?;

		let registrar: T::AccountId = account("registrar", 0, SEED);

		Identity::<T>::request_judgement(caller_origin.clone(), registrar.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), registrar.clone());

		assert_last_event::<T>(Event::<T>::JudgementUnrequested { who: caller, registrar }.into());

		Ok(())
	}
	#[benchmark]
	fn set_account_id(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;

		let registrars = Registrars::<T>::get();
		ensure!(registrars[r as usize].as_ref().unwrap().account == caller, "id not set.");

		let new_account = T::Lookup::unlookup(account("new", 0, SEED));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), new_account);

		let updated_registrars = Registrars::<T>::get();
		ensure!(
			updated_registrars[r as usize].as_ref().unwrap().account == account("new", 0, SEED),
			"id not changed."
		);

		Ok(())
	}

	#[benchmark]
	fn set_fields(r: Linear<1, { T::MaxRegistrars::get() - 1 }>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;

		let fields = IdentityFields(
			<T::IdentityInformation as IdentityInformationProvider>::IdentityField::all(),
		);

		let registrars = Registrars::<T>::get();
		ensure!(
			registrars[r as usize].as_ref().unwrap().fields == Default::default(),
			"fields already set."
		);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), fields);

		let updated_registrars = Registrars::<T>::get();
		ensure!(
			updated_registrars[r as usize].as_ref().unwrap().fields != Default::default(),
			"fields not set."
		);

		Ok(())
	}

	#[benchmark]
	fn provide_judgement(
		r: Linear<1, { T::MaxRegistrars::get() - 1 }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		// The user
		let user: T::AccountId = account("user", r, SEED);
		let user_origin =
			<T as frame_system::Config>::RuntimeOrigin::from(RawOrigin::Signed(user.clone()));
		let user_lookup = <T::Lookup as StaticLookup>::unlookup(user.clone());

		let caller: T::AccountId = whitelisted_caller();
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		add_registrars::<T>(r)?;

		let info = T::IdentityInformation::create_identity_info(x);
		let info_hash = T::Hashing::hash_of(&info);
		Identity::<T>::set_identity(user_origin.clone(), Box::new(info))?;

		let registrar_origin = T::RegistrarOrigin::try_successful_origin()
			.expect("RegistrarOrigin has no successful origin required for the benchmark");
		Identity::<T>::add_registrar(registrar_origin, caller_lookup)?;
		Identity::<T>::request_judgement(user_origin, caller.clone())?;

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), user_lookup, Judgement::Reasonable, info_hash);

		assert_last_event::<T>(
			Event::<T>::JudgementGiven { target: user, registrar: caller }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn kill_identity(
		r: Linear<1, { T::MaxRegistrars::get() }>,
		x: Linear<0, { T::MaxAdditionalFields::get() }>,
	) -> Result<(), BenchmarkError> {
		add_registrars::<T>(r)?;

		let target: T::AccountId = account("target", 0, SEED);
		let target_origin: <T as frame_system::Config>::RuntimeOrigin =
			RawOrigin::Signed(target.clone()).into();
		let target_lookup = T::Lookup::unlookup(target.clone());

		let info = T::IdentityInformation::create_identity_info(x);
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

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, target_lookup);

		ensure!(!IdentityOf::<T>::contains_key(&target), "Identity not removed");

		Ok(())
	}

	impl_benchmark_test_suite!(Identity, crate::tests::new_test_ext(), crate::tests::Test);
}
