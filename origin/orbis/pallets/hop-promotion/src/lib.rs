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

//! # HOP Promotion Pallet
//!
//! Promotes near-expiry HOP pool data to permanent chain storage via
//! `pallet-transaction-storage`. Uses general transactions with
//! `#[pallet::authorize]` — no signature, no fees, priority 0, and no
//! debit of the submitter's Bulletin allowance: promotion only lands in
//! blockspace that would otherwise be unused, so charging the user
//! would just leave that space empty for no benefit.
//!
//! The authorize closure verifies the user's submit-time signature and the
//! freshness of the submit timestamp, and refuses promotion for accounts
//! whose Bulletin authorization is missing or expired.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

/// Domain separator for `hop_submit` signatures. Must remain byte-identical
/// to the constant in `sc-hop` (`substrate/client/hop/src/types.rs`).
pub const HOP_SUBMIT_CONTEXT: &[u8] = b"hop-submit-v1:";

/// Reconstructs the signing payload that the user signed at submit time, given
/// the precomputed blake2_256 hash of the data.
///
/// The bytes must remain identical to the SDK-side construction in `sc-hop`,
/// otherwise valid promotions will be rejected on chain.
pub fn signing_payload(data_hash: &[u8; 32], submit_timestamp: u64) -> [u8; 32] {
	const CTX_LEN: usize = HOP_SUBMIT_CONTEXT.len();
	let mut buf = [0u8; CTX_LEN + 32 + 8];
	buf[..CTX_LEN].copy_from_slice(HOP_SUBMIT_CONTEXT);
	buf[CTX_LEN..CTX_LEN + 32].copy_from_slice(data_hash);
	buf[CTX_LEN + 32..].copy_from_slice(&submit_timestamp.to_le_bytes());
	sp_io::hashing::blake2_256(&buf)
}

#[frame_support::pallet]
pub mod pallet {
	use super::signing_payload;
	use crate::WeightInfo;
	use alloc::vec::Vec;
	use bulletin_transaction_storage_primitives::{
		cids::{HashingAlgorithm, RAW_CODEC},
		ContentHash,
	};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pallet_bulletin_transaction_storage::WeightInfo as _;
	use sp_runtime::{
		traits::{IdentifyAccount, Verify},
		AccountId32, MultiSignature, MultiSigner,
	};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config<AccountId = AccountId32>
		+ pallet_bulletin_transaction_storage::Config
		+ pallet_timestamp::Config<Moment = u64>
	{
		/// Maximum allowable skew (in milliseconds) between the user's
		/// submit timestamp and the on-chain time when validating a promotion.
		#[pallet::constant]
		type SubmitTimestampTolerance: Get<u64>;

		/// Weight information for this pallet.
		type WeightInfo: crate::WeightInfo;
	}

	impl<T: Config> Pallet<T> {
		/// Returns whether `who` may have a HOP blob promoted on their behalf.
		///
		/// Satisfied when the account has an unexpired authorization entry in
		/// `pallet-bulletin-transaction-storage`, even if its store/renew
		/// extent has been fully spent. The storage pallet keeps the entry
		/// around (with zero extent) until expiration so that promotion stays
		/// available for the rest of the auth window.
		pub fn can_account_promote(who: &T::AccountId, _data_len: u32) -> bool {
			pallet_bulletin_transaction_storage::Pallet::<T>::account_has_active_authorization(who)
		}

		/// Whether `content_hash` is currently stored on-chain — i.e. some
		/// retained transaction in `pallet-bulletin-transaction-storage`
		/// indexes it.
		///
		/// Used by HOP's maintenance task to confirm a previously submitted
		/// promotion extrinsic landed in a block. Delegates to
		/// `pallet-bulletin-transaction-storage::contains_transaction`,
		/// which answers in O(1) via the content-hash index.
		pub fn is_promoted_on_chain(content_hash: ContentHash) -> bool {
			pallet_bulletin_transaction_storage::Pallet::<T>::contains_transaction(content_hash)
		}

		/// Authorizes a [`Call::promote`] dispatch in the tx pool: validates the
		/// source, data size, block fullness, submit-timestamp freshness, account
		/// authorization, and the user's sr25519 signature over `(data, ts)`.
		// Signature must match the `Call::promote` variant (`Vec<u8>`), so the
		// reference is `&Vec<u8>` rather than `&[u8]`.
		#[allow(clippy::ptr_arg)]
		pub fn authorize_promote(
			source: TransactionSource,
			signer: &MultiSigner,
			signature: &MultiSignature,
			submit_timestamp: &u64,
			data: &Vec<u8>,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			if matches!(source, TransactionSource::External) {
				return Err(InvalidTransaction::Call.into());
			}
			if !pallet_bulletin_transaction_storage::Pallet::<T>::data_size_ok(data.len()) {
				return Err(InvalidTransaction::Custom(0).into());
			}

			// Mirrors the early-out in pallet_bulletin_transaction_storage so we don't pay for
			// chunking + ordered-root hashing when the block is already at MaxBlockTransactions.
			if pallet_bulletin_transaction_storage::Pallet::<T>::block_transactions_full() {
				return Err(InvalidTransaction::ExhaustsResources.into());
			}

			// Reject signatures whose submit_timestamp is too far from the current block time.
			let now_ms = pallet_timestamp::Pallet::<T>::get();
			let skew = now_ms.abs_diff(*submit_timestamp);
			if skew > T::SubmitTimestampTolerance::get() {
				return Err(InvalidTransaction::Stale.into());
			}

			// Account-level authorization check before the expensive signature verify so
			// unauthorized accounts can't force sr25519 verifies on garbage signatures.
			let account_id = signer.clone().into_account();
			if !Self::can_account_promote(&account_id, data.len() as u32) {
				return Err(InvalidTransaction::BadSigner.into());
			}

			// Verify the user's signature over (data, submit_timestamp).
			let data_hash = sp_io::hashing::blake2_256(data);
			let payload = signing_payload(&data_hash, *submit_timestamp);
			if !signature.verify(&payload[..], &account_id) {
				return Err(InvalidTransaction::BadProof.into());
			}

			Ok((
				ValidTransaction::with_tag_prefix("HopPromotion")
					.priority(0)
					.longevity(5)
					.propagate(false)
					.and_provides(data_hash)
					.build()
					.expect("builder always succeeds; qed"),
				Weight::zero(),
			))
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(
			<T as pallet_bulletin_transaction_storage::Config>::WeightInfo::store(data.len() as u32)
		)]
		#[pallet::authorize(Pallet::<T>::authorize_promote)]
		#[pallet::weight_of_authorize(<T as Config>::WeightInfo::authorize_promote(data.len() as u32))]
		// `signer`/`signature`/`submit_timestamp` are validated by `authorize_promote`
		// above; the dispatch body trusts them and only runs after authorization.
		//
		// `data` MUST be the last argument — see the FOOTGUN note on
		// `pallet_bulletin_transaction_storage::Pallet::do_store`: the trailing
		// `data.len()` bytes of the encoded extrinsic get indexed, so any field
		// encoded after `data` corrupts the stored blob.
		pub fn promote(
			origin: OriginFor<T>,
			_signer: MultiSigner,
			_signature: MultiSignature,
			_submit_timestamp: u64,
			data: Vec<u8>,
		) -> DispatchResult {
			ensure_authorized(origin)?;
			pallet_bulletin_transaction_storage::Pallet::<T>::do_store(
				data,
				HashingAlgorithm::Blake2b256,
				RAW_CODEC,
			)
		}
	}
}
