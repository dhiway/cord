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

// Low-level types used throughout the CORD code.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::crypto::Ss58AddressFormat;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, OpaqueExtrinsic,
};
use sp_std::vec::Vec;

/// The current node version, which takes the basic SemVer form `<major>.<minor>.<patch>`.
/// In general, minor should be bumped on every release while major or patch releases are
/// relatively rare.
///
/// The associated worker binaries should use the same version as the node that spawns them.
pub const NODE_VERSION: &'static str = "0.9.3";

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it
/// equivalent to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of
/// them.
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

// A timestamp: milliseconds since the unix epoch.
/// `u64` is enough to represent a duration of half a billion years, when the
/// time scale is milliseconds.
pub type Timestamp = u64;

/// Digest item type.
pub type DigestItem = generic::DigestItem;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type.
pub type Block = generic::Block<Header, OpaqueExtrinsic>;

/// Block ID.
pub type BlockId = generic::BlockId<Block>;

/// CID type.
pub type CidOf = Vec<u8>;

/// Version type.
pub type VersionOf = Vec<u8>;

/// Score type.
pub type RatingOf = u32;

/// Score count type.
pub type CountOf = u32;

/// A DID subject identifier.
pub type DidIdentifier = AccountId;

// /// MetaData type.
// pub type MetaDataOf = BoundedVec<u8, ConstU32<15360>>;

/// status Information
pub type StatusOf = bool;

/// node identifier
pub type NodeId = Vec<u8>;

/// Authorship perios
pub const AUTHORSHIP_PERIOD: u32 = 20;

/// Trait definition for network type
pub trait IsPermissioned {
	fn is_permissioned() -> bool;
}

#[derive(Debug, Clone, Copy)]
pub enum Ss58AddressFormatPrefix {
	/// Default for Braid Base
	Base = 3893,
	/// Default for Braid Plus
	Plus = 4926,
	/// Default for Loom
	Loom = 29,
	/// Default for Weave
	Weave = 14058,
	/// Default for unknown chains
	Default = 42,
}

impl From<Ss58AddressFormatPrefix> for Ss58AddressFormat {
	fn from(prefix: Ss58AddressFormatPrefix) -> Self {
		Ss58AddressFormat::custom(prefix as u16)
	}
}
