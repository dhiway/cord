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

//! Fixed-width, versioned identity and personhood views for native Orbis clients.
//!
//! The API intentionally returns status only. Identity field values, ring keys, aliases and
//! proofs remain private to their pallets and are never copied into an unbounded runtime response.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode, MaxEncodedLen};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

pub const RESPONSE_VERSION: u16 = 1;

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct Versioned<T> {
	pub version: u16,
	pub value: T,
}

impl<T> Versioned<T> {
	pub const fn new(value: T) -> Self {
		Self { version: RESPONSE_VERSION, value }
	}
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct IdentityStatus {
	pub registered: bool,
	pub judgement_count: u32,
	pub requested: u32,
	pub reasonable: u32,
	pub known_good: u32,
	pub out_of_date: u32,
	pub low_quality: u32,
	pub erroneous: u32,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct PersonhoodStatus {
	pub full_personal_id: Option<u64>,
	pub full_recognized: bool,
	pub lite_recognized: bool,
}

#[derive(Clone, Debug, Decode, DecodeAsType, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct AttestationAllowance {
	pub remaining: u32,
}

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait IdentityPersonhoodApi<AccountId>
	where
		AccountId: Codec,
	{
		fn identity_status(account: AccountId) -> Versioned<IdentityStatus>;
		fn personhood_status(account: AccountId) -> Versioned<PersonhoodStatus>;
		fn attestation_allowance(account: AccountId) -> Versioned<AttestationAllowance>;
	}
}
