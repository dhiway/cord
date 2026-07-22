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

//! Fixed-width interface between Commons metadata pallets and its canonical storage control plane.
//!
//! The interface deliberately contains no storage implementation, economic policy or legacy ledger
//! vocabulary. Runtimes provide one adapter and Drive/S3 fail closed when it reports anything other
//! than a publishable manifest and matching provider commitment.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub type Commitment = [u8; 32];

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum CommitmentState {
	Publishable,
	Pending,
	Tombstoned,
	Missing,
}

/// Canonical, deterministic control-state queries needed by metadata pallets.
pub trait CanonicalStorageControl {
	fn manifest_state(manifest: &Commitment) -> CommitmentState;

	fn provider_commitment_matches(manifest: &Commitment, provider_commitment: &Commitment)
		-> bool;

	/// Active Drive roots prevent S3 history pruning of the referenced manifest.
	fn is_drive_referenced(_manifest: &Commitment) -> bool {
		false
	}

	/// True only after the governed deletion-evidence window is satisfied.
	fn deletion_evidence_satisfied(_manifest: &Commitment) -> bool {
		false
	}
}

/// The empty adapter is fail-closed and is useful only for runtimes without storage composition.
impl CanonicalStorageControl for () {
	fn manifest_state(_: &Commitment) -> CommitmentState {
		CommitmentState::Missing
	}

	fn provider_commitment_matches(_: &Commitment, _: &Commitment) -> bool {
		false
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_adapter_is_fail_closed() {
		let value = [7; 32];
		assert_eq!(<()>::manifest_state(&value), CommitmentState::Missing);
		assert!(!<()>::provider_commitment_matches(&value, &value));
		assert!(!<()>::is_drive_referenced(&value));
		assert!(!<()>::deletion_evidence_satisfied(&value));
	}
}
