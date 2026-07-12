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

//! Helpers shared between the pallet's unit tests and its mock benchmark setup.
//!
//! Generates real ring-VRF proofs against a fixed mock ring of 255 deterministic
//! members, verified in-process by the mock `MembershipMultiProver`.

use alloc::vec::Vec;
use indiv_support::traits::MembershipProver;
use verifiable::{
	ring::{bandersnatch::BandersnatchVrfVerifiable, RingDomainSize},
	GenerateVerifiable,
};

pub(crate) fn get_ring_config() -> <BandersnatchVrfVerifiable as GenerateVerifiable>::Config {
	// Corresponds to the `RingExponent` value of `R2e14`.
	RingDomainSize::Domain16
}

fn ith_secret_key<T: GenerateVerifiable>(i: u8) -> T::Secret {
	T::new_secret([i; 32])
}

fn ith_pub_key<T: GenerateVerifiable>(i: u8) -> T::Member {
	T::member_from_secret(&ith_secret_key::<T>(i))
}

pub(crate) fn get_mock_ring_members<T: GenerateVerifiable>() -> impl Iterator<Item = T::Member> {
	(0..255u8).map(|i| ith_pub_key::<T>(i))
}

/// Real Ring-VRF proof for benchmarking.
pub(crate) fn prove_vote<T: crate::Config>(
	vote: &crate::VoteData,
	member: u8,
	message: &[u8],
) -> crate::RingProofOf<T>
where
	<<T::MemberService as MembershipProver>::Crypto as GenerateVerifiable>::Proof:
		From<<BandersnatchVrfVerifiable as GenerateVerifiable>::Proof>,
{
	let config = get_ring_config();
	let pub_key = ith_pub_key::<BandersnatchVrfVerifiable>(member);
	let members = get_mock_ring_members::<BandersnatchVrfVerifiable>();
	let commitment = BandersnatchVrfVerifiable::open(config, &pub_key, members)
		.expect("Failed to create commitment");

	let secret = ith_secret_key::<BandersnatchVrfVerifiable>(member);
	let contexts = vote.get_contexts();
	let contexts: Vec<_> = contexts.iter().map(|context| &context[..]).collect();
	let (proof, _) =
		BandersnatchVrfVerifiable::create_multi_context(commitment, &secret, &contexts, message)
			.expect("Failed to create proof");

	proof.into()
}
