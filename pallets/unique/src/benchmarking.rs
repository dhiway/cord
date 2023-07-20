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
use codec::Encode;
use frame_benchmarking::v1::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{sp_runtime::traits::Hash, BoundedVec};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};

use cord_utilities::traits::GenerateBenchmarkOrigin;

const SEED: u32 = 0;
const MAX_PAYLOAD_BYTE_LENGTH: u32 = 5 * 1024;

benchmarks! {
	where_clause {
		where
		<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::RegistryCreatorId>,
		}
	create {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);
		let did1: T::RegistryCreatorId = account("did1", 0, SEED);

		let stream = [77u8; 32].to_vec();

		let stream_digest = <T as frame_system::Config>::Hashing::hash(&stream[..]);

		let raw_registry = [56u8; 256].to_vec();

		let registry: InputRegistryOf<T> = BoundedVec::try_from(raw_registry).unwrap();

		let id_digest = <T as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &did.encode()[..]].concat()[..],
		);

		let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&stream_digest.encode()[..], &registry_id.encode()[..], &did.encode()[..]]
				.concat()[..],
		);

		let identifier = Ss58Identifier::to_stream_id(&(id_digest).encode()[..]).unwrap();

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry_id.encode()[..], &did1.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

		<Authorizations<T>>::insert(
			&authorization_id,
			RegistryAuthorizationOf::<T> {
				registry_id: registry_id.clone(),
				delegate: did.clone(),
				schema: None,
				permissions: Permissions::all(),
			},
		);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

	}: _<T::RuntimeOrigin>(origin, stream_digest, authorization_id, None)
	verify {
		assert_last_event::<T>(Event::Create { identifier,digest: stream_digest, author: did}.into());
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::new_test_ext(),
	crate::mock::Test
}
