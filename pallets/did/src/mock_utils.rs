// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
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

use frame_support::storage::bounded_btree_set::BoundedBTreeSet;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::{AccountId32, SaturatedConversion};
use sp_std::{
	collections::btree_set::BTreeSet,
	convert::{TryFrom, TryInto},
	vec,
	vec::Vec,
};

use crate::{
	did_details::{
		DidCreationDetails, DidDetails, DidEncryptionKey, DidNewKeyAgreementKeySet,
		DidVerificationKey,
	},
	service_endpoints::DidEndpoint,
	AccountIdOf, Config, DidCreationDetailsOf, DidIdentifierOf,
};

pub(crate) type DidNewKeyAgreementKeySetOf<T> =
	DidNewKeyAgreementKeySet<<T as Config>::MaxNewKeyAgreementKeys>;

pub fn get_key_agreement_keys<T: Config>(n_keys: u32) -> DidNewKeyAgreementKeySetOf<T> {
	BoundedBTreeSet::try_from(
		(1..=n_keys)
			.map(|i| {
				// Converts the loop index to a 32-byte array;
				let mut seed_vec = i.to_be_bytes().to_vec();
				seed_vec.resize(32, 0u8);
				let seed: [u8; 32] =
					seed_vec.try_into().expect("Failed to create encryption key from raw seed.");
				DidEncryptionKey::X25519(seed)
			})
			.collect::<BTreeSet<DidEncryptionKey>>(),
	)
	.expect("Failed to convert key_agreement_keys to BoundedBTreeSet")
}

pub fn get_service_endpoints<T: Config>(
	count: u32,
	endpoint_id_length: u32,
	endpoint_type_count: u32,
	endpoint_type_length: u32,
	endpoint_url_count: u32,
	endpoint_url_length: u32,
) -> Vec<DidEndpoint<T>> {
	(0..count)
		.map(|i| {
			// Create a string of characters of all 'a', 'b', 'c', and so on depending on
			// the current iteration value, given by `i`.
			let endpoint_id = vec![b'a' + i as u8; endpoint_id_length.saturated_into()];
			let endpoint_types = (0..endpoint_type_count)
				.map(|t| {
					let mut endpoint_type = t.to_be_bytes().to_vec();
					endpoint_type.resize(endpoint_type_length.saturated_into(), 0u8);
					endpoint_type
				})
				.collect();
			let endpoint_urls = (0..endpoint_url_count)
				.map(|u| {
					// Create a string of characters of all 'a', 'b', 'c', and so on depending on
					// the  iteration value, given by `u`.current
					vec![b'a' + u as u8; endpoint_url_length.saturated_into()]
				})
				.collect();
			DidEndpoint::new(endpoint_id, endpoint_types, endpoint_urls)
		})
		.collect()
}

pub fn generate_base_did_creation_details<T: Config>(
	did: DidIdentifierOf<T>,
	submitter: AccountIdOf<T>,
) -> DidCreationDetailsOf<T> {
	DidCreationDetails {
		did,
		submitter,
		new_key_agreement_keys: BoundedBTreeSet::new(),
		new_assertion_key: None,
		new_delegation_key: None,
		new_service_details: Vec::new(),
	}
}

pub fn generate_base_did_details<T>(
	authentication_key: DidVerificationKey<AccountIdOf<T>>,
) -> DidDetails<T>
where
	T: Config,
	<T as frame_system::Config>::AccountId: From<AccountId32>,
{
	DidDetails::new(authentication_key, BlockNumberFor::<T>::default())
		.expect("Failed to generate new DidDetails from auth_key due to BoundedBTreeSet bound")
}
