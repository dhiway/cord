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

//! Resources types

extern crate alloc;

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::fmt::Debug;
use scale_info::TypeInfo;

pub type CryptoOf<T> = <<T as Config>::MemberService as MembershipProver>::Crypto;
pub type MemberOf<T> = <CryptoOf<T> as GenerateVerifiable>::Member;
pub type ProofOf<T> = <CryptoOf<T> as GenerateVerifiable>::Proof;
pub type SignatureOf<T> = <CryptoOf<T> as GenerateVerifiable>::Signature;

/// The method through which the user was recognized as a lite person.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub enum RecognitionMethod<Account> {
	/// User has a unique device, corroborated by the attester.
	UniqueDevice(Account),
	// Voucher(PersonalId)
}

/// Information about a registered lite person.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct LitePersonInfo<Member, Method> {
	/// The user's ring vrf key.
	pub ring_vrf_key: Member,
	/// The method through which the user was registered.
	pub method: Method,
}
pub type LitePersonInfoOf<T> =
	LitePersonInfo<MemberOf<T>, RecognitionMethod<<T as frame_system::Config>::AccountId>>;

/// Request parameters to be automatically enrolled as a lite consumer when registering as a lite
/// person.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct LiteConsumerRegistrationParams<AccountId, Signature> {
	/// The signature of the user, constructed as shown in [Self::signing_payload].
	pub signature: Signature,
	/// The account ID of the user.
	pub account: AccountId,
	/// The identifier key of the user.
	pub identifier_key: CommunicationIdentifier,
	/// The user's chosen username.
	pub username: Username,
	/// The user's chosen reserved username, if applicable.
	pub reserved_username: Option<Username>,
}

impl<AccountId: Encode, Signature: Encode> LiteConsumerRegistrationParams<AccountId, Signature> {
	/// Creates a payload to be signed by the user for a consumer registration request.
	///
	/// The signing payload will not contain the `.` separator and the following digits of the
	/// username, as they can be chosen by the attester after the user settles on the primary alpha
	/// part of the username.
	pub fn signing_payload(&self, verifier: &AccountId) -> alloc::vec::Vec<u8> {
		let separator_idx =
			self.username.iter().position(|b| *b == b'.').unwrap_or(self.username.len());
		(
			&self.account,
			verifier,
			&self.identifier_key,
			&self.username[..separator_idx],
			&self.reserved_username,
		)
			.encode()
	}
}

pub type LiteConsumerRegistrationParamsOf<T> = LiteConsumerRegistrationParams<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AttestationSignature,
>;
