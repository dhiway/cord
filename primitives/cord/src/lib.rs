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

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

extern crate alloc;
use alloc::vec::Vec;
use codec::{CompactAs, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{RuntimeDebug, TypeId};
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, OpaqueExtrinsic,
};

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Alias to the public key used for this chain, actually a `MultiSigner`. Like the signature, this
/// also isn't a fixed size when encoded, as different cryptos have different size public keys.
pub type AccountPublic = <Signature as Verify>::Signer;

/// Alias to the opaque account ID type for this chain, actually a `AccountId32`. This is always
/// 32 bytes.
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

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

/// Unique identifier of a network.
#[derive(
	Clone,
	CompactAs,
	Copy,
	Decode,
	Default,
	Encode,
	Eq,
	Hash,
	MaxEncodedLen,
	Ord,
	PartialEq,
	PartialOrd,
	RuntimeDebug,
	serde::Serialize,
	serde::Deserialize,
	TypeInfo,
	DecodeWithMemTracking,
)]
#[cfg_attr(feature = "std", derive(derive_more::Display))]
pub struct Id(u32);

impl TypeId for Id {
	const TYPE_ID: [u8; 4] = *b"para";
}

impl From<Id> for u32 {
	fn from(x: Id) -> Self {
		x.0
	}
}

impl From<u32> for Id {
	fn from(x: u32) -> Self {
		Id(x)
	}
}

impl From<usize> for Id {
	fn from(x: usize) -> Self {
		// can't panic, so need to truncate
		let x = x.try_into().unwrap_or(u32::MAX);
		Id(x)
	}
}

// When we added a second From impl for Id, type inference could no longer
// determine which impl should apply for things like `5.into()`. It therefore
// raised a bunch of errors in our test code, scattered throughout the
// various modules' tests, that there is no impl of `From<i32>` (`i32` being
// the default numeric type).
//
// We can't use `cfg(test)` here, because that configuration directive does not
// propagate between crates, which would fail to fix tests in crates other than
// this one.
//
// Instead, let's take advantage of the observation that what really matters for a
// ParaId within a test context is that it is unique and constant. I believe that
// there is no case where someone does `(-1).into()` anyway, but if they do, it
// never matters whether the actual contained ID is `-1` or `4294967295`. Nobody
// does arithmetic on a `ParaId`; doing so would be a bug.
impl From<i32> for Id {
	fn from(x: i32) -> Self {
		Id(x as u32)
	}
}

const PUBLIC_INDEX_START: u32 = 2000;

pub const LOWEST_PUBLIC_ID: Id = Id(PUBLIC_INDEX_START);

impl Id {
	pub const fn new(id: u32) -> Self {
		Self(id)
	}

	pub fn inner(&self) -> u32 {
		self.0
	}
}

impl core::ops::Add<u32> for Id {
	type Output = Self;

	fn add(self, other: u32) -> Self {
		Self(self.0 + other)
	}
}

impl core::ops::Sub<u32> for Id {
	type Output = Self;

	fn sub(self, other: u32) -> Self {
		Self(self.0 - other)
	}
}

pub trait NetworkInfoProvider {
	fn get_network_id() -> Id;
}
