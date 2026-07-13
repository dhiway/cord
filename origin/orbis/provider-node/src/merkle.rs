// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic append-only Merkle proof helpers.

use serde::Serialize;

const LEAF_DOMAIN: &[u8] = b"orbis/provider-leaf/v1";
const NODE_DOMAIN: &[u8] = b"orbis/provider-node/v1";
const EMPTY_DOMAIN: &[u8] = b"orbis/provider-empty/v1";

pub(crate) fn hash_leaf<T: Serialize>(value: &T) -> Result<[u8; 32], serde_json::Error> {
	let encoded = serde_json::to_vec(value)?;
	Ok(sp_crypto_hashing::blake2_256(
		&[LEAF_DOMAIN, &(encoded.len() as u64).to_le_bytes(), &encoded].concat(),
	))
}

fn hash_node(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
	sp_crypto_hashing::blake2_256(&[NODE_DOMAIN, &left, &right].concat())
}

pub(crate) fn root(leaves: &[[u8; 32]]) -> [u8; 32] {
	if leaves.is_empty() {
		return sp_crypto_hashing::blake2_256(EMPTY_DOMAIN);
	}
	let mut level = leaves.to_vec();
	while level.len() > 1 {
		level = level
			.chunks(2)
			.map(|pair| hash_node(pair[0], *pair.get(1).unwrap_or(&pair[0])))
			.collect();
	}
	level[0]
}

pub(crate) fn proof(leaves: &[[u8; 32]], mut index: usize) -> Option<Vec<[u8; 32]>> {
	if index >= leaves.len() {
		return None;
	}
	let mut level = leaves.to_vec();
	let mut proof = Vec::new();
	while level.len() > 1 {
		let sibling = if index % 2 == 0 {
			*level.get(index + 1).unwrap_or(&level[index])
		} else {
			level[index - 1]
		};
		proof.push(sibling);
		index /= 2;
		level = level
			.chunks(2)
			.map(|pair| hash_node(pair[0], *pair.get(1).unwrap_or(&pair[0])))
			.collect();
	}
	Some(proof)
}

pub(crate) fn peaks(leaves: &[[u8; 32]]) -> Vec<[u8; 32]> {
	let mut result = Vec::new();
	let mut offset = 0;
	let mut remaining = leaves.len();
	while remaining > 0 {
		let size = 1usize << (usize::BITS - 1 - remaining.leading_zeros());
		result.push(root(&leaves[offset..offset + size]));
		offset += size;
		remaining -= size;
	}
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn roots_and_proofs_are_deterministic() {
		let leaves = [[1; 32], [2; 32], [3; 32]];
		assert_eq!(root(&leaves), root(&leaves));
		assert_eq!(proof(&leaves, 1).unwrap().len(), 2);
		assert!(proof(&leaves, 3).is_none());
		assert_eq!(peaks(&leaves).len(), 2);
	}
}
