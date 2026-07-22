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

//! Stable read API for the native Orbis schema and attestation registry.
//!
//! Every collection returned across the runtime boundary has a fixed upper bound and every
//! top-level response carries [`RESPONSE_VERSION`]. Clients choose the block at which the API is
//! invoked; production clients must invoke every related read at the same finalized Orbis block
//! hash. The API deliberately performs no best-head selection and exposes no contract ABI or
//! legacy compatibility facade.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use bounded_collections::{BoundedVec, ConstU32};
use codec::{Codec, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

/// Version encoded into every top-level attestation API response.
pub const RESPONSE_VERSION: u16 = 1;
/// Maximum number of identifiers returned by any one index query.
pub const MAX_PAGE_SIZE: u32 = 100;
/// Maximum schema definition size exposed by API version 1.
pub const MAX_SCHEMA_DEFINITION_BYTES: u32 = 16 * 1024;
/// Maximum authorized issuers exposed by API version 1.
pub const MAX_AUTHORIZED_ISSUERS: u32 = 64;

pub type SchemaDefinition = BoundedVec<u8, ConstU32<MAX_SCHEMA_DEFINITION_BYTES>>;
pub type AuthorizedIssuers<AccountId> = BoundedVec<AccountId, ConstU32<MAX_AUTHORIZED_ISSUERS>>;
pub type IdPageItems<Id> = BoundedVec<Id, ConstU32<MAX_PAGE_SIZE>>;

/// A versioned optional result used by exact identifier lookups.
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

/// Stable schema lifecycle state. Runtime implementations convert pallet states explicitly.
#[derive(
	Clone,
	Copy,
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
pub enum SchemaStatus {
	#[codec(index = 0)]
	Active,
	#[codec(index = 1)]
	Paused,
	#[codec(index = 2)]
	Retired,
}

/// Bounded native index selection fixed when a schema is registered.
#[derive(
	Clone,
	Copy,
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
pub enum IndexPolicy {
	#[codec(index = 0)]
	None,
	#[codec(index = 1)]
	Issuer,
	#[codec(index = 2)]
	SubjectAndSchema,
	#[codec(index = 3)]
	IssuerAndSubjectSchema,
}

/// Complete bounded schema view returned by [`AttestationApi::schema_by_id`].
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct SchemaView<AccountId, BlockNumber, Hash> {
	pub schema: Hash,
	pub creator: AccountId,
	pub definition: SchemaDefinition,
	pub definition_commitment: Hash,
	pub status: SchemaStatus,
	pub revocable: bool,
	pub unique: bool,
	pub index_policy: IndexPolicy,
	pub authorized_issuers: AuthorizedIssuers<AccountId>,
	pub created_at: BlockNumber,
}

/// Metadata-aware client representation of [`SchemaView`].
///
/// The runtime retains its bounded consensus fields. Subxt decodes their sequence-shaped metadata
/// into `Vec`, because `BoundedVec` deliberately does not implement `DecodeAsType`.
#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientSchemaView<AccountId, BlockNumber, Hash> {
	pub schema: Hash,
	pub creator: AccountId,
	pub definition: Vec<u8>,
	pub definition_commitment: Hash,
	pub status: SchemaStatus,
	pub revocable: bool,
	pub unique: bool,
	pub index_policy: IndexPolicy,
	pub authorized_issuers: Vec<AccountId>,
	pub created_at: BlockNumber,
}

/// Complete commitment-only attestation view.
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
pub struct AttestationView<AccountId, BlockNumber, Hash> {
	pub attestation: Hash,
	pub issuer: AccountId,
	pub schema: Hash,
	pub subject_commitment: Hash,
	pub payload_commitment: Hash,
	pub status_commitment: Hash,
	pub parent: Option<Hash>,
	pub expiry: Option<BlockNumber>,
	pub uniqueness_commitment: Option<Hash>,
	pub revocable: bool,
	pub issuance_nonce: u64,
	pub issued_at: BlockNumber,
	pub revoked_at: Option<BlockNumber>,
	pub revoked_by: Option<AccountId>,
}

/// Existence and liveness at the exact block where the API is invoked.
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
pub struct AttestationLiveStatus<BlockNumber> {
	pub version: u16,
	pub exists: bool,
	pub live: bool,
	pub evaluated_at: BlockNumber,
	pub expiry: Option<BlockNumber>,
	pub revoked_at: Option<BlockNumber>,
}

impl<BlockNumber> AttestationLiveStatus<BlockNumber> {
	pub const fn new(
		exists: bool,
		live: bool,
		evaluated_at: BlockNumber,
		expiry: Option<BlockNumber>,
		revoked_at: Option<BlockNumber>,
	) -> Self {
		Self { version: RESPONSE_VERSION, exists, live, evaluated_at, expiry, revoked_at }
	}
}

/// Cursor page over one of the pallet's append-only bounded indexes.
///
/// Cursors are zero-based offsets. Implementations clamp `limit` to [`MAX_PAGE_SIZE`], return an
/// empty page with no next cursor for `limit == 0` or a cursor past the end, and set `next_cursor`
/// only when another identifier exists after a non-empty page.
#[derive(
	Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct IdPage<Id> {
	pub version: u16,
	pub items: IdPageItems<Id>,
	pub next_cursor: Option<u32>,
}

