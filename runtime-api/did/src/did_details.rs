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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_did::{did_details::DidPublicKeyDetails, AccountIdOf, KeyIdOf};
use scale_info::TypeInfo;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};

#[derive(Encode, Decode, TypeInfo, Clone, Debug, Eq, PartialEq, MaxEncodedLen)]
pub struct DidDetails<Key: Ord, BlockNumber: MaxEncodedLen, AccountId> {
	pub authentication_key: Key,
	pub key_agreement_keys: BTreeSet<Key>,
	pub delegation_key: Option<Key>,
	pub assertion_key: Option<Key>,
	pub public_keys: BTreeMap<Key, DidPublicKeyDetails<BlockNumber, AccountId>>,
	pub last_tx_counter: u64,
}

impl<T: pallet_did::Config> From<pallet_did::did_details::DidDetails<T>>
	for DidDetails<KeyIdOf<T>, BlockNumberFor<T>, AccountIdOf<T>>
{
	fn from(did_details: pallet_did::did_details::DidDetails<T>) -> Self {
		Self {
			authentication_key: did_details.authentication_key,
			key_agreement_keys: did_details.key_agreement_keys.into(),
			delegation_key: did_details.delegation_key,
			assertion_key: did_details.assertion_key,
			public_keys: did_details.public_keys.into(),
			last_tx_counter: did_details.last_tx_counter,
		}
	}
}
