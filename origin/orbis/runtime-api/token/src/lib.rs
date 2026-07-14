// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq, Clone, Debug)]
pub struct DecodedTokenApi {
	pub origin: bool,
	pub network: u16,
	pub pallet: u16,
	pub genesis: String,
}

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq, Clone, Debug)]
pub enum TokenStatusApi {
	Found { last_state: Option<u32> },
	WrongChain,
	PalletNotFound,
	TokenNotFound,
	InvalidToken,
}

sp_api::decl_runtime_apis! {
	pub trait TokenOriginCommonsRuntimeApi {
		fn decode_token(token: Vec<u8>) -> Option<DecodedTokenApi>;
		fn resolve_pallet(index: u16) -> Option<String>;
		fn token_status(token: Vec<u8>) -> TokenStatusApi;
	}
}
