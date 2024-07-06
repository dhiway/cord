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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

mod did_details;
mod service_endpoint;

pub use did_details::*;
pub use service_endpoint::*;

#[derive(Encode, Decode, TypeInfo, Eq, PartialEq)]
pub struct DidLinkedInfo<
	DidIdentifier,
	AccountId,
	DidName,
	Id,
	Type,
	Url,
	Key: Ord,
	BlockNumber: MaxEncodedLen,
> {
	pub identifier: DidIdentifier,
	pub account: AccountId,
	pub name: Option<DidName>,
	pub service_endpoints: Vec<ServiceEndpoint<Id, Type, Url>>,
	pub details: DidDetails<Key, BlockNumber, AccountId>,
}

/// The DidLinkedInfo represented as a byte array.
///
/// This will be returned by the runtime and processed by the client side RPC
/// implementation.
pub type RawDidLinkedInfo<DidIdentifier, AccountId, Key, BlockNumber> =
	DidLinkedInfo<DidIdentifier, AccountId, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Key, BlockNumber>;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait DidApi<DidIdentifier, AccountId, Key: Ord, BlockNumber: MaxEncodedLen> where
		DidIdentifier: Codec,
		AccountId: Codec,
		BlockNumber: Codec + MaxEncodedLen,
		Key: Codec,
	{
	/// Given a didname this returns:
	/// * the DID
	/// * public keys stored for the did
	/// * the didName (optional)
	/// * service endpoints
	fn query_by_name(name: Vec<u8>) -> Option<RawDidLinkedInfo<DidIdentifier, AccountId, Key, BlockNumber>>;

	/// Given a did this returns:
	/// * the DID
	/// * public keys stored for the did
	/// * service endpoints
	fn query(did: DidIdentifier) -> Option<RawDidLinkedInfo<DidIdentifier, AccountId, Key, BlockNumber>>;
	}
}
