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

//! Types for Proof-of-Personhood system.

#![allow(clippy::result_unit_err)]

use super::*;
use frame_support::pallet_prelude::*;

pub type RevisionIndex = u32;
pub type PageIndex = u32;
pub type KeyCount = u64;

pub type CryptoOf<T> = <<T as Config>::MemberService as MembershipProver>::Crypto;
pub type MemberOf<T> = <CryptoOf<T> as GenerateVerifiable>::Member;
pub type ProofOf<T> = <CryptoOf<T> as GenerateVerifiable>::Proof;
pub type MembersOf<T> = <CryptoOf<T> as GenerateVerifiable>::Members;
pub type IntermediateOf<T> = <CryptoOf<T> as GenerateVerifiable>::Intermediate;
pub type SecretOf<T> = <CryptoOf<T> as GenerateVerifiable>::Secret;
pub type SignatureOf<T> = <CryptoOf<T> as GenerateVerifiable>::Signature;

/// Record of personhood.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct PersonRecord<Member, AccountId> {
	// The key used for the person.
	pub key: Member,
	/// An optional privileged account that can send transaction on the behalf of the person.
	pub account: Option<AccountId>,
}
