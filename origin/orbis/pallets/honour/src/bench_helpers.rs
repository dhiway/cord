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
