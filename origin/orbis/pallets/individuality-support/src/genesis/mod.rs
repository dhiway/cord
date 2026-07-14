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

extern crate alloc;
use alloc::{vec, vec::Vec};

use codec::Encode;
use verifiable::ring::{RingDomainSize, RingSuiteExt, StaticChunk};

use crate::traits::RingExponent;

/// Helper function to get ring verifier builder params for a specific domain size.
pub fn ring_verifier_builder_params<S: RingSuiteExt>(
	domain_size: RingDomainSize,
) -> Vec<StaticChunk<S>> {
	let params = verifiable::ring::ring_verifier_builder_params::<S>(domain_size);
	let chunks: Vec<StaticChunk<S>> = params.0.iter().map(|c| StaticChunk(*c)).collect();
	chunks
}

/// Helper function to get raw ring verifier builder params for a specific domain size.
pub fn ring_verifier_builder_params_raw<S: RingSuiteExt>(domain_size: RingDomainSize) -> Vec<u8> {
	ring_verifier_builder_params::<S>(domain_size).encode()
}

/// Helper function to get the hashes of the chunk pages of a specific domain size.
pub fn ring_verifier_builder_params_hashes<S: RingSuiteExt>(
	domain_size: RingDomainSize,
	page_size: u32,
) -> Vec<[u8; 32]> {
	let mut hashes = Vec::new();
	let chunks = ring_verifier_builder_params::<S>(domain_size);
	for paginated_chunks in chunks.chunks(page_size as usize) {
		let hash = paginated_chunks.to_vec().using_encoded(sp_io::hashing::blake2_256);
		hashes.push(hash);
	}
	hashes
}

/// Helper function to get the hashes of the chunk pages of all domain sizes.
/// Returns tuples of (ring_exponent_value, hashes) where ring_exponent_value is the u8 exponent.
pub fn ring_verifier_all_builder_params_hashes<S: RingSuiteExt>(
	page_size: u32,
) -> Vec<(u8, Vec<[u8; 32]>)> {
	vec![
		(
			RingExponent::R2e9.exponent(),
			ring_verifier_builder_params_hashes::<S>(RingDomainSize::Domain11, page_size),
		),
		(
			RingExponent::R2e10.exponent(),
			ring_verifier_builder_params_hashes::<S>(RingDomainSize::Domain12, page_size),
		),
		(
			RingExponent::R2e14.exponent(),
			ring_verifier_builder_params_hashes::<S>(RingDomainSize::Domain16, page_size),
		),
	]
}

pub fn ring_verifier_r2e9_r2e10_builder_params_hashes<S: RingSuiteExt>(
	page_size: u32,
) -> Vec<(u8, Vec<[u8; 32]>)> {
	vec![
		(
			RingExponent::R2e9.exponent(),
			ring_verifier_builder_params_hashes::<S>(RingDomainSize::Domain11, page_size),
		),
		(
			RingExponent::R2e10.exponent(),
			ring_verifier_builder_params_hashes::<S>(RingDomainSize::Domain12, page_size),
		),
	]
}