/// Metadata-aware client representation of [`IdPage`].
#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientIdPage<Id> {
	pub version: u16,
	pub items: Vec<Id>,
	pub next_cursor: Option<u32>,
}

impl<Id> IdPage<Id> {
	pub const fn new(items: IdPageItems<Id>, next_cursor: Option<u32>) -> Self {
		Self { version: RESPONSE_VERSION, items, next_cursor }
	}
}

/// Exact next nonce accepted for a delegated intent from one issuer.
#[derive(
	Clone,
	Copy,
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
pub struct DelegatedNonce {
	pub version: u16,
	pub next_nonce: u64,
}

impl DelegatedNonce {
	pub const fn new(next_nonce: u64) -> Self {
		Self { version: RESPONSE_VERSION, next_nonce }
	}
}

/// Versioned monotonic count for an append-only registry.
#[derive(
	Clone,
	Copy,
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
pub struct RegistryCount {
	pub version: u16,
	pub count: u64,
}

impl RegistryCount {
	pub const fn new(count: u64) -> Self {
		Self { version: RESPONSE_VERSION, count }
	}
}

/// Exact next issuer-scoped nonce used to derive an attestation identifier.
#[derive(
	Clone,
	Copy,
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
pub struct IssuanceNonce {
	pub version: u16,
	pub next_nonce: u64,
}

impl IssuanceNonce {
	pub const fn new(next_nonce: u64) -> Self {
		Self { version: RESPONSE_VERSION, next_nonce }
	}
}

/// Immutable issuer-authenticated revocation of an externally maintained status commitment.
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
pub struct ExternalStatusView<AccountId, BlockNumber, Hash> {
	pub key: Hash,
	pub issuer: AccountId,
	pub status_commitment: Hash,
	pub revoked_at: BlockNumber,
}

sp_api::decl_runtime_apis! {
	/// Read API for the native Orbis schema and attestation registry.
	#[api_version(1)]
	pub trait AttestationApi<AccountId, BlockNumber, Hash>
	where
		AccountId: Codec,
		BlockNumber: Codec,
		Hash: Codec,
	{
		/// Return the complete bounded schema stored under `schema`.
		fn schema_by_id(
			schema: Hash,
		) -> Versioned<SchemaView<AccountId, BlockNumber, Hash>>;

		/// Return the complete commitment-only attestation stored under `attestation`.
		fn attestation_by_id(
			attestation: Hash,
		) -> Versioned<AttestationView<AccountId, BlockNumber, Hash>>;

		/// Evaluate exact attestation liveness at the selected invocation block.
		fn attestation_live_status(
			attestation: Hash,
		) -> AttestationLiveStatus<BlockNumber>;

		/// Page through schema identifiers created by `creator`.
		fn creator_schemas(
			creator: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> IdPage<Hash>;

		/// Page through attestation identifiers issued by `issuer`.
		fn issuer_attestations(
			issuer: AccountId,
			cursor: Option<u32>,
			limit: u32,
		) -> IdPage<Hash>;

		/// Page through attestations for one exact subject commitment and schema pair.
		fn subject_schema_attestations(
			subject_commitment: Hash,
			schema: Hash,
			cursor: Option<u32>,
			limit: u32,
		) -> IdPage<Hash>;

		/// Return the exact next delegated nonce accepted from `issuer`.
		fn next_delegated_nonce(issuer: AccountId) -> DelegatedNonce;

		/// Return the total number of schemas registered since genesis.
		fn schema_count() -> RegistryCount;

		/// Return the total number of attestations issued since genesis.
		fn attestation_count() -> RegistryCount;

		/// Return the exact next issuer-scoped attestation identifier nonce.
		fn next_issuance_nonce(issuer: AccountId) -> IssuanceNonce;

		/// Return an issuer's revocation of an externally maintained status commitment.
		fn external_status(
			issuer: AccountId,
			status_commitment: Hash,
		) -> Versioned<ExternalStatusView<AccountId, BlockNumber, Hash>>;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn versioned_results_and_pages_are_scale_stable() {
		let response = Versioned::new(Some(9u32));
		assert_eq!(Versioned::<u32>::decode(&mut response.encode().as_slice()).unwrap(), response);

		let items: IdPageItems<u32> =
			(0..MAX_PAGE_SIZE).collect::<std::vec::Vec<_>>().try_into().unwrap();
		let page = IdPage::new(items, Some(MAX_PAGE_SIZE));
		assert_eq!(IdPage::<u32>::decode(&mut page.encode().as_slice()).unwrap(), page);
	}

	#[test]
	fn page_bound_rejects_more_than_one_hundred_items() {
		let too_many = (0..=MAX_PAGE_SIZE).collect::<std::vec::Vec<_>>();
		assert!(IdPageItems::<u32>::try_from(too_many).is_err());
	}
}
