// This file is part of CORD – https://cord.network

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

//! Runtime API definition for assets.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct DecodedTokenApi {
	pub origin: bool,
	pub network: u16,
	pub pallet: u16,
	pub genesis: String,
}

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct ViewAuthorization<AccountId, Signature> {
	pub account: AccountId,
	pub payload: Vec<u8>,
	pub signature: Signature,
}

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct TokenHistoryEvent<Hash> {
	pub action: Vec<u8>,
	pub digest: Hash,
	pub height: u32,
	pub index: u32,
}

sp_api::decl_runtime_apis! {
	pub trait TokenApi<AccountId, Signature, Hash>
	where
		AccountId: codec::Codec,
		Signature: codec::Codec,
		Hash: codec::Codec,
	{
		/// Authorised resolution of a token into its structured components.
		fn resolve_identifier(
			auth: ViewAuthorization<AccountId, Signature>,
			token: Vec<u8>,
		) -> Option<DecodedTokenApi>;

		/// Authorised pallet name resolution by index.
		fn resolve_pallet(
			auth: ViewAuthorization<AccountId, Signature>,
			index: u16,
		) -> Option<String>;

		/// Authorised token history query with pagination controls.
		fn token_history(
			auth: ViewAuthorization<AccountId, Signature>,
			token: Vec<u8>,
			start: Option<u32>,
			limit: u32,
		) -> Vec<TokenHistoryEvent<Hash>>;
	}
}
