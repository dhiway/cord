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

pub(crate) mod v1 {
	use codec::{Decode, Encode};

	use crate::*;

	/// The details associated to a DID identity.
	#[derive(Clone, Decode, Encode, PartialEq)]
	pub struct DidDetails<T: Config> {
		pub(crate) authentication_key: KeyIdOf<T>,
		pub(crate) key_agreement_keys: DidKeyAgreementKeySet<T>,
		pub(crate) delegation_key: Option<KeyIdOf<T>>,
		pub(crate) attestation_key: Option<KeyIdOf<T>>,
		pub(crate) public_keys: DidPublicKeyMap<T>,
		pub(crate) endpoint_url: Option<Url<T>>,
		pub(crate) last_tx_counter: u64,
	}

	#[cfg(test)]
	impl<T: Config> DidDetails<T> {
		pub(crate) fn new(authentication_key: DidVerificationKey, block_number: BlockNumberOf<T>) -> Self {
			let mut public_keys = DidPublicKeyMap::<T>::default();
			let authentication_key_id = utils::calculate_key_id::<T>(&authentication_key.clone().into());
			public_keys
				.try_insert(
					authentication_key_id,
					DidPublicKeyDetails {
						key: authentication_key.into(),
						block_number,
					},
				)
				.expect("Should not exceed BoundedBTreeMap bounds when setting public keys");
			Self {
				authentication_key: authentication_key_id,
				key_agreement_keys: DidKeyAgreementKeySet::<T>::default(),
				attestation_key: None,
				delegation_key: None,
				endpoint_url: None,
				public_keys,
				last_tx_counter: 0u64,
			}
		}
	}

	pub(crate) mod storage {
		use frame_support::{decl_module, decl_storage};
		use sp_std::prelude::*;

		use super::*;

		decl_module! {
			pub struct OldPallet<T: Config> for enum Call where origin: <T as pallet::Config>::Origin {}
		}

		decl_storage! {
			trait Store for OldPallet<T: Config> as Did {
				pub(crate) Did get(fn did): map hasher(blake2_128_concat) DidIdentifierOf<T> => Option<super::DidDetails<T>>;
			}
		}
	}
}
