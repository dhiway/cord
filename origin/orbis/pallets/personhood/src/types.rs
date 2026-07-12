// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
