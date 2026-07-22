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
