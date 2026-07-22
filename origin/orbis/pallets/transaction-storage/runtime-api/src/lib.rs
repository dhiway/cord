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

//! Runtime API for the Orbis Storage transaction-storage pallet.
//!
//! Exposes one summary call and two boolean predicates that mirror the
//! validation logic of `store` and `renew`. Clients can use these to preview
//! whether a call will be accepted before signing it.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
use orbis_transaction_storage_primitives::{
	ContentHash, ProviderAllocationId, ReservationId, ResourceReservationLink,
	ResourceReservationView, StorageActor, StorageRef, TransactionRef,
};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;

/// Active-authorization summary for an account. Returned by
/// [`OrbisTransactionStorageApi::account_authorization`] when the account
/// has an unexpired authorization entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, DecodeAsType, TypeInfo)]
pub struct AccountAuthorization<BlockNumber> {
	/// Block at which this account's authorization expires.
	pub expires_at: BlockNumber,
	/// Total byte cap granted by the authorizer.
	pub bytes_allowance: u64,
	/// Bytes already consumed by `store` calls.
	pub bytes_used: u64,
	/// Bytes already consumed by `renew` calls (counts against the same
	/// `bytes_allowance` cap).
	pub bytes_permanent_used: u64,
	/// Total transaction cap granted by the authorizer. Used together with
	/// `transactions_used` to predict whether a `store` will receive the
	/// priority boost.
	pub transactions_allowance: u32,
	/// Transactions already consumed by `store` and `renew` calls.
	pub transactions_used: u32,
}

/// Metadata-aware client representation of an exact Orbis Storage position.
#[derive(Clone, Copy, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientStorageRef<BlockNumber> {
	pub block: BlockNumber,
	pub transaction_index: u32,
}

/// Metadata-aware client representation of content provenance.
#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub enum ClientStorageActor<AccountId> {
	Account(AccountId),
	Root,
	Preimage([u8; 32]),
	AutoRenew(AccountId),
}

#[derive(Clone, Copy, Debug, DecodeAsType, Eq, PartialEq)]
pub enum ClientResourceClosure {
	Cancelled,
	Expired,
	Exhausted,
}

#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientResourceReservation<AccountId, BlockNumber> {
	pub owner: AccountId,
	pub purpose_digest: [u8; 32],
	pub bytes_remaining: u64,
	pub transactions_remaining: u32,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
}

#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientResourceReservationTombstone<AccountId, BlockNumber> {
	pub owner: AccountId,
	pub purpose_digest: [u8; 32],
	pub final_bytes_remaining: u64,
	pub final_transactions_remaining: u32,
	pub outcome: ClientResourceClosure,
	pub closed_at: BlockNumber,
}

#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub enum ClientResourceReservationView<AccountId, BlockNumber> {
	Active(ClientResourceReservation<AccountId, BlockNumber>),
	Tombstone(ClientResourceReservationTombstone<AccountId, BlockNumber>),
}

#[derive(Clone, Debug, DecodeAsType, Eq, PartialEq)]
pub struct ClientResourceReservationLink<AccountId, BlockNumber> {
	pub reservation_id: u64,
	pub content_hash: [u8; 32],
	pub storage_ref: ClientStorageRef<BlockNumber>,
	pub owner: AccountId,
	pub size: u32,
	pub retention_boundary: BlockNumber,
}

sp_api::decl_runtime_apis! {
	/// Runtime API for the Orbis Storage transaction-storage pallet.
	pub trait OrbisTransactionStorageApi<AccountId, BlockNumber>
	where
		AccountId: Codec,
		BlockNumber: Codec,
	{
		/// Authorization summary for `account`, or `None` if the account has
		/// no unexpired authorization.
		fn account_authorization(account: AccountId) -> Option<AccountAuthorization<BlockNumber>>;

		/// Returns `true` iff a `store(data)` call where `data.len() == data_len`
		/// would currently pass transaction validation for `account`.
		fn can_store(account: AccountId, data_len: u32) -> bool;

		/// Returns `true` iff a `renew(entry)` call would currently pass transaction
		/// validation for `account`.
		fn can_renew(account: AccountId, entry: TransactionRef<BlockNumber>) -> bool;

		/// Explicit actor for an exact retained position, or `None` when the position is absent.
		fn stored_content_provenance(reference: StorageRef<BlockNumber>) -> Option<StorageActor<AccountId>>;

		/// Scalar active-or-tombstone reservation audit view.
		fn resource_reservation(
			reservation_id: ReservationId,
		) -> Option<ResourceReservationView<AccountId, BlockNumber>>;

		/// Current exact link for one reservation/content pair.
		fn resource_reservation_link(
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> Option<ResourceReservationLink<AccountId, BlockNumber>>;

		/// Native provider agreement attached to this reservation, when present.
		fn resource_provider_ref(
			reservation_id: ReservationId,
		) -> Option<ProviderAllocationId>;
	}
}
