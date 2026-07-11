// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Runtime API for the Bulletin Chain transaction-storage pallet.
//!
//! Exposes one summary call and two boolean predicates that mirror the
//! validation logic of `store` and `renew`. Clients can use these to preview
//! whether a call will be accepted before signing it.

#![cfg_attr(not(feature = "std"), no_std)]

use bulletin_transaction_storage_primitives::TransactionRef;
use codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;

/// Active-authorization summary for an account. Returned by
/// [`BulletinTransactionStorageApi::account_authorization`] when the account
/// has an unexpired authorization entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, TypeInfo)]
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

sp_api::decl_runtime_apis! {
	/// Runtime API for the Bulletin Chain transaction-storage pallet.
	pub trait BulletinTransactionStorageApi<AccountId, BlockNumber>
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
	}
}
