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
}

impl<AccountId: Encode, Signature: Encode> LiteConsumerRegistrationParams<AccountId, Signature> {
	/// Creates a payload to be signed by the user for a consumer registration request.
	///
	pub fn signing_payload(&self, verifier: &AccountId) -> alloc::vec::Vec<u8> {
		(&self.account, verifier, &self.identifier_key).encode()
	}
}

pub type LiteConsumerRegistrationParamsOf<T> = LiteConsumerRegistrationParams<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AttestationSignature,
>;
