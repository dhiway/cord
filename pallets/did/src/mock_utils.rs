// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use crate::*;
use cord_primitives::Hash;
use did_details::*;
use frame_support::{storage::bounded_btree_set::BoundedBTreeSet, BoundedVec};
use sp_std::{
	collections::btree_set::BTreeSet,
	convert::{TryFrom, TryInto},
	vec,
};

pub(crate) const DEFAULT_URL_SCHEME: [u8; 8] = *b"https://";
const DEFAULT_SERVICE_ENDPOINT_HASH_SEED: u64 = 200u64;

pub fn get_key_agreement_keys<T: Config>(n_keys: u32) -> DidNewKeyAgreementKeySet<T> {
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

// Assumes that the length of the URL is larger than 8 (length of the prefix https://)
pub fn get_service_endpoints<T: Config>(count: u32, length: u32) -> ServiceEndpoints<T> {
	let total_length =
		usize::try_from(length).expect("Failed to convert URL max length value to usize value.");
	let total_count =
		usize::try_from(count).expect("Failed to convert number (count) of URLs to usize value.");
	let mut url_encoded_string = DEFAULT_URL_SCHEME.to_vec();
	url_encoded_string.resize(total_length, b'0');
	let url = Url::<T>::Http(
		HttpUrl::try_from(url_encoded_string.as_ref())
			.expect("Failed to create default URL with provided length."),
	);

	ServiceEndpoints::<T> {
		#[cfg(not(feature = "std"))]
		content_hash: Hash::default(),
		#[cfg(feature = "std")]
		content_hash: Hash::from_low_u64_be(DEFAULT_SERVICE_ENDPOINT_HASH_SEED),
		urls: BoundedVec::<Url<T>, T::MaxEndpointUrlsCount>::try_from(vec![url; total_count])
			.expect("Exceeded max endpoint urls when creating service endpoints"),
		content_type: ContentType::ApplicationJson,
	}
}

pub fn generate_base_did_creation_details<T: Config>(
	did: DidIdentifierOf<T>,
) -> DidCreationDetails<T> {
	DidCreationDetails {
		did,
		new_key_agreement_keys: BoundedBTreeSet::new(),
		new_attestation_key: None,
		new_delegation_key: None,
		new_service_endpoints: None,
	}
}

pub fn generate_base_did_details<T: Config>(
	authentication_key: DidVerificationKey,
) -> DidDetails<T> {
	DidDetails::new(authentication_key, BlockNumberOf::<T>::default())
		.expect("Failed to generate new DidDetails from auth_key due to BoundedBTreeSet bound")
}
