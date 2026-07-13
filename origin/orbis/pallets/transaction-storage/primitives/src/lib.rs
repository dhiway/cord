// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Primitives for the transaction storage pallet.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub mod cids;

/// 32-byte hash of a stored blob of data.
pub type ContentHash = [u8; 32];

/// Identifier allocated by the Resources pallet for an isolated storage reservation.
pub type ReservationId = u64;

/// Native storage-provider agreement attached to an isolated reservation.
pub type ProviderAllocationId = [u8; 32];

/// Exact position of a transaction in the Bulletin retention ledger.
///
/// `transaction_index` is the position in the block's `Transactions` vector. It is deliberately
/// not the extrinsic index.
#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct BulletinRef<BlockNumber> {
	pub block: BlockNumber,
	pub transaction_index: u32,
}

/// Actor responsible for creating a retained Bulletin position.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum StorageActor<AccountId> {
	Account(AccountId),
	Root,
	Preimage(ContentHash),
	AutoRenew(AccountId),
}

/// Active, isolated storage capacity owned by one Resources claim.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct ResourceReservation<AccountId, BlockNumber> {
	pub owner: AccountId,
	pub purpose_digest: ContentHash,
	pub bytes_remaining: u64,
	pub transactions_remaining: u32,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
}

/// Current Bulletin position for content charged to a Resources reservation.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct ResourceReservationLink<AccountId, BlockNumber> {
	pub reservation_id: ReservationId,
	pub content_hash: ContentHash,
	pub bulletin_ref: BulletinRef<BlockNumber>,
	pub owner: AccountId,
	pub size: u32,
	pub retention_boundary: BlockNumber,
}

#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum ResourceClosure {
	Cancelled,
	Expired,
	Exhausted,
}

/// Bounded audit row left behind when a reservation closes.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct ResourceReservationTombstone<AccountId, BlockNumber> {
	pub owner: AccountId,
	pub purpose_digest: ContentHash,
	pub final_bytes_remaining: u64,
	pub final_transactions_remaining: u32,
	pub outcome: ResourceClosure,
	pub closed_at: BlockNumber,
}

/// Scalar runtime-API view of either an active reservation or its retained audit row.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum ResourceReservationView<AccountId, BlockNumber> {
	Active(ResourceReservation<AccountId, BlockNumber>),
	Tombstone(ResourceReservationTombstone<AccountId, BlockNumber>),
}

/// Resumable position in the numerically ordered reservation-expiry index.
#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	Default,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct ResourceExpiryCursor<BlockNumber> {
	pub block: Option<BlockNumber>,
	pub offset: u32,
}

/// Identifies a previously-stored entry in the pallet's `Transactions` map.
#[derive(
	Clone,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	codec::DecodeWithMemTracking,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum TransactionRef<BlockNumber> {
	Position { block: BlockNumber, index: u32 },
	ContentHash(ContentHash),
}
