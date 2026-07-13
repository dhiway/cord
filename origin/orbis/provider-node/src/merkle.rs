// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic append-only Merkle proof helpers.

use codec::Encode;
use serde::Serialize;
use sp_core::H256;

const LEAF_DOMAIN: &[u8] = b"orbis/provider-leaf/v1";
const EMPTY_DOMAIN: &[u8] = b"orbis/provider-empty/v1";

pub(crate) fn hash_leaf<T: Serialize>(value: &T) -> Result<[u8; 32], serde_json::Error> {
	let encoded = serde_json::to_vec(value)?;
	Ok(sp_crypto_hashing::blake2_256(
		&[LEAF_DOMAIN, &(encoded.len() as u64).to_le_bytes(), &encoded].concat(),
	))
}

fn hash_node(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
	sp_crypto_hashing::blake2_256(
		&(b"orbis/provider-node/v2", H256::from(left), H256::from(right)).encode(),
	)
}

pub(crate) fn append_frontier(
	frontier: &mut Vec<Option<[u8; 32]>>,
	leaf_count: u64,
	mut node: [u8; 32],
) -> bool {
	let mut level = 0usize;
	let mut occupied = leaf_count;
	while occupied & 1 == 1 {
		let Some(left) = frontier.get_mut(level).and_then(Option::take) else {
			return false;
		};
		node = hash_node(left, node);
		level += 1;
		occupied >>= 1;
	}
	while frontier.len() <= level {
		frontier.push(None);
	}
	if frontier[level].is_some() {
		return false;
	}
	frontier[level] = Some(node);
	true
}

pub(crate) fn frontier_root(frontier: &[Option<[u8; 32]>], leaf_count: u64) -> Option<[u8; 32]> {
	if leaf_count == 0 {
		return None;
	}
	for (level, peak) in frontier.iter().enumerate() {
		let occupied = leaf_count.checked_shr(level as u32).unwrap_or(0) & 1 == 1;
		if peak.is_some() != occupied {
			return None;
		}
	}
	if leaf_count.checked_shr(frontier.len() as u32).unwrap_or(0) != 0 {
		return None;
	}
	let mut current = None;
	for (level, peak) in frontier.iter().enumerate() {
		let Some(peak) = peak else { continue };
		current = Some(match current {
			None => (*peak, level),
			Some((mut right, mut right_level)) => {
				while right_level < level {
					right = hash_node(right, right);
					right_level += 1;
				}
				(hash_node(*peak, right), level + 1)
			},
		});
	}
	current.map(|(root, _)| root)
}

pub(crate) fn accumulate(leaves: &[[u8; 32]]) -> Option<(Vec<Option<[u8; 32]>>, Vec<[u8; 32]>)> {
	let mut frontier = Vec::new();
	let mut history = Vec::with_capacity(leaves.len());
	for (index, leaf) in leaves.iter().copied().enumerate() {
		if !append_frontier(&mut frontier, index as u64, leaf) {
			return None;
		}
		history.push(frontier_root(&frontier, index as u64 + 1)?);
	}
	Some((frontier, history))
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

pub(crate) fn verify(
	mut leaf: [u8; 32],
	mut index: usize,
	mut leaf_count: usize,
	proof: &[[u8; 32]],
	expected_root: [u8; 32],
) -> bool {
	if leaf_count == 0 || index >= leaf_count {
		return false;
	}
	let mut expected_depth = 0;
	let mut width = leaf_count;
	while width > 1 {
		width = width.saturating_add(1) / 2;
		expected_depth += 1;
	}
	if proof.len() != expected_depth {
		return false;
	}
	for sibling in proof {
		if index % 2 == 0 && index.saturating_add(1) >= leaf_count && *sibling != leaf {
			return false;
		}
		leaf = if index % 2 == 0 { hash_node(leaf, *sibling) } else { hash_node(*sibling, leaf) };
		index /= 2;
		leaf_count = leaf_count.saturating_add(1) / 2;
	}
	let _ = leaf_count;
	leaf == expected_root
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

	#[test]
	fn incremental_frontier_history_matches_duplicate_last_roots() {
		let leaves: Vec<[u8; 32]> = (1..=15).map(|value| [value; 32]).collect();
		let (frontier, history) = accumulate(&leaves).unwrap();
		for count in 1..=leaves.len() {
			assert_eq!(history[count - 1], root(&leaves[..count]));
		}
		assert_eq!(frontier_root(&frontier, leaves.len() as u64), history.last().copied());
	}

	#[test]
	fn odd_even_paths_reconstruct_roots_and_reject_tampering() {
		for leaves in [vec![[1; 32], [2; 32], [3; 32]], vec![[1; 32], [2; 32], [3; 32], [4; 32]]] {
			for index in 0..leaves.len() {
				let path = proof(&leaves, index).unwrap();
				let mut current = leaves[index];
				let mut cursor = index;
				for sibling in &path {
					current = if cursor % 2 == 0 {
						hash_node(current, *sibling)
					} else {
						hash_node(*sibling, current)
					};
					cursor /= 2;
				}
				assert_eq!(current, root(&leaves));
				assert!(verify(leaves[index], index, leaves.len(), &path, root(&leaves)));
				let mut bad = path.clone();
				bad[0][0] ^= 1;
				let mut bad_root = leaves[index];
				let mut cursor = index;
				for sibling in &bad {
					bad_root = if cursor % 2 == 0 {
						hash_node(bad_root, *sibling)
					} else {
						hash_node(*sibling, bad_root)
					};
					cursor /= 2;
				}
				assert_ne!(bad_root, root(&leaves));
				assert!(!verify(leaves[index], index, leaves.len(), &bad, root(&leaves)));
			}
		}
	}

	#[test]
	fn odd_terminal_level_rejects_non_duplicate_sibling_even_for_matching_root() {
		let tombstone = [7; 32];
		let left_root = hash_node([1; 32], [2; 32]);
		let forged = [9; 32];
		let proof = [forged, left_root];
		let forged_root = hash_node(left_root, hash_node(tombstone, forged));
		assert!(!verify(tombstone, 2, 3, &proof, forged_root));
	}
}
