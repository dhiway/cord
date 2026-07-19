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

/// Exact position of a transaction in the Orbis Storage retention ledger.
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
pub struct StorageRef<BlockNumber> {
	pub block: BlockNumber,
	pub transaction_index: u32,
}

/// Actor responsible for creating a retained Orbis Storage position.
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

/// Current Orbis Storage position for content charged to a Resources reservation.
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
	pub storage_ref: StorageRef<BlockNumber>,
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
