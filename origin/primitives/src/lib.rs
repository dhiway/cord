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

extern crate alloc;

pub mod element;
pub use crate::element::{ElementType, ElementView, Elum};

pub mod attribute;
pub use crate::attribute::{Attribute, AttributeValueView, Attributes, AttributesError, Element};

pub mod packet;
pub use crate::packet::{
	PacketInformationProvider, PacketMetadata, PacketMetadataView, PacketPointer, PacketState,
	PacketStateView, PacketStatus, PacketUpdateError, PacketUpdateOp,
};

pub mod registry;
pub use crate::registry::{
	LookupSpec, RegistryAttributeSpec, RegistryInfoView, RegistryKind, RegistryPermissions,
	RegistryStatus,
};

pub mod authorization;
pub use crate::authorization::{authorization_signature_hash, Authorization};

pub mod identifier;
pub use crate::identifier::Ss58Identifier;

use alloc::vec::Vec;
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
