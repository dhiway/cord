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

//! Stable runtime API for the native Orbis Names registry.
//!
//! Byte-oriented inputs and outputs are bounded at the API boundary. Clients select the block at
//! which to invoke this API; production clients should use a finalized block hash. The runtime API
//! deliberately contains no best-head selection, contract ABI, or legacy compatibility facade.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use bounded_collections::{BoundedVec, ConstU32};
use codec::{Codec, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

/// Version encoded into every top-level Orbis Names response.
pub const RESPONSE_VERSION: u16 = 1;
/// Consensus label-policy version expected by normalized-label lookups.
pub const LABEL_POLICY_VERSION: u16 = 1;
/// Maximum number of names returned by one owner query.
pub const MAX_OWNER_NAMES_PAGE_SIZE: u32 = 100;
/// Maximum number of controllers returned for one name.
pub const MAX_CONTROLLERS: u32 = 32;

pub const MAX_LABEL_LENGTH: u32 = 63;
pub const MAX_ADDRESS_LENGTH: u32 = 128;
pub const MAX_TEXT_KEY_LENGTH: u32 = 32;
pub const MAX_TEXT_VALUE_LENGTH: u32 = 256;

/// A label accepted by label policy v1: lowercase ASCII letters, digits, and internal hyphens.
pub type NormalizedLabel = BoundedVec<u8, ConstU32<MAX_LABEL_LENGTH>>;
/// An opaque address record. Interpretation belongs to the consuming application or SDK.
pub type Address = BoundedVec<u8, ConstU32<MAX_ADDRESS_LENGTH>>;
/// A bounded text-record key.
pub type TextKey = BoundedVec<u8, ConstU32<MAX_TEXT_KEY_LENGTH>>;
/// A bounded text-record value.
pub type TextValue = BoundedVec<u8, ConstU32<MAX_TEXT_VALUE_LENGTH>>;
/// A bounded page of canonical name identifiers.
pub type OwnerNameItems<NameId> = BoundedVec<NameId, ConstU32<MAX_OWNER_NAMES_PAGE_SIZE>>;
/// Bounded controller view for a canonical name.
pub type ControllerItems<AccountId> = BoundedVec<AccountId, ConstU32<MAX_CONTROLLERS>>;

/// A versioned optional result.
///
/// `None` is also used by resolver calls when a name is missing or expired. Call
/// [`NamesApi::name_status`] when a client needs to distinguish those states.
#[derive(
	Clone,
	Debug,
	Decode,
	DecodeAsType,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub struct Versioned<T> {
	pub version: u16,
	pub value: Option<T>,
}

impl<T> Versioned<T> {
	pub const fn new(value: Option<T>) -> Self {
		Self { version: RESPONSE_VERSION, value }
	}
}

/// Stable metadata view returned by the name-by-ID query.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct NameView<AccountId, BlockNumber, NameId> {
	pub name: NameId,
	pub parent: Option<NameId>,
	pub label: NormalizedLabel,
	pub owner: AccountId,
	pub expires_at: BlockNumber,
	pub depth: u32,
}

/// Metadata-aware client representation of [`NameView`].
///
/// The runtime retains its bounded label. Subxt decodes the same sequence-shaped metadata field
/// into `Vec`, because `BoundedVec` deliberately does not implement `DecodeAsType`.
#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientNameView<AccountId, BlockNumber, NameId> {
	pub name: NameId,
	pub parent: Option<NameId>,
	pub label: Vec<u8>,
	pub owner: AccountId,
	pub expires_at: BlockNumber,
	pub depth: u32,
}

/// Cursor-based page over the pallet's bounded owner index.
///
/// Cursors are zero-based offsets. Runtime implementations clamp `limit` to
/// [`MAX_OWNER_NAMES_PAGE_SIZE`] and return `next_cursor` only when another item exists.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct OwnerNamesPage<NameId> {
	pub version: u16,
	pub names: OwnerNameItems<NameId>,
	pub next_cursor: Option<u32>,
}

/// Metadata-aware client representation of [`OwnerNamesPage`].
#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientOwnerNamesPage<NameId> {
	pub version: u16,
	pub names: Vec<NameId>,
	pub next_cursor: Option<u32>,
}

impl<NameId> OwnerNamesPage<NameId> {
	pub const fn new(names: OwnerNameItems<NameId>, next_cursor: Option<u32>) -> Self {
		Self { version: RESPONSE_VERSION, names, next_cursor }
	}
}

/// Existence and expiry information at the block where the API is invoked.
#[derive(
	Clone,
	Debug,
	Decode,
	DecodeAsType,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub struct NameStatus<BlockNumber> {
	pub version: u16,
	pub exists: bool,
	pub active: bool,
	pub expires_at: Option<BlockNumber>,
}

#[derive(
	Clone,
	Debug,
	Decode,
	DecodeAsType,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub struct ContentPublication<ContentCommitment> {
	pub content: ContentCommitment,
	pub revision: u64,
}

sp_api::decl_runtime_apis! {
	/// Read API for the native Orbis Names pallet.
	#[api_version(1)]
	pub trait NamesApi<AccountId, BlockNumber, NameId, SubjectId, AttestationId, ContentCommitment>
	where
		AccountId: Codec,
		BlockNumber: Codec,
		NameId: Codec,
		SubjectId: Codec,
		AttestationId: Codec,
		ContentCommitment: Codec,
	{
		/// Return the consensus label-policy version used by registration and lookup.
		fn label_policy_version() -> u16;

		/// Return stable metadata for a stored name, including expired names not yet cleaned up.
		fn name_by_id(
			name: NameId,
		) -> Versioned<NameView<AccountId, BlockNumber, NameId>>;

		/// Resolve an already-normalized root label under label policy v1.
		///
		/// Invalid labels fail closed with `value = None`; the implementation derives the root name ID
		/// using the pallet's genesis-bound derivation and verifies that the name exists.
		fn root_name_by_normalized_label(label: NormalizedLabel) -> Versioned<NameId>;

		/// Page through canonical name IDs owned by `owner`.
		///
		/// `cursor` is a zero-based offset and `limit` is clamped to 100.
		fn owner_names(
			owner: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> OwnerNamesPage<NameId>;

		/// Return the bounded controller accounts for an active or stored name.
		fn controllers(name: NameId) -> Versioned<ControllerItems<AccountId>>;

		/// Resolve the opaque address of an active name.
		fn resolve_address(name: NameId) -> Versioned<Address>;

		/// Resolve the canonical identity/personhood subject of an active name.
		fn resolve_subject(name: NameId) -> Versioned<SubjectId>;

		/// Resolve the canonical attestation identifier of an active name, returning `None` when
		/// the linked attestation is missing, revoked, expired, or belongs to an inactive schema.
		fn resolve_attestation(name: NameId) -> Versioned<AttestationId>;

		/// Resolve the canonical storage content commitment of an active name.
		fn resolve_content(name: NameId) -> Versioned<ContentCommitment>;

		/// Resolve content together with its consensus CAS revision.
		fn resolve_content_publication(name: NameId) -> Versioned<ContentPublication<ContentCommitment>>;

		/// Resolve one bounded text record of an active name.
		fn resolve_text(name: NameId, key: TextKey) -> Versioned<TextValue>;

		/// Return the active primary name currently owned by `owner`.
		fn primary_name(owner: AccountId) -> Versioned<NameId>;

		/// Return existence, activity, and expiry at the selected invocation block.
		fn name_status(name: NameId) -> NameStatus<BlockNumber>;
	}
}
