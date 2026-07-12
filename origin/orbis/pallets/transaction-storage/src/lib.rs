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

//! Transaction storage pallet. Indexes transactions and manages storage proofs.
//!
//! This pallet is designed to be used on chains with no transaction fees. It must be used with a
//! `TransactionExtension` implementation that calls the
//! [`validate_signed`](Pallet::validate_signed) and
//! [`pre_dispatch_signed`](Pallet::pre_dispatch_signed) functions.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod weights;

pub mod migrations;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;

use alloc::vec::Vec;
use bulletin_transaction_storage_primitives::{
	cids::{calculate_cid, Cid, CidCodec, CidConfig, HashingAlgorithm, RAW_CODEC},
	BulletinRef, ContentHash, ReservationId, ResourceClosure, ResourceExpiryCursor,
	ResourceReservation, ResourceReservationLink, ResourceReservationTombstone,
	ResourceReservationView, StorageActor,
};
use codec::{Decode, Encode, MaxEncodedLen};
use core::fmt::Debug;
use indiv_support::traits::{ResourceClaimLifecycle, TwoPhaseStorage};
use pallet_bulletin_transaction_storage_runtime_api::AccountAuthorization;
use polkadot_sdk_frame::{
	deps::*,
	prelude::*,
	traits::{
		fungible::{hold::Balanced, Mutate, MutateHold},
		parameter_types, OriginTrait,
	},
};
use sp_transaction_storage_proof::{
	encode_index, num_chunks, random_chunk, ChunkIndex, InherentError, TransactionStorageProof,
	CHUNK_SIZE, INHERENT_IDENTIFIER,
};

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

const LOG_TARGET: &str = "runtime::transaction-storage";

/// Default retention period for data (in blocks). 14 days at 6s block time.
pub const DEFAULT_RETENTION_PERIOD: u32 = 2 * 100800;
parameter_types! {
	pub const DefaultRetentionPeriod: u32 = DEFAULT_RETENTION_PERIOD;
}

/// Maximum bytes that can be stored in one transaction.
/// Setting a higher limit may exceed the WASM allocator's 128 MiB heap and cause OOM errors.
///
/// Note: 2 MiB is aligned with the Bitswap maximum block size.
pub const DEFAULT_MAX_TRANSACTION_SIZE: u32 = 2 * 1024 * 1024;
/// Default maximum number of indexed transactions in a block.
pub const DEFAULT_MAX_BLOCK_TRANSACTIONS: u32 = 512;

/// Encountered an impossible situation, implies a bug.
pub const IMPOSSIBLE: InvalidTransaction = InvalidTransaction::Custom(0);
/// Data size is not in the allowed range.
pub const BAD_DATA_SIZE: InvalidTransaction = InvalidTransaction::Custom(1);
/// Renewed extrinsic not found.
pub const RENEWED_NOT_FOUND: InvalidTransaction = InvalidTransaction::Custom(2);
/// Authorization was not found.
pub const AUTHORIZATION_NOT_FOUND: InvalidTransaction = InvalidTransaction::Custom(3);
/// Authorization has not expired.
pub const AUTHORIZATION_NOT_EXPIRED: InvalidTransaction = InvalidTransaction::Custom(4);
/// Renew rejected: would push the signer's `bytes_permanent` past their `bytes_allowance`
/// (per-account hard cap).
pub const PERMANENT_ALLOWANCE_EXCEEDED: InvalidTransaction = InvalidTransaction::Custom(5);
/// Renew rejected: would push `PermanentStorageUsed` past `MaxPermanentStorageSize`
/// (chain-wide hard cap).
pub const CHAIN_PERMANENT_CAP_REACHED: InvalidTransaction = InvalidTransaction::Custom(6);
/// Authorizer account was not found.
pub const AUTHORIZER_NOT_FOUND: InvalidTransaction = InvalidTransaction::Custom(7);
/// Authorizer budget has not been exhausted.
pub const AUTHORIZATION_NOT_EXHAUSTED: InvalidTransaction = InvalidTransaction::Custom(8);
/// `disable_auto_renew`: no auto-renewal is registered for the given content hash.
pub const AUTO_RENEWAL_NOT_ENABLED: InvalidTransaction = InvalidTransaction::Custom(9);
/// `disable_auto_renew`: caller is not the account that registered the auto-renewal.
pub const NOT_AUTO_RENEWAL_OWNER: InvalidTransaction = InvalidTransaction::Custom(10);
/// `enable_auto_renew`: an auto-renewal is already registered for this content hash.
pub const AUTO_RENEWAL_ALREADY_ENABLED: InvalidTransaction = InvalidTransaction::Custom(11);
/// `disable_auto_renew`: the registration has been prepaid for its next cycle and
/// cannot be disabled by the owner until the cycle fires and consumes the prepayment.
/// Root can still disable for governance cleanup.
pub const CANNOT_DISABLE_PREPAID_AUTO_RENEWAL: InvalidTransaction = InvalidTransaction::Custom(12);

/// Percent of `MaxPermanentStorageSize` at which the pallet emits
/// [`Event::PermanentStorageNearCap`] (rising-edge only). Off-chain governance consumers
/// can use this as a "raise the cap or coordinate another bulletin chain" trigger.
pub const PERMANENT_STORAGE_NEAR_CAP_PERCENT: u64 = 80;

struct PreparedReservedStore<AccountId, BlockNumber> {
	owner: AccountId,
	reservation: ResourceReservation<AccountId, BlockNumber>,
	content_hash: ContentHash,
	size: u32,
	bulletin_ref: BulletinRef<BlockNumber>,
}

struct PreparedReservedRenew<AccountId, BlockNumber> {
	owner: AccountId,
	reservation: ResourceReservation<AccountId, BlockNumber>,
	source: ResourceReservationLink<AccountId, BlockNumber>,
	bulletin_ref: BulletinRef<BlockNumber>,
}

struct PreparedRenew<AccountId, BlockNumber> {
	info: TransactionInfo,
	content_hash: ContentHash,
	extrinsic_index: u32,
	bulletin_ref: BulletinRef<BlockNumber>,
	actor: StorageActor<AccountId>,
}

#[cfg(test)]
std::thread_local! {
	static RENEW_HOST_OBSERVER: core::cell::RefCell<Option<alloc::boxed::Box<dyn FnMut()>>> =
		const { core::cell::RefCell::new(None) };
}

pub use extension::{CallInspector, MAX_WRAPPER_DEPTH};

#[polkadot_sdk_frame::pallet]
pub mod pallet {
	use super::*;

	/// A reason for this pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// The funds are held as deposit for the used storage.
		StorageFeeHold,
	}

	#[pallet::config]
	pub trait Config:
		frame_system::Config<
		RuntimeOrigin: OriginTrait<PalletsOrigin: From<Origin<Self>> + TryInto<Origin<Self>>>,
	>
	{
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A dispatchable call.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;
		/// The fungible type for this pallet.
		type Currency: Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ Balanced<Self::AccountId>;
		/// The overarching runtime hold reason.
		type RuntimeHoldReason: From<HoldReason>;
		/// Handler for the unbalanced decrease when fees are burned.
		type FeeDestination: OnUnbalanced<CreditOf<Self>>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
		/// Maximum number of indexed transactions in the block.
		#[pallet::constant]
		type MaxBlockTransactions: Get<u32>;
		/// Maximum data set in a single transaction in bytes.
		#[pallet::constant]
		type MaxTransactionSize: Get<u32>;
		/// Cap, in bytes, on total permanent storage (via `renew`) committed across
		/// all authorizations. Tracks chain-wide capacity for permanent data.
		#[pallet::constant]
		type MaxPermanentStorageSize: Get<u64>;
		/// Maximum total number of active reservations and retained tombstones.
		#[pallet::constant]
		type MaxReservations: Get<u32>;
		/// Maximum number of distinct blocks in the expiry index.
		#[pallet::constant]
		type MaxReservationExpiryBlocks: Get<u32>;
		/// Maximum reservations sharing one expiry block.
		#[pallet::constant]
		type MaxReservationsPerExpiryBlock: Get<u32>;
		/// Maximum live reservation/content links.
		#[pallet::constant]
		type MaxReservationLinks: Get<u32>;
		/// Number of blocks a closed reservation remains queryable.
		#[pallet::constant]
		type TombstoneRetention: Get<BlockNumberFor<Self>>;
		/// Bounded purpose supplied by the Resources claim allocator.
		type ReservationPurpose: Parameter + MaxEncodedLen;
		/// Synchronous Resources-side claim cleanup invoked before tombstone removal.
		type ResourceClaimLifecycle: ResourceClaimLifecycle<ReservationId, Self::ReservationPurpose>;
		/// Authorizations expire after this many blocks.
		#[pallet::constant]
		type AuthorizationPeriod: Get<BlockNumberFor<Self>>;
		/// The origin that manages the authorizer list.
		type AuthorizerRegistrarOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// The origin that can authorize data storage. `Success` is
		/// `Some(AuthorizationOrigin { .. })` when the dispatcher is an
		/// [`AllowedAuthorizers`] entry — carrying the budget owner and the
		/// authorizer's `valid_until` (used to clamp the granted authorization's
		/// expiry, so a grant cannot outlive its grantor). `None` for Root / XCM /
		/// other non-account authorizers, which have no budget and no clamping.
		type Authorizer: EnsureOrigin<
			Self::RuntimeOrigin,
			Success = Option<AuthorizationOrigin<Self::AccountId, BlockNumberFor<Self>>>,
		>;
		/// Priority of store/renew transactions.
		#[pallet::constant]
		type StoreRenewPriority: Get<TransactionPriority>;
		/// Longevity of store/renew transactions.
		#[pallet::constant]
		type StoreRenewLongevity: Get<TransactionLongevity>;
		/// Priority of unsigned transactions to remove expired authorizations.
		#[pallet::constant]
		type RemoveExpiredAuthorizationPriority: Get<TransactionPriority>;
		/// Longevity of unsigned transactions to remove expired authorizations.
		#[pallet::constant]
		type RemoveExpiredAuthorizationLongevity: Get<TransactionLongevity>;
		/// Benchmark helper — provides pre-computed proof matching this runtime's config.
		/// Use [`DefaultCheckProofHelper`](crate::benchmarking::DefaultCheckProofHelper) for
		/// [`DEFAULT_MAX_TRANSACTION_SIZE`] / [`DEFAULT_MAX_BLOCK_TRANSACTIONS`].
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Attempted to call `store`/`renew` outside of block execution.
		BadContext,
		/// Data size is not in the allowed range.
		BadDataSize,
		/// Too many transactions in the block.
		TooManyTransactions,
		/// Invalid configuration.
		NotConfigured,
		/// Renewed extrinsic is not found.
		RenewedNotFound,
		/// Proof was not expected in this block.
		UnexpectedProof,
		/// Proof failed verification.
		InvalidProof,
		/// Missing storage proof.
		MissingProof,
		/// Unable to verify proof because state data is missing.
		MissingStateData,
		/// Double proof check in the block.
		DoubleCheck,
		/// Storage proof was not checked in the block.
		ProofNotChecked,
		/// Authorization was not found.
		AuthorizationNotFound,
		/// Authorization has not expired.
		AuthorizationNotExpired,
		/// Renew rejected: would push the signer's `bytes_permanent` past their
		/// `bytes_allowance` (per-account hard cap).
		PermanentAllowanceExceeded,
		/// Renew rejected: would push `PermanentStorageUsed` past
		/// `MaxPermanentStorageSize` (chain-wide hard cap).
		ChainPermanentCapReached,
		/// Content hash was not calculated.
		InvalidContentHash,
		/// Authorizer account was not found.
		AuthorizerNotFound,
		/// Authorizer is not eligible for permissionless removal — it still has budget on both
		/// axes AND (if `valid_until` is set) has not yet expired.
		AuthorizerBudgetNotExhausted,
		/// Auto-renewal is already enabled for this content hash.
		AutoRenewalAlreadyEnabled,
		/// Auto-renewal is not enabled for this content hash.
		AutoRenewalNotEnabled,
		/// Caller is not the owner of the auto-renewal registration.
		NotAutoRenewalOwner,
		/// `disable_auto_renew` rejected: the registration has been prepaid for its next
		/// cycle. The owner must wait until that cycle consumes the prepayment before
		/// disabling. Root can disable regardless.
		CannotDisablePrepaidAutoRenewal,
		/// `valid_until` supplied to `add_authorizer` is in the past (`<= now`, would
		/// expire immediately). Pass `None` for no expiration.
		InvalidValidUntil,
		/// `authorize_account` / `authorize_preimage` called by a signer whose
		/// `AllowedAuthorizers` budget cannot cover the requested
		/// `transactions` / `bytes` (or `max_size`).
		InsufficientAuthorizerBudget,
		ReservationNotFound,
		NotReservationOwner,
		ReservationNotActive,
		ReservationExpired,
		ContentNotFound,
		ContentAlreadyLinked,
		ContentTooLarge,
		TransactionAllowanceExhausted,
		BytesAllowanceExhausted,
		StoredContentOwnerMismatch,
		LegacyContentUnrenewable,
		BulletinRefHashMismatch,
		ExpiryBucketFull,
		ExpiryBlockSetFull,
		CleanupLimitExceeded,
		ReservationCapacityExceeded,
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(7);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Data associated with a renewal registration in [`AutoRenewals`].
	///
	/// Holds the owner account, a `recurring` flag that decides whether the
	/// registration is consumed after a single successful renewal (`false`, set by
	/// [`Pallet::renew`]) or persists forever (`true`, set by
	/// [`Pallet::enable_auto_renew`]), and a `paid` flag indicating that the next
	/// cycle has already been charged against the owner's authorization at
	/// registration time.
	///
	/// Both [`Pallet::renew`] and [`Pallet::enable_auto_renew`] insert with
	/// `paid: true`: the extension's `check_signed` charges `bytes_permanent`,
	/// `PermanentStorageUsed`, and one tx slot up front (same as `force_renew`).
	/// [`Pallet::do_process_auto_renewals`] keys its charge-skip off `paid`: when
	/// `paid` is true the cycle renews without re-charging and then flips `paid`
	/// to false (for recurring entries) so subsequent cycles pay per-cycle, as
	/// before. One-shot entries (`recurring: false`) are removed after the single
	/// renewal so the flag is inert after that point.
	///
	/// While `paid` is true, [`Pallet::disable_auto_renew`] rejects signed callers
	/// — the owner must wait for the first cycle to consume the prepayment. This
	/// is what makes `enable_auto_renew` honestly cost a renewal even if the
	/// owner immediately disables (bytes already left the per-account quota at
	/// registration). Root can still disable for governance cleanup.
	#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
	pub struct RenewalData<AccountId> {
		/// Account whose authorization will be consumed each time data is auto-renewed.
		pub account: AccountId,
		/// `true` — auto-renew forever (set by `enable_auto_renew`).
		/// `false` — one-shot: removed from `AutoRenewals` after the first successful
		/// renewal cycle (set by `renew`).
		pub recurring: bool,
		/// `true` — the next renewal cycle has already been charged at registration
		/// time and will fire free. After the cycle delivers, the flag is flipped to
		/// `false` for recurring entries; for one-shot entries the registration is
		/// removed outright.
		pub paid: bool,
	}

	/// Custom origin for authorized signed transaction storage operations.
	///
	/// This origin is set by the [`extension::ValidateStorageCalls`] transaction extension
	/// for signed transactions that pass authorization checks. Unsigned transactions
	/// do not use this origin (they are validated via [`ValidateUnsigned`]).
	#[pallet::origin]
	#[derive(
		Clone,
		PartialEq,
		Eq,
		Debug,
		codec::Encode,
		codec::Decode,
		codec::DecodeWithMemTracking,
		scale_info::TypeInfo,
		codec::MaxEncodedLen,
	)]
	pub enum Origin<T: Config> {
		/// A signed transaction that has been authorized to store data.
		/// Contains the signer and the scope of authorization that was consumed.
		Authorized { who: T::AccountId, scope: AuthorizationScopeFor<T> },
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Mandatory per-block hook: ages out the obsolete `Transactions[obsolete]` entry,
		/// decrements [`PermanentStorageUsed`] for any `kind == Renew` items in it, cleans
		/// up `TransactionByContentHash`, and queues auto-renewals for `process_auto_renewals`.
		///
		/// Weight is charged via the [`WeightInfo::on_initialize_with_expiry`] benchmark.
		/// The fit within `max_block` is asserted by [`ensure_weight_sanity`] — every
		/// runtime should exercise it from a test.
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let mut weight = Weight::zero();

			// Run v0→v1 migration if it hasn't been applied yet.
			// This handles the case where `codeSubstitutes` loaded the fix runtime
			// without triggering `on_runtime_upgrade` (spec_version unchanged).
			// Safe alongside the regular `MigrateV0ToV1` wired in Executive: both
			// check `on_chain_storage_version() < 1`, so whichever runs first bumps
			// the version and the other becomes a no-op.
			// TODO: Remove once all chains have been migrated past v1 — after that
			// this is just a redundant storage read per block.
			weight.saturating_accrue(migrations::v1::maybe_migrate_v0_to_v1::<T>());

			// Drop obsolete roots and decrement the chain-wide permanent counter for any
			// renewed bytes that just aged out. The proof for `obsolete` will be checked
			// later in this block, so we drop `obsolete` - 1.
			let period = Self::retention_period();
			let obsolete = n.saturating_sub(period.saturating_add(One::one()));
			let mut num_expiring: u32 = 0;
			if obsolete > Zero::zero() {
				if let Some(transactions) = <Transactions<T>>::take(obsolete) {
					num_expiring = transactions.len() as u32;

					// Single pass: sum renewed sizes, clean up `TransactionByContentHash`,
					// and schedule auto-renewals.
					let mut pending = PendingAutoRenewals::<T>::get();
					let mut renewed_sum: u64 = 0;
					for (transaction_index, tx_info) in transactions.into_iter().enumerate() {
						let hash: ContentHash = tx_info.content_hash;
						let bulletin_ref = BulletinRef {
							block: obsolete,
							transaction_index: transaction_index as u32,
						};
						StoredBy::<T>::remove(bulletin_ref);
						if let Some((reservation_id, linked_hash)) =
							ResourceLinkByRef::<T>::take(bulletin_ref)
						{
							ResourceReservationLinks::<T>::remove(reservation_id, linked_hash);
							if ResourceLinkByContentHash::<T>::get(linked_hash)
								== Some(reservation_id)
							{
								ResourceLinkByContentHash::<T>::remove(linked_hash);
							}
							ResourceReservationLinkCount::<T>::mutate(|count| {
								*count = count.saturating_sub(1)
							});
						}

						// Sum renewed sizes for the chain-wide permanent counter decrement.
						if matches!(tx_info.kind, TransactionKind::Renew) {
							renewed_sum = renewed_sum.saturating_add(tx_info.size as u64);
						}

						// Only act on this entry if it is the latest reference for `hash`:
						// a newer `store`/`renew` (or the force-renew inside
						// `enable_auto_renew`) may have moved `TransactionByContentHash` to a
						// later block, in which case this is a stale shadow entry that should
						// not trigger cleanup or re-schedule auto-renewal — the later entry's
						// own expiry will.
						let is_latest = TransactionByContentHash::<T>::get(hash)
							.is_some_and(|(block, _)| block == obsolete);
						if !is_latest {
							continue;
						}
						TransactionByContentHash::<T>::remove(hash);
						// `try_push` cannot overflow: `pending` is empty per `on_finalize`'s
						// drain invariant, and `transactions.len() <= MaxBlockTransactions`.
						if let Some(renewal_data) = AutoRenewals::<T>::get(hash) {
							let _ = pending.try_push((hash, tx_info, renewal_data));
						}
					}

					if renewed_sum > 0 {
						Self::update_permanent_storage_used(|used| {
							used.saturating_sub(renewed_sum)
						});
					}
					if !pending.is_empty() {
						PendingAutoRenewals::<T>::put(&pending);
					}
				}
			}

			// Charge the expiry-sweep cost via the benchmarked weight. `n = 0` covers the
			// no-expiry path (early blocks, blocks where obsolete had no transactions);
			// the constant component captures the RetentionPeriod read and the
			// reservation for `on_finalize`.
			weight.saturating_accrue(T::WeightInfo::on_initialize_with_expiry(num_expiring));

			weight
		}

		fn on_finalize(n: BlockNumberFor<T>) {
			let proof_ok = <ProofChecked<T>>::take() || {
				// Proof is not required for early or empty blocks.
				let period = Self::retention_period();
				let target_number = n.saturating_sub(period);

				target_number.is_zero() || {
					// An empty block means no transactions were stored, relying on the fact
					// below that we store transactions only if they contain chunks.
					!Transactions::<T>::contains_key(target_number)
				}
			};

			// During try-runtime testing, no inherents (including storage proofs) are
			// submitted, so we log instead of panicking.
			#[cfg(feature = "try-runtime")]
			if !proof_ok {
				tracing::warn!(
					target: LOG_TARGET,
					"Storage proof was not checked in this block (expected during try-runtime)"
				);
			}
			#[cfg(not(feature = "try-runtime"))]
			assert!(proof_ok, "Storage proof must be checked once in the block");

			// All pending auto-renewals must have been processed by the
			// `apply_block_inherents` inherent.
			#[cfg(feature = "try-runtime")]
			if !PendingAutoRenewals::<T>::get().is_empty() {
				tracing::warn!(
					target: LOG_TARGET,
					"Pending auto-renewals were not processed (expected during try-runtime)"
				);
				// Clear pending renewals so try-runtime doesn't leave stale state.
				PendingAutoRenewals::<T>::kill();
			}

			#[cfg(not(feature = "try-runtime"))]
			assert!(
				PendingAutoRenewals::<T>::get().is_empty(),
				"All pending auto-renewals must be processed by apply_block_inherents"
			);

			// Insert new transactions, iff they have chunks.
			let transactions = <BlockTransactions<T>>::take();
			let total_chunks = TransactionInfo::total_chunks(&transactions);
			if total_chunks != 0 {
				<Transactions<T>>::insert(n, transactions);
			}
		}

		#[cfg(feature = "try-runtime")]
		fn try_state(n: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
			Self::do_try_state(n)
		}

		fn integrity_test() {
			assert!(
				!T::MaxBlockTransactions::get().is_zero(),
				"MaxBlockTransactions must be greater than zero"
			);
			assert!(
				!T::MaxTransactionSize::get().is_zero(),
				"MaxTransactionSize must be greater than zero"
			);
			let default_period = DEFAULT_RETENTION_PERIOD.into();
			let retention_period = GenesisConfig::<T>::default().retention_period;
			assert_eq!(
				retention_period, default_period,
				"GenesisConfig.retention_period must match DEFAULT_RETENTION_PERIOD"
			);
			assert!(
				!T::AuthorizationPeriod::get().is_zero(),
				"AuthorizationPeriod must be greater than zero"
			);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Index and store data off chain. Minimum data size is 1 byte, maximum is
		/// `MaxTransactionSize`. Data will be removed after `RetentionPeriod` blocks, unless
		/// `renew` is called.
		///
		/// Authorization is required to store data using regular signed/unsigned transactions.
		/// Regular signed transactions require account authorization (see
		/// [`authorize_account`](Self::authorize_account)), regular unsigned transactions require
		/// preimage authorization (see [`authorize_preimage`](Self::authorize_preimage)).
		///
		/// Emits [`Stored`](Event::Stored) when successful.
		///
		/// ## Complexity
		///
		/// O(n*log(n)) of data size, as all data is pushed to an in-memory trie.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::store(data.len() as u32))]
		#[pallet::feeless_if(|origin: &OriginFor<T>, data: &Vec<u8>| -> bool { true })]
		pub fn store(origin: OriginFor<T>, data: Vec<u8>) -> DispatchResult {
			let caller = Self::ensure_authorized(origin)?;
			Self::do_store_for(caller, data, HashingAlgorithm::Blake2b256, RAW_CODEC)
		}

		/// Index and store data off chain with an explicit CID configuration.
		///
		/// Behaves identically to [`store`](Self::store), but the CID configuration
		/// (codec and hashing algorithm) is passed directly as a parameter.
		///
		/// Emits [`Stored`](Event::Stored) when successful.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::store(data.len() as u32))]
		#[pallet::feeless_if(|_origin: &OriginFor<T>, _cid: &CidConfig, _data: &Vec<u8>| -> bool { true })]
		pub fn store_with_cid_config(
			origin: OriginFor<T>,
			cid: CidConfig,
			data: Vec<u8>,
		) -> DispatchResult {
			let caller = Self::ensure_authorized(origin)?;
			Self::do_store_for(caller, data, cid.hashing, cid.codec)
		}

		/// Store real content against capacity isolated by a Resources reservation.
		///
		/// This is an ordinary paid signed call. `data` must remain the final encoded argument
		/// because the transaction-index host indexes the trailing `data.len()` bytes.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::store_reserved(data.len() as u32))]
		pub fn store_reserved(
			origin: OriginFor<T>,
			reservation_id: ReservationId,
			cid_config: CidConfig,
			data: Vec<u8>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::do_store_reserved(owner, reservation_id, cid_config, data)
		}

		/// Renew the exact current Bulletin copy linked to `content_hash`.
		///
		/// The old and new copies overlap in permanent-byte accounting until normal retention
		/// cleanup removes the old position.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::renew_reserved())]
		pub fn renew_reserved(
			origin: OriginFor<T>,
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::do_renew_reserved(owner, reservation_id, content_hash)
		}

		/// Schedule a **one-shot** auto-renewal of previously stored data. The renewal fires
		/// exactly once, when the data reaches its `RetentionPeriod` boundary, and then the
		/// registration is removed. For continuous renewal, use
		/// [`enable_auto_renew`](Self::enable_auto_renew) instead.
		///
		/// `entry` identifies the data either by `(block, index)` or by content hash.
		///
		/// Feeless. Registration cost (one transaction unit) is charged in `check_signed`;
		/// the eventual renewal cycle charges bytes against `bytes_permanent` and the
		/// chain-wide cap.
		///
		/// Rejects with [`AutoRenewalAlreadyEnabled`](Error::AutoRenewalAlreadyEnabled) if a
		/// scheduled renewal already exists for this content hash.
		///
		/// Emits [`RenewalEnabled`](Event::RenewalEnabled) `{ recurring: false }`.
		///
		/// For synchronous renewal at dispatch time, see [`force_renew`](Self::force_renew).
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::renew())]
		#[pallet::feeless_if(|_origin: &OriginFor<T>, _entry: &TransactionRef<BlockNumberFor<T>>| -> bool { true })]
		pub fn renew(
			origin: OriginFor<T>,
			entry: TransactionRef<BlockNumberFor<T>>,
		) -> DispatchResult {
			let AuthorizedCaller::Signed { who, scope: _ } = Self::ensure_authorized(origin)?
			else {
				return Err(DispatchError::BadOrigin);
			};
			let info = Self::resolve_transaction_ref(&entry)?;
			let content_hash = info.content_hash;

			ensure!(
				!AutoRenewals::<T>::contains_key(content_hash),
				Error::<T>::AutoRenewalAlreadyEnabled
			);

			AutoRenewals::<T>::insert(
				content_hash,
				RenewalData { account: who.clone(), recurring: false, paid: true },
			);
			Self::deposit_event(Event::RenewalEnabled { content_hash, who, recurring: false });
			Ok(())
		}

		/// Immediately renew previously stored data, synchronous at dispatch time.
		///
		/// Authorization is required (as with [`store`](Self::store)). Charges `info.size`
		/// against `bytes_permanent` (per-account renew cap) and `PermanentStorageUsed`
		/// (chain-wide cap).
		///
		/// Emits [`Renewed`](Event::Renewed) when successful.
		#[pallet::call_index(2)]
		#[pallet::weight((T::WeightInfo::force_renew(), DispatchClass::Operational))]
		#[pallet::feeless_if(|_origin: &OriginFor<T>, _entry: &TransactionRef<BlockNumberFor<T>>| -> bool { true })]
		pub fn force_renew(
			origin: OriginFor<T>,
			entry: TransactionRef<BlockNumberFor<T>>,
		) -> DispatchResultWithPostInfo {
			let caller = Self::ensure_authorized(origin)?;
			let info = Self::resolve_transaction_ref(&entry)?;

			// In the case of a regular unsigned transaction, this should have been checked by
			// pre_dispatch. In the case of a regular signed transaction, this should have been
			// checked by pre_dispatch_signed.
			Self::ensure_data_size_ok(info.size as usize)?;

			let content_hash = info.content_hash;
			let charge_root = matches!(&caller, AuthorizedCaller::Root);
			let actor = Self::actor_for_caller(caller, info.content_hash);
			let new_index = Self::do_renew(info, actor, charge_root)?;
			Self::deposit_event(Event::Renewed { index: new_index, content_hash });
			Ok(().into())
		}

		/// Authorize an account to store up to `bytes` of arbitrary data in `transactions`
		/// boost-tier transactions. The authorization will expire after a configured number
		/// of blocks.
		///
		/// If the account already has an unexpired authorization, this call **adds** `bytes`
		/// and `transactions` to the existing `bytes_allowance` and `transactions_allowance`
		/// caps (both saturating); the expiration block is **not** pushed back, and the
		/// consumed counters are preserved. Once the authorization has expired, the next call
		/// replaces it with a fresh entry (consumed counters reset to `0`, allowances set to
		/// the new values, expiry = `now + AuthorizationPeriod`).
		///
		/// Parameters:
		///
		/// - `who`: The account to be credited with an authorization to store data.
		/// - `transactions`: The number of boost-tier transactions that `who` may submit.
		/// - `bytes`: The number of bytes that `who` may submit.
		///
		/// The origin for this call must be the pallet's `Authorizer`. Emits
		/// [`AccountAuthorized`](Event::AccountAuthorized) when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::authorize_account())]
		#[pallet::feeless_if(
			|origin: &OriginFor<T>, _who: &T::AccountId, _transactions: &u32, _bytes: &u64| -> bool {
				Pallet::<T>::is_feeless_authorizer(origin)
			}
		)]
		pub fn authorize_account(
			origin: OriginFor<T>,
			who: T::AccountId,
			transactions: u32,
			bytes: u64,
		) -> DispatchResult {
			let maybe_authorizer = T::Authorizer::ensure_origin(origin)?;
			ensure!(bytes > 0, Error::<T>::BadDataSize);
			Self::authorize(
				AuthorizationScope::Account(who.clone()),
				transactions,
				bytes,
				maybe_authorizer,
			)?;
			Self::deposit_event(Event::AccountAuthorized { who, transactions, bytes });
			Ok(())
		}

		/// Authorize anyone to store a preimage of the given content hash. The authorization will
		/// expire after a configured number of blocks.
		///
		/// If authorization already exists for a preimage of the given hash to be stored, the
		/// maximum size of the preimage will be increased to `max_size`. The expiration block
		/// is **not** pushed back; use
		/// [`refresh_preimage_authorization`](Self::refresh_preimage_authorization) to extend
		/// expiry.
		///
		/// Parameters:
		///
		/// - `content_hash`: The hash of the data to be submitted. For [`store`](Self::store) this
		///   is the BLAKE2b-256 hash; for [`store_with_cid_config`](Self::store_with_cid_config)
		///   this is the hash produced by the CID config's hashing algorithm.
		/// - `max_size`: The maximum size, in bytes, of the preimage.
		///
		/// The origin for this call must be the pallet's `Authorizer`. Emits
		/// [`PreimageAuthorized`](Event::PreimageAuthorized) when successful.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::authorize_preimage())]
		pub fn authorize_preimage(
			origin: OriginFor<T>,
			content_hash: ContentHash,
			max_size: u64,
		) -> DispatchResult {
			let maybe_authorizer = T::Authorizer::ensure_origin(origin)?;
			ensure!(max_size > 0, Error::<T>::BadDataSize);
			// Preimage scope is single-use, so the per-grant tx budget is `1`.
			Self::authorize(
				AuthorizationScope::Preimage(content_hash),
				1,
				max_size,
				maybe_authorizer,
			)?;
			Self::deposit_event(Event::PreimageAuthorized { content_hash, max_size });
			Ok(())
		}

		/// Remove an expired account authorization from storage. Anyone can call this.
		///
		/// Parameters:
		///
		/// - `who`: The account with an expired authorization to remove.
		///
		/// Emits [`ExpiredAccountAuthorizationRemoved`](Event::ExpiredAccountAuthorizationRemoved)
		/// when successful.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::remove_expired_account_authorization())]
		pub fn remove_expired_account_authorization(
			_origin: OriginFor<T>,
			who: T::AccountId,
		) -> DispatchResult {
			Self::remove_expired_authorization(AuthorizationScope::Account(who.clone()))?;
			Self::deposit_event(Event::ExpiredAccountAuthorizationRemoved { who });
			Ok(())
		}

		/// Remove an expired preimage authorization from storage. Anyone can call this.
		///
		/// Parameters:
		///
		/// - `content_hash`: The BLAKE2b hash that was authorized.
		///
		/// Emits
		/// [`ExpiredPreimageAuthorizationRemoved`](Event::ExpiredPreimageAuthorizationRemoved)
		/// when successful.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::remove_expired_preimage_authorization())]
		pub fn remove_expired_preimage_authorization(
			_origin: OriginFor<T>,
			content_hash: ContentHash,
		) -> DispatchResult {
			Self::remove_expired_authorization(AuthorizationScope::Preimage(content_hash))?;
			Self::deposit_event(Event::ExpiredPreimageAuthorizationRemoved { content_hash });
			Ok(())
		}

		/// Refresh the expiration of an existing authorization for an account.
		///
		/// Only the expiration block is updated — consumed counters (`bytes`,
		/// `transactions`) and the granted caps (`bytes_allowance`,
		/// `transactions_allowance`) are left untouched. To extend the caps, call
		/// `authorize_account` instead (additive on the unexpired path).
		///
		/// If the account does not have an authorization, the call will fail.
		///
		/// Parameters:
		///
		/// - `who`: The account to be credited with an authorization to store data.
		///
		/// The origin for this call must be the pallet's `Authorizer`. Emits
		/// [`AccountAuthorizationRefreshed`](Event::AccountAuthorizationRefreshed) when successful.
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::refresh_account_authorization())]
		#[pallet::feeless_if(|origin: &OriginFor<T>, _who: &T::AccountId| -> bool {
			Pallet::<T>::is_feeless_authorizer(origin)
		})]
		pub fn refresh_account_authorization(
			origin: OriginFor<T>,
			who: T::AccountId,
		) -> DispatchResult {
			let maybe_authorizer = T::Authorizer::ensure_origin(origin)?;
			Self::refresh_authorization(
				AuthorizationScope::Account(who.clone()),
				maybe_authorizer,
			)?;
			Self::deposit_event(Event::AccountAuthorizationRefreshed { who });
			Ok(())
		}

		/// Refresh the expiration of an existing authorization for a preimage of a BLAKE2b hash.
		///
		/// Only the expiration block is updated — consumed counters (`bytes`,
		/// `transactions`) and the granted caps (`bytes_allowance`,
		/// `transactions_allowance`) are left untouched. To raise the cap, call
		/// `authorize_preimage` instead.
		///
		/// If the preimage does not have an authorization, the call will fail.
		///
		/// Parameters:
		///
		/// - `content_hash`: The BLAKE2b hash of the data to be submitted.
		///
		/// The origin for this call must be the pallet's `Authorizer`. Emits
		/// [`PreimageAuthorizationRefreshed`](Event::PreimageAuthorizationRefreshed) when
		/// successful.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::refresh_preimage_authorization())]
		pub fn refresh_preimage_authorization(
			origin: OriginFor<T>,
			content_hash: ContentHash,
		) -> DispatchResult {
			let maybe_authorizer = T::Authorizer::ensure_origin(origin)?;
			Self::refresh_authorization(
				AuthorizationScope::Preimage(content_hash),
				maybe_authorizer,
			)?;
			Self::deposit_event(Event::PreimageAuthorizationRefreshed { content_hash });
			Ok(())
		}

		/// Enable automatic renewal for a previously stored piece of data.
		///
		/// **Recurring scheduler with pre-paid first cycle.** The extension's
		/// `check_signed` charges `bytes_permanent`, `PermanentStorageUsed`, and
		/// one tx slot at registration (same hard-cap accounting as `force_renew`
		/// / one-shot `renew`). The registration is inserted as
		/// [`RenewalData`] `{ recurring: true, paid: true }`. The first renewal
		/// cycle fires at the next `RetentionPeriod` boundary **without**
		/// re-charging — the slot is already paid for; the cycle then flips
		/// `paid` to `false`. From that point on, every subsequent cycle charges
		/// the owner's authorization in [`Self::do_process_auto_renewals`],
		/// dropping the registration with [`Event::AutoRenewalFailed`] if the
		/// quota is exhausted at cycle time.
		///
		/// Feeless: no token fee. Spam is bounded structurally by the up-front
		/// hard-cap charge — the caller cannot over-schedule past their
		/// `bytes_allowance` or the chain-wide `MaxPermanentStorageSize`.
		/// [`Self::disable_auto_renew`] additionally rejects the owner while
		/// `paid` is `true`, so the prepayment cannot be reclaimed before the
		/// first cycle fires.
		///
		/// Emits [`RenewalEnabled`](Event::RenewalEnabled) `{ recurring: true }`
		/// for the registration; the first actual renewal is emitted as
		/// [`DataAutoRenewed`](Event::DataAutoRenewed) at cycle time.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::enable_auto_renew())]
		#[pallet::feeless_if(|_origin: &OriginFor<T>, _content_hash: &ContentHash| -> bool { true })]
		pub fn enable_auto_renew(
			origin: OriginFor<T>,
			content_hash: ContentHash,
		) -> DispatchResult {
			let AuthorizedCaller::Signed { who, scope: _ } = Self::ensure_authorized(origin)?
			else {
				return Err(DispatchError::BadOrigin);
			};

			ensure!(
				!AutoRenewals::<T>::contains_key(content_hash),
				Error::<T>::AutoRenewalAlreadyEnabled
			);

			// Defensive content-hash existence check. The hard-cap accounting
			// (`bytes_permanent`, `PermanentStorageUsed`, one tx slot) is performed by
			// the extension's `check_signed` for this call with `is_renew = true`,
			// matching the one-shot `renew`. Registering here must not call
			// `do_renew`, otherwise `bytes_permanent` would be double-charged (once
			// by registration, once by `do_process_auto_renewals`'s prepaid cycle).
			let (block, index) = TransactionByContentHash::<T>::get(content_hash)
				.ok_or(Error::<T>::RenewedNotFound)?;
			Self::transaction_info(block, index).ok_or(Error::<T>::RenewedNotFound)?;

			AutoRenewals::<T>::insert(
				content_hash,
				RenewalData { account: who.clone(), recurring: true, paid: true },
			);
			Self::deposit_event(Event::RenewalEnabled { content_hash, who, recurring: true });
			Ok(())
		}

		/// Disable automatic renewal for a piece of data.
		///
		/// Signed: the caller must be the account that originally enabled the renewal,
		/// and the registration must not be in its prepaid window — see
		/// [`Error::CannotDisablePrepaidAutoRenewal`]. Both registrations from
		/// [`Pallet::renew`] and [`Pallet::enable_auto_renew`] start with `paid: true`;
		/// the owner has to wait for the first cycle to consume the prepayment before
		/// they can disable.
		///
		/// Root: bypasses the owner check and the prepaid-window check
		/// (governance/cleanup).
		///
		/// Feeless: no token fee and no authorization is consumed. Signed admission is
		/// gated in [`check_signed`](Self::check_signed) on ownership and the prepaid
		/// flag, so a caller can issue at most one successful `disable_auto_renew` per
		/// registration it owns — and only after the first cycle has fired.
		///
		/// Emits [`AutoRenewalDisabled`](Event::AutoRenewalDisabled) when successful.
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::disable_auto_renew())]
		#[pallet::feeless_if(|_origin: &OriginFor<T>, _content_hash: &ContentHash| -> bool { true })]
		pub fn disable_auto_renew(
			origin: OriginFor<T>,
			content_hash: ContentHash,
		) -> DispatchResult {
			let caller = Self::ensure_authorized(origin)?;
			let renewal_data =
				AutoRenewals::<T>::get(content_hash).ok_or(Error::<T>::AutoRenewalNotEnabled)?;
			match caller {
				AuthorizedCaller::Signed { who, .. } => {
					ensure!(renewal_data.account == who, Error::<T>::NotAutoRenewalOwner);
					ensure!(!renewal_data.paid, Error::<T>::CannotDisablePrepaidAutoRenewal);
				},
				AuthorizedCaller::Root => {},
				AuthorizedCaller::Unsigned => return Err(DispatchError::BadOrigin),
			}

			AutoRenewals::<T>::remove(content_hash);
			Self::deposit_event(Event::AutoRenewalDisabled {
				content_hash,
				who: renewal_data.account,
			});
			Ok(())
		}

		/// Composite block-level inherent: optionally validates a transaction storage proof and
		/// always drains [`PendingAutoRenewals`].
		///
		/// `ProvideInherent::create_inherent` only returns a single `Call`, but this pallet
		/// has two block-end concerns — verifying the storage proof for the block at
		/// `n - RetentionPeriod`, and renewing entries flagged via [`AutoRenewals`] before
		/// they expire at `n - RetentionPeriod - 1`. Both effects collapse into this single
		/// mandatory inherent so that block authors emit one extrinsic that satisfies both
		/// `on_finalize` invariants (`ProofChecked` and "PendingAutoRenewals empty").
		///
		/// `proof` is `Some` when the inherent data provider supplied one; otherwise the
		/// proof step is skipped (early or empty blocks). The auto-renewal drain runs
		/// unconditionally — emitting an inherent at all implies that `on_initialize` may
		/// have populated `PendingAutoRenewals`.
		#[pallet::call_index(14)]
		#[pallet::weight((
			T::WeightInfo::apply_block_inherents(T::MaxBlockTransactions::get()),
			DispatchClass::Mandatory,
		))]
		pub fn apply_block_inherents(
			origin: OriginFor<T>,
			proof: Option<TransactionStorageProof>,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			if let Some(proof) = proof {
				Self::do_check_proof(proof)?;
			}
			// Refund from the worst-case declaration to the count actually drained.
			let n_actual = Self::do_process_auto_renewals();
			Ok(Some(T::WeightInfo::apply_block_inherents(n_actual)).into())
		}

		/// Add an account to the set of allowed authorizers. Allowed authorizers can call
		/// [`authorize_account`](Self::authorize_account) and
		/// [`authorize_preimage`](Self::authorize_preimage) to grant storage access.
		///
		/// If the account is already an allowed authorizer, its `budget` is **overwritten**
		/// with the new values.
		///
		/// `budget` constraints:
		///
		/// - `valid_until`: when `Some(t)`, must satisfy `t > now`. The entry stops authorizing
		///   once `now >= t` and becomes eligible for permissionless cleanup via
		///   [`remove_exhausted_authorizer`](Self::remove_exhausted_authorizer). Authorizations
		///   granted by this entry have their expiration clamped to `t`.
		///
		/// The origin for this call must satisfy `AuthorizerRegistrarOrigin`. Emits
		/// [`AuthorizerAdded`](Event::AuthorizerAdded) when successful.
		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::add_authorizer())]
		pub fn add_authorizer(
			origin: OriginFor<T>,
			who: T::AccountId,
			budget: AuthorizerBudget<BlockNumberFor<T>>,
		) -> DispatchResult {
			T::AuthorizerRegistrarOrigin::ensure_origin(origin)?;
			ensure!(!budget.is_expired(Self::now()), Error::<T>::InvalidValidUntil);
			let is_new = !AllowedAuthorizers::<T>::contains_key(&who);
			AllowedAuthorizers::<T>::insert(&who, budget);
			if is_new {
				Self::inc_authorizer_providers(&who);
			}
			Self::deposit_event(Event::AuthorizerAdded { who });
			Ok(())
		}

		/// Remove an account from the set of allowed authorizers. The removed account will no
		/// longer be able to call [`authorize_account`](Self::authorize_account) or
		/// [`authorize_preimage`](Self::authorize_preimage).
		///
		/// If the account is not currently an allowed authorizer, this is a no-op.
		///
		/// Parameters:
		///
		/// - `who`: The account to remove from the allowed authorizers.
		///
		/// The origin for this call must satisfy `AuthorizerRegistrarOrigin`. Emits
		/// [`AuthorizerRemoved`](Event::AuthorizerRemoved) when successful.
		#[pallet::call_index(16)]
		#[pallet::weight(T::WeightInfo::remove_authorizer())]
		pub fn remove_authorizer(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			T::AuthorizerRegistrarOrigin::ensure_origin(origin)?;
			// `take` returns the previous value; only emit the event when an entry
			// actually existed so observers don't see phantom "removed" events.
			if AllowedAuthorizers::<T>::take(&who).is_some() {
				Self::dec_authorizer_providers(&who);
				Self::deposit_event(Event::AuthorizerRemoved { who });
			}
			Ok(())
		}

		/// Remove an authorizer that is exhausted (budget zero on either axis) or expired
		/// (`now >= valid_until` for an entry that set `valid_period`). Anyone can call this.
		///
		/// Parameters:
		///
		/// - `who`: The authorizer to remove.
		///
		/// Emits [`ExhaustedAuthorizerRemoved`](Event::ExhaustedAuthorizerRemoved)
		/// when successful.
		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::remove_exhausted_authorizer())]
		pub fn remove_exhausted_authorizer(
			_origin: OriginFor<T>,
			who: T::AccountId,
		) -> DispatchResult {
			let budget =
				AllowedAuthorizers::<T>::get(&who).ok_or(Error::<T>::AuthorizerNotFound)?;
			ensure!(budget.is_inactive(Self::now()), Error::<T>::AuthorizerBudgetNotExhausted,);
			AllowedAuthorizers::<T>::remove(&who);
			Self::dec_authorizer_providers(&who);
			Self::deposit_event(Event::ExhaustedAuthorizerRemoved { who });
			Ok(())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Stored data under specified index.
		Stored {
			index: u32,
			content_hash: ContentHash,
			cid: Option<Cid>,
		},
		/// Renewed data under specified index.
		Renewed {
			index: u32,
			content_hash: ContentHash,
		},
		/// Storage proof was successfully checked.
		ProofChecked,
		/// An account `who` was authorized to store `bytes` bytes in `transactions` boost-tier
		/// transactions.
		AccountAuthorized {
			who: T::AccountId,
			transactions: u32,
			bytes: u64,
		},
		/// An authorization for account `who` was refreshed.
		AccountAuthorizationRefreshed {
			who: T::AccountId,
		},
		/// Authorization was given for a preimage of `content_hash` (not exceeding `max_size`) to
		/// be stored by anyone.
		PreimageAuthorized {
			content_hash: ContentHash,
			max_size: u64,
		},
		/// An authorization for a preimage of `content_hash` was refreshed.
		PreimageAuthorizationRefreshed {
			content_hash: ContentHash,
		},
		/// An expired account authorization was removed.
		ExpiredAccountAuthorizationRemoved {
			who: T::AccountId,
		},
		/// An expired preimage authorization was removed.
		ExpiredPreimageAuthorizationRemoved {
			content_hash: ContentHash,
		},
		/// An authorizer was added to the allowed list.
		AuthorizerAdded {
			who: T::AccountId,
		},
		/// An authorizer was removed from the allowed list by the manager.
		AuthorizerRemoved {
			who: T::AccountId,
		},
		/// An authorizer was removed from the allowed list due to budget exhaustion.
		ExhaustedAuthorizerRemoved {
			who: T::AccountId,
		},
		/// A renewal was enabled for `content_hash` by `who`.
		RenewalEnabled {
			content_hash: ContentHash,
			who: T::AccountId,
			recurring: bool,
		},
		/// Auto-renewal disabled for `content_hash`. `who` is the registration's owner
		/// (not the caller when Root issued the disable).
		AutoRenewalDisabled {
			content_hash: ContentHash,
			who: T::AccountId,
		},
		/// Data was automatically renewed at `index` with `content_hash` for `account`.
		DataAutoRenewed {
			index: u32,
			content_hash: ContentHash,
			account: T::AccountId,
		},
		/// Auto-renewal failed for `content_hash` (insufficient authorization for `account`).
		AutoRenewalFailed {
			content_hash: ContentHash,
			account: T::AccountId,
		},
		/// `PermanentStorageUsed` changed (a `renew` bumped it, or the lazy drain
		/// decremented it). Off-chain capacity-planning consumers can drive their dashboards
		/// from these.
		PermanentStorageUsedUpdated {
			used: u64,
		},
		/// `PermanentStorageUsed` just crossed the [`PERMANENT_STORAGE_NEAR_CAP_PERCENT`]
		/// threshold of `MaxPermanentStorageSize` on the rising edge. Emitted once per
		/// crossing — no re-emission while still above the threshold.
		PermanentStorageNearCap {
			used: u64,
			cap: u64,
		},
		StoredContentProvenanceRecorded {
			bulletin_ref: BulletinRef<BlockNumberFor<T>>,
			actor: StorageActor<T::AccountId>,
		},
		ResourceCapacityReserved {
			reservation_id: ReservationId,
			owner: T::AccountId,
			bytes: u64,
			transactions: u32,
			expires_at: BlockNumberFor<T>,
		},
		ReservedContentStored {
			reservation_id: ReservationId,
			content_hash: ContentHash,
			bulletin_ref: BulletinRef<BlockNumberFor<T>>,
		},
		ReservedContentRenewed {
			reservation_id: ReservationId,
			content_hash: ContentHash,
			bulletin_ref: BulletinRef<BlockNumberFor<T>>,
		},
		ResourceCapacityReleased {
			reservation_id: ReservationId,
			unused_bytes: u64,
			unused_transactions: u32,
			outcome: ResourceClosure,
		},
		ResourceReservationExpired {
			reservation_id: ReservationId,
		},
		ResourceTombstonePruned {
			reservation_id: ReservationId,
			claim_removed: bool,
		},
	}

	/// Authorizations, keyed by scope.
	#[pallet::storage]
	pub(super) type Authorizations<T: Config> =
		StorageMap<_, Blake2_128Concat, AuthorizationScopeFor<T>, AuthorizationFor<T>, OptionQuery>;

	/// List of accounts allowed to give authorizations.
	#[pallet::storage]
	pub type AllowedAuthorizers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AuthorizerBudgetFor<T>, OptionQuery>;

	/// Collection of transaction metadata by block number.
	#[pallet::storage]
	#[pallet::getter(fn transaction_roots)]
	pub type Transactions<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<TransactionInfo, T::MaxBlockTransactions>,
		OptionQuery,
	>;

	#[pallet::storage]
	/// Storage fee per byte.
	pub type ByteFee<T: Config> = StorageValue<_, BalanceOf<T>>;

	#[pallet::storage]
	/// Storage fee per transaction.
	pub type EntryFee<T: Config> = StorageValue<_, BalanceOf<T>>;

	/// Number of blocks for which stored data must be retained.
	///
	/// Data older than `RetentionPeriod` blocks is eligible for removal unless it
	/// has been explicitly renewed. Validators are required to prove possession of
	/// data corresponding to block `N - RetentionPeriod` when producing block `N`.
	#[pallet::storage]
	pub type RetentionPeriod<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	// Intermediates
	#[pallet::storage]
	pub(super) type BlockTransactions<T: Config> =
		StorageValue<_, BoundedVec<TransactionInfo, T::MaxBlockTransactions>, ValueQuery>;

	/// Maps content hash to its most recent (block_number, tx_index) location.
	#[pallet::storage]
	pub(super) type TransactionByContentHash<T: Config> =
		StorageMap<_, Blake2_128Concat, ContentHash, (BlockNumberFor<T>, u32), OptionQuery>;

	/// Maps content hash to the account that registered it for auto-renewal.
	#[pallet::storage]
	pub type AutoRenewals<T: Config> =
		StorageMap<_, Blake2_128Concat, ContentHash, RenewalData<T::AccountId>, OptionQuery>;

	/// Transactions that must be auto-renewed in the current block.
	///
	/// Populated by `on_initialize` when a block's data is about to expire.
	/// Cleared by the `apply_block_inherents` mandatory inherent executed in the same block.
	#[pallet::storage]
	pub(super) type PendingAutoRenewals<T: Config> = StorageValue<
		_,
		BoundedVec<
			(ContentHash, TransactionInfo, RenewalData<T::AccountId>),
			T::MaxBlockTransactions,
		>,
		ValueQuery,
	>;

	/// Was the proof checked in this block?
	#[pallet::storage]
	pub(super) type ProofChecked<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// Chain-wide total of currently-on-chain renewed bytes. Source of truth for the
	/// chain-wide hard cap: a `renew` of `size` bytes is rejected when
	/// `PermanentStorageUsed + size > MaxPermanentStorageSize`.
	///
	/// Bumped on each successful `renew`. Decremented by `on_initialize` when an obsolete
	/// `Transactions[block]` is removed: each entry with `kind == Renew` contributes its
	/// `size` to the decrement.
	#[pallet::storage]
	pub type PermanentStorageUsed<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Explicit V6 actor provenance for each retained transaction position.
	#[pallet::storage]
	pub type StoredBy<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BulletinRef<BlockNumberFor<T>>,
		StorageActor<T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type ResourceReservations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ReservationId,
		ResourceReservation<T::AccountId, BlockNumberFor<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type ResourceReservationExpiryBlocks<T: Config> =
		StorageValue<_, BoundedVec<BlockNumberFor<T>, T::MaxReservationExpiryBlocks>, ValueQuery>;

	#[pallet::storage]
	pub type ResourceReservationExpiryBuckets<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BlockNumberFor<T>,
		BoundedVec<ReservationId, T::MaxReservationsPerExpiryBlock>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type ResourceReservationExpiryCursor<T: Config> =
		StorageValue<_, ResourceExpiryCursor<BlockNumberFor<T>>, ValueQuery>;

	#[pallet::storage]
	pub type ResourceReservationLinks<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		ReservationId,
		Blake2_128Concat,
		ContentHash,
		ResourceReservationLink<T::AccountId, BlockNumberFor<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type ResourceLinkByRef<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		BulletinRef<BlockNumberFor<T>>,
		(ReservationId, ContentHash),
		OptionQuery,
	>;

	/// Exact reverse index used to reject a content hash linked by any reservation without a
	/// global `ResourceReservationLinks` scan.
	#[pallet::storage]
	pub type ResourceLinkByContentHash<T: Config> =
		StorageMap<_, Blake2_128Concat, ContentHash, ReservationId, OptionQuery>;

	/// Number of active reservation links. Kept explicitly so admission is a constant-time read.
	#[pallet::storage]
	pub type ResourceReservationLinkCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// Number of active reservations plus retained tombstones. Active-to-tombstone transitions do
	/// not change this value; successful tombstone pruning decrements it.
	#[pallet::storage]
	pub type ResourceReservationRowCount<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	pub type ResourceReservationTombstones<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ReservationId,
		ResourceReservationTombstone<T::AccountId, BlockNumberFor<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type TombstonePruneQueue<T: Config> =
		StorageValue<_, BoundedVec<ReservationId, T::MaxReservations>, ValueQuery>;

	#[pallet::storage]
	pub type TombstonePruneCursor<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// Sum of unused bytes across active isolated reservations.
	#[pallet::storage]
	pub type ReservedPermanentCapacity<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub byte_fee: BalanceOf<T>,
		pub entry_fee: BalanceOf<T>,
		pub retention_period: BlockNumberFor<T>,
		/// Initial additional accounts that are allowed to issue authorizations and their budgets
		/// as (account, transaction, bytes) tuples.
		pub allowed_authorizers: Vec<(T::AccountId, u32, u64)>,
		/// Initial account authorizations as (account, transactions_allowance, bytes_allowance)
		/// tuples.
		pub account_authorizations: Vec<(T::AccountId, u32, u64)>,
		/// Initial preimage authorizations as (content_hash, max_size) tuples. Each preimage
		/// gets `transactions_allowance = 1`.
		pub preimage_authorizations: Vec<(ContentHash, u64)>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				byte_fee: 10u32.into(),
				entry_fee: 1000u32.into(),
				retention_period: DEFAULT_RETENTION_PERIOD.into(),
				allowed_authorizers: Vec::new(),
				account_authorizations: Vec::new(),
				preimage_authorizations: Vec::new(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			ByteFee::<T>::put(self.byte_fee);
			EntryFee::<T>::put(self.entry_fee);
			RetentionPeriod::<T>::put(self.retention_period);
			let expiration = T::AuthorizationPeriod::get();
			for (who, transactions_allowance, bytes_allowance) in &self.account_authorizations {
				let scope = AuthorizationScope::Account(who.clone());
				Authorizations::<T>::insert(
					&scope,
					Authorization {
						extent: AuthorizationExtent {
							bytes: 0,
							bytes_permanent: 0,
							bytes_allowance: *bytes_allowance,
							transactions: 0,
							transactions_allowance: *transactions_allowance,
						},
						expiration,
					},
				);
				Pallet::<T>::authorization_added(&scope);
			}
			for (content_hash, max_size) in &self.preimage_authorizations {
				let scope = AuthorizationScope::Preimage(*content_hash);
				Authorizations::<T>::insert(
					&scope,
					Authorization {
						extent: AuthorizationExtent {
							bytes: 0,
							bytes_permanent: 0,
							bytes_allowance: *max_size,
							transactions: 0,
							transactions_allowance: 1,
						},
						expiration,
					},
				);
			}
			for (account, transactions, bytes) in &self.allowed_authorizers {
				let is_new = !AllowedAuthorizers::<T>::contains_key(account);
				AllowedAuthorizers::<T>::insert(
					account,
					AuthorizerBudget {
						quota: Some(Quota { transactions: *transactions, bytes: *bytes }),
						// Genesis authorizers never expire; root can re-add them later to set
						// a `valid_until` or flip `feeless`.
						valid_until: None,
						feeless: true,
					},
				);
				if is_new {
					Pallet::<T>::inc_authorizer_providers(account);
				}
			}
		}
	}

	#[pallet::inherent]
	impl<T: Config> ProvideInherent for Pallet<T> {
		type Call = Call<T>;
		type Error = InherentError;
		const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

		fn create_inherent(data: &InherentData) -> Option<Self::Call> {
			// `ProvideInherent::create_inherent` returns a single `Call`, but two block-end
			// concerns may need to land in the same block: verifying the storage proof for the
			// block at `n - RetentionPeriod`, and draining `PendingAutoRenewals` populated by
			// `on_initialize`. Both effects collapse into the composite
			// `Call::apply_block_inherents { proof: Option<_> }`, emitted whenever either side
			// has work to do.
			let proof = data
				.get_data::<TransactionStorageProof>(&Self::INHERENT_IDENTIFIER)
				.unwrap_or(None);
			let has_pending_renewals = !PendingAutoRenewals::<T>::get().is_empty();

			if proof.is_none() && !has_pending_renewals {
				return None;
			}
			Some(Call::apply_block_inherents { proof })
		}

		fn check_inherent(_call: &Self::Call, _data: &InherentData) -> Result<(), Self::Error> {
			Ok(())
		}

		fn is_inherent(call: &Self::Call) -> bool {
			matches!(call, Call::apply_block_inherents { .. })
		}
	}

	// `ValidateUnsigned` is deprecated upstream (will be removed after April 2027) in favour of
	// `#[pallet::authorize]` + `frame_system::AuthorizeCall`. Migration is tracked separately;
	// silence the deprecation here so `-D warnings` in CI does not block the SDK bump.
	#[allow(deprecated)]
	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			// Mandatory inherent (`apply_block_inherents`) is injected by the block author,
			// not the transaction pool. Return a valid but empty transaction if one arrives
			// here.
			if Self::is_inherent(call) {
				return Ok(ValidTransaction::default());
			}
			Self::check_unsigned(call, CheckContext::Validate)?.ok_or(IMPOSSIBLE.into())
		}

		fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
			// Allow inherents here.
			if Self::is_inherent(call) {
				return Ok(());
			}

			Self::check_unsigned(call, CheckContext::PreDispatch).map(|_| ())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Verify a transaction storage proof for the block at `n - RetentionPeriod` and mark
		/// [`ProofChecked`]. Invoked by the [`Self::apply_block_inherents`] mandatory inherent.
		pub(super) fn do_check_proof(proof: TransactionStorageProof) -> DispatchResult {
			ensure!(!ProofChecked::<T>::get(), Error::<T>::DoubleCheck);

			let number = <frame_system::Pallet<T>>::block_number();
			let period = Self::retention_period();
			let target_number = number.saturating_sub(period);
			ensure!(!target_number.is_zero(), Error::<T>::UnexpectedProof);
			// Shape-tolerant: `transactions_at` falls back to the v2 layout while the
			// v2→v3 multi-block migration is still in flight, so historical entries
			// that have not yet been rewritten can still be proof-verified.
			let transactions =
				Self::transactions_at(target_number).ok_or(Error::<T>::MissingStateData)?;

			let parent_hash = frame_system::Pallet::<T>::parent_hash();
			Self::verify_chunk_proof(proof, parent_hash.as_ref(), transactions.to_vec())?;
			ProofChecked::<T>::put(true);
			Self::deposit_event(Event::ProofChecked);
			Ok(())
		}

		/// Drain [`PendingAutoRenewals`] and return the count drained.
		///
		/// Batches the [`BlockTransactions`] read/write across all `n` renewals by threading
		/// an in-memory accumulator through repeated prepared-renew transitions.
		/// A naive `do_renew`-per-item loop would re-encode the full vec per iteration
		/// (O(n²)), which a linear weight model underestimates by ~17% at saturation.
		///
		/// **Failure handling.** A pending renewal is treated as failed (the registration
		/// is removed from [`AutoRenewals`] and [`Event::AutoRenewalFailed`] is emitted)
		/// when any of the following hold:
		///
		/// - [`Self::check_authorization`] rejects — the auth was missing/expired, the per-account
		///   renew quota (`bytes_permanent + size > bytes_allowance`) was exhausted, or the
		///   chain-wide cap (`PermanentStorageUsed + size > MaxPermanentStorageSize`) would be
		///   breached.
		/// - renewal preparation returns `None` because the per-block transaction slot cap
		///   (`MaxBlockTransactions`) is reached.
		///
		/// On failure the data is **gone**: the same `on_initialize` that queued the
		/// pending renewal already `take`-d the obsolete `Transactions` entry and cleared
		/// [`TransactionByContentHash`]. The caller cannot re-`enable_auto_renew` because
		/// the content hash no longer resolves to a stored entry — to keep the data alive
		/// they must re-`store` it first.
		pub(super) fn do_process_auto_renewals() -> u32 {
			let pending = PendingAutoRenewals::<T>::take();
			let n_actual = pending.len() as u32;
			if n_actual == 0 {
				return 0;
			}

			let extrinsic_index = match <frame_system::Pallet<T>>::extrinsic_index() {
				Some(idx) => idx,
				// Defensive: no extrinsic context means we can't index renewals; fail all
				// rather than silently skip.
				None => {
					for (content_hash, _, renewal_data) in pending.into_iter() {
						AutoRenewals::<T>::remove(content_hash);
						Self::deposit_event(Event::AutoRenewalFailed {
							content_hash,
							account: renewal_data.account,
						});
					}
					return n_actual;
				},
			};
			let mut transactions = BlockTransactions::<T>::get();
			let mut prepared = Vec::new();
			let mut failures = Vec::new();
			for (content_hash, tx_info, renewal_data) in pending.into_iter() {
				// `paid = true` means the cycle was already charged at registration. All
				// other recurring cycles charge before any host operation.
				let was_paid = renewal_data.paid;
				let scope = AuthorizationScope::Account(renewal_data.account.clone());
				let charged =
					was_paid || Self::check_authorization(&scope, tx_info.size, true, true).is_ok();
				let maybe_prepared = charged
					.then(|| {
						Self::prepare_renew_in_memory(
							&transactions,
							&tx_info,
							extrinsic_index,
							StorageActor::AutoRenew(renewal_data.account.clone()),
						)
					})
					.flatten();

				if let Some(item) = maybe_prepared {
					Self::stage_prepared_renew(&mut transactions, &item);
					prepared.push((item, renewal_data));
				} else {
					if charged {
						// Reverse only the chain-wide bump. Per-account quota remains burned,
						// preserving the historical slot-cap failure semantics.
						let size_u64: u64 = tx_info.size.into();
						Self::update_permanent_storage_used(|used| used.saturating_sub(size_u64));
					}
					failures.push((content_hash, renewal_data.account));
				}
			}

			// Commit every FRAME mutation for the entire batch before the first host call.
			if !prepared.is_empty() {
				BlockTransactions::<T>::put(transactions);
			}
			for (item, renewal_data) in &prepared {
				Self::commit_prepared_renew_metadata(item);
				if !renewal_data.recurring {
					AutoRenewals::<T>::remove(item.content_hash);
				} else if renewal_data.paid {
					// Do not re-arm a registration removed earlier in this block by Root.
					AutoRenewals::<T>::mutate(item.content_hash, |entry| {
						if let Some(data) = entry {
							data.paid = false;
						}
					});
				}
			}
			for (content_hash, _) in &failures {
				AutoRenewals::<T>::remove(content_hash);
			}

			// Host operations form a final, non-fallible phase. After this begins, only
			// events and the return value are produced.
			for (item, _) in &prepared {
				Self::invoke_renew_host(item.extrinsic_index, item.content_hash);
			}
			for (item, renewal_data) in prepared {
				Self::deposit_event(Event::StoredContentProvenanceRecorded {
					bulletin_ref: item.bulletin_ref,
					actor: item.actor,
				});
				Self::deposit_event(Event::DataAutoRenewed {
					index: item.bulletin_ref.transaction_index,
					content_hash: item.content_hash,
					account: renewal_data.account,
				});
			}
			for (content_hash, account) in failures {
				Self::deposit_event(Event::AutoRenewalFailed { content_hash, account });
			}
			n_actual
		}

		fn prepare_renew_in_memory(
			transactions: &BoundedVec<TransactionInfo, T::MaxBlockTransactions>,
			info: &TransactionInfo,
			extrinsic_index: u32,
			actor: StorageActor<T::AccountId>,
		) -> Option<PreparedRenew<T::AccountId, BlockNumberFor<T>>> {
			if transactions.len() >= T::MaxBlockTransactions::get() as usize {
				return None;
			}
			let block_chunks =
				TransactionInfo::total_chunks(transactions).saturating_add(num_chunks(info.size));
			let new_index = transactions.len() as u32;
			Some(PreparedRenew {
				info: TransactionInfo {
					chunk_root: info.chunk_root,
					size: info.size,
					content_hash: info.content_hash,
					hashing: info.hashing,
					cid_codec: info.cid_codec,
					extrinsic_index,
					block_chunks,
					kind: TransactionKind::Renew,
				},
				content_hash: info.content_hash,
				extrinsic_index,
				bulletin_ref: BulletinRef { block: Self::now(), transaction_index: new_index },
				actor,
			})
		}

		fn stage_prepared_renew(
			transactions: &mut BoundedVec<TransactionInfo, T::MaxBlockTransactions>,
			prepared: &PreparedRenew<T::AccountId, BlockNumberFor<T>>,
		) {
			transactions
				.try_push(prepared.info.clone())
				.expect("prepared renewal reserves a bounded transaction slot");
		}

		fn commit_prepared_renew_metadata(
			prepared: &PreparedRenew<T::AccountId, BlockNumberFor<T>>,
		) {
			TransactionByContentHash::<T>::insert(
				prepared.content_hash,
				(prepared.bulletin_ref.block, prepared.bulletin_ref.transaction_index),
			);
			StoredBy::<T>::insert(prepared.bulletin_ref, &prepared.actor);
		}

		fn invoke_renew_host(extrinsic_index: u32, content_hash: ContentHash) {
			#[cfg(test)]
			RENEW_HOST_OBSERVER.with(|observer| {
				if let Some(callback) = observer.borrow_mut().as_mut() {
					callback();
				}
			});
			sp_io::transaction_index::renew(extrinsic_index, content_hash);
		}

		#[cfg(test)]
		pub(crate) fn set_renew_host_observer(observer: Option<alloc::boxed::Box<dyn FnMut()>>) {
			RENEW_HOST_OBSERVER.with(|slot| *slot.borrow_mut() = observer);
		}
	}

	impl<T: Config> Pallet<T> {
		/// Read [`PermanentStorageUsed`], apply `f` to compute the new value, write it back,
		/// and emit [`Event::PermanentStorageUsedUpdated`]. If the value was below the
		/// [`PERMANENT_STORAGE_NEAR_CAP_PERCENT`] threshold and crossed it (rising edge),
		/// also emit [`Event::PermanentStorageNearCap`].
		///
		/// Centralising read + write + events in one helper guarantees every change to the
		/// chain-wide counter is observable off-chain, and that the near-cap signal fires
		/// exactly once per crossing.
		fn update_permanent_storage_used(f: impl FnOnce(u64) -> u64) {
			let old = PermanentStorageUsed::<T>::get();
			let new = f(old);
			PermanentStorageUsed::<T>::put(new);
			Self::deposit_event(Event::PermanentStorageUsedUpdated { used: new });
			let cap = T::MaxPermanentStorageSize::get();
			// Divide-first to avoid u64 overflow on extreme caps (`cap * 80` saturates
			// above ~230 EiB). Loses ≤`pct` bytes of precision; harmless for the rising-edge.
			let threshold = (cap / 100).saturating_mul(PERMANENT_STORAGE_NEAR_CAP_PERCENT);
			if old < threshold && new >= threshold {
				Self::deposit_event(Event::PermanentStorageNearCap { used: new, cap });
			}
		}

		/// Validate that `origin` is one of the accepted caller types for store/renew
		/// extrinsics, and return a typed description of the caller.
		///
		/// Accepted origins:
		///
		/// - [`Origin::Authorized`] (set by [`extension::ValidateStorageCalls`]) →
		///   [`AuthorizedCaller::Signed`]
		/// - Root → [`AuthorizedCaller::Root`]
		/// - None (unsigned) → [`AuthorizedCaller::Unsigned`]
		///
		/// Any other origin (including plain `Signed`) returns
		/// [`DispatchError::BadOrigin`].
		pub fn ensure_authorized(
			origin: OriginFor<T>,
		) -> Result<AuthorizedCallerFor<T>, DispatchError> {
			// 1. Try pallet::Origin::Authorized (set by ValidateStorageCalls extension)
			if let Ok(Origin::Authorized { who, scope }) = origin.clone().into_caller().try_into() {
				return Ok(AuthorizedCaller::Signed { who, scope });
			}

			// 2. Try root
			if ensure_root(origin.clone()).is_ok() {
				return Ok(AuthorizedCaller::Root);
			}

			// 3. Try none (unsigned)
			ensure_none(origin)?;
			Ok(AuthorizedCaller::Unsigned)
		}

		/// Common implementation for [`store`](Self::store) and
		/// [`store_with_cid_config`](Self::store_with_cid_config).
		///
		/// FOOTGUN: `sp_io::transaction_index::index` (called below) indexes the
		/// *trailing* `data_len` bytes of the encoded extrinsic. Since an extrinsic
		/// encodes as `preamble ++ call`, `data` must be the LAST field of any
		/// dispatchable that funnels into `do_store` (e.g. [`store`](Self::store),
		/// [`store_with_cid_config`](Self::store_with_cid_config),
		/// `pallet-bulletin-hop-promotion::promote`). A field encoded after `data`
		/// shifts the indexed window onto the wrong bytes and corrupts the stored
		/// blob — without any dispatch error to flag it.
		pub fn do_store(
			data: Vec<u8>,
			hashing: HashingAlgorithm,
			cid_codec: CidCodec,
		) -> DispatchResult {
			Self::do_store_with_actor(data, hashing, cid_codec, None)
		}

		fn do_store_for(
			caller: AuthorizedCallerFor<T>,
			data: Vec<u8>,
			hashing: HashingAlgorithm,
			cid_codec: CidCodec,
		) -> DispatchResult {
			Self::do_store_with_actor(data, hashing, cid_codec, Some(caller))
		}

		fn do_store_with_actor(
			data: Vec<u8>,
			hashing: HashingAlgorithm,
			cid_codec: CidCodec,
			caller: Option<AuthorizedCallerFor<T>>,
		) -> DispatchResult {
			let data_len = data.len() as u32;

			// In the case of a regular unsigned transaction, this should have been checked by
			// pre_dispatch. In the case of a regular signed transaction, this should have been
			// checked by pre_dispatch_signed.
			Self::ensure_data_size_ok(data_len as usize)?;

			let cid_config = CidConfig { codec: cid_codec, hashing };
			let cid = calculate_cid(&data, cid_config.clone())
				.map_err(|_| Error::<T>::InvalidContentHash)?;
			let actor = caller.map_or_else(
				|| StorageActor::Preimage(cid.content_hash),
				|caller| Self::actor_for_caller(caller, cid.content_hash),
			);

			// Chunk data and compute storage root
			let chunks: Vec<_> = data.chunks(CHUNK_SIZE).map(|c| c.to_vec()).collect();

			// We don't need `data` anymore.
			core::mem::drop(data);

			let chunk_count = chunks.len() as u32;
			debug_assert_eq!(chunk_count, num_chunks(data_len));
			let root = sp_io::trie::blake2_256_ordered_root(chunks, sp_runtime::StateVersion::V1);

			let extrinsic_index =
				<frame_system::Pallet<T>>::extrinsic_index().ok_or(Error::<T>::BadContext)?;

			let index = Self::append_to_block_transactions(
				root,
				data_len,
				cid.content_hash,
				cid_config,
				extrinsic_index,
				TransactionKind::Store,
				actor,
			)?;
			// Index after the runtime mutation — index ops aren't rolled back on dispatch error.
			// Indexes the trailing `data_len` bytes of the extrinsic, so `data` must be the
			// caller's last call argument (see the FOOTGUN note on `do_store`).
			sp_io::transaction_index::index(extrinsic_index, data_len, cid.content_hash);

			Self::deposit_event(Event::Stored {
				index,
				content_hash: cid.content_hash,
				cid: cid.to_bytes(),
			});

			Ok(())
		}

		/// Single-renewal entry point for the [`force_renew`](Self::force_renew) and
		/// [`enable_auto_renew`](Self::enable_auto_renew) dispatchables.
		///
		/// Prepares and commits the complete FRAME transition before invoking the renewal host.
		///
		/// Hard-cap accounting (per-account `bytes_permanent`, chain-wide
		/// [`PermanentStorageUsed`]) is enforced by [`Self::check_authorization`] in the
		/// extension's `pre_dispatch` before this runs.
		fn do_renew(
			info: TransactionInfo,
			actor: StorageActor<T::AccountId>,
			charge_root: bool,
		) -> Result<u32, Error<T>> {
			let extrinsic_index =
				<frame_system::Pallet<T>>::extrinsic_index().ok_or(Error::<T>::BadContext)?;
			let mut transactions = BlockTransactions::<T>::get();
			let prepared =
				Self::prepare_renew_in_memory(&transactions, &info, extrinsic_index, actor)
					.ok_or(Error::<T>::TooManyTransactions)?;
			if charge_root {
				ensure!(
					Self::has_chain_permanent_capacity(u64::from(info.size)),
					Error::<T>::ChainPermanentCapReached
				);
				Self::update_permanent_storage_used(|used| {
					used.checked_add(u64::from(info.size))
						.expect("root renewal capacity was checked")
				});
			}
			Self::stage_prepared_renew(&mut transactions, &prepared);
			BlockTransactions::<T>::put(transactions);
			Self::commit_prepared_renew_metadata(&prepared);
			Self::invoke_renew_host(prepared.extrinsic_index, prepared.content_hash);
			Self::deposit_event(Event::StoredContentProvenanceRecorded {
				bulletin_ref: prepared.bulletin_ref,
				actor: prepared.actor,
			});
			Ok(prepared.bulletin_ref.transaction_index)
		}

		/// Append a new entry to [`BlockTransactions`] (with the cumulative `block_chunks`)
		/// and update [`TransactionByContentHash`]. Caller must call
		/// `transaction_index::{index,renew}` AFTER this — host calls aren't rolled back on
		/// dispatch error.
		fn append_to_block_transactions(
			chunk_root: <BlakeTwo256 as Hash>::Output,
			size: u32,
			content_hash: ContentHash,
			cid_config: CidConfig,
			extrinsic_index: u32,
			kind: TransactionKind,
			actor: StorageActor<T::AccountId>,
		) -> Result<u32, Error<T>> {
			let new_index = <BlockTransactions<T>>::try_mutate(|transactions| {
				let block_chunks =
					TransactionInfo::total_chunks(transactions).saturating_add(num_chunks(size));
				let new_index = transactions.len() as u32;
				transactions
					.try_push(TransactionInfo {
						chunk_root,
						size,
						content_hash,
						hashing: cid_config.hashing,
						cid_codec: cid_config.codec,
						extrinsic_index,
						block_chunks,
						kind,
					})
					.map_err(|_| Error::<T>::TooManyTransactions)?;
				Ok::<_, Error<T>>(new_index)
			})?;
			TransactionByContentHash::<T>::insert(content_hash, (Self::now(), new_index));
			let bulletin_ref = BulletinRef { block: Self::now(), transaction_index: new_index };
			StoredBy::<T>::insert(bulletin_ref, &actor);
			Self::deposit_event(Event::StoredContentProvenanceRecorded { bulletin_ref, actor });
			Ok(new_index)
		}

		/// Read-only validation shared by the transaction extension and dispatch.
		pub fn prepare_reserved_store(
			owner: &T::AccountId,
			reservation_id: ReservationId,
			cid_config: CidConfig,
			data: &[u8],
		) -> DispatchResult {
			Self::prepare_reserved_store_inner(owner, reservation_id, cid_config, data).map(|_| ())
		}

		fn prepare_reserved_store_inner(
			owner: &T::AccountId,
			reservation_id: ReservationId,
			cid_config: CidConfig,
			data: &[u8],
		) -> Result<PreparedReservedStore<T::AccountId, BlockNumberFor<T>>, DispatchError> {
			Self::ensure_data_size_ok(data.len())?;
			let size = u32::try_from(data.len()).map_err(|_| Error::<T>::ContentTooLarge)?;
			let content_hash = calculate_cid(data, cid_config)
				.map_err(|_| Error::<T>::InvalidContentHash)?
				.content_hash;
			let reservation = ResourceReservations::<T>::get(reservation_id)
				.ok_or(Error::<T>::ReservationNotFound)?;
			ensure!(reservation.owner == *owner, Error::<T>::NotReservationOwner);
			ensure!(Self::now() < reservation.expires_at, Error::<T>::ReservationExpired);
			ensure!(
				reservation.transactions_remaining > 0,
				Error::<T>::TransactionAllowanceExhausted
			);
			ensure!(
				reservation.bytes_remaining >= u64::from(size),
				Error::<T>::BytesAllowanceExhausted
			);
			ensure!(
				!ResourceReservationLinks::<T>::contains_key(reservation_id, content_hash),
				Error::<T>::ContentAlreadyLinked
			);
			ensure!(
				!ResourceLinkByContentHash::<T>::contains_key(content_hash),
				Error::<T>::ContentAlreadyLinked
			);
			if let Some((block, transaction_index)) =
				TransactionByContentHash::<T>::get(content_hash)
			{
				match StoredBy::<T>::get(BulletinRef { block, transaction_index }) {
					Some(StorageActor::Account(ref stored_owner)) if stored_owner == owner => {},
					Some(StorageActor::LegacyUnknown) | None => {
						return Err(Error::<T>::LegacyContentUnrenewable.into())
					},
					_ => return Err(Error::<T>::StoredContentOwnerMismatch.into()),
				}
			}
			ensure!(
				ResourceReservationLinkCount::<T>::get() < T::MaxReservationLinks::get(),
				Error::<T>::ReservationCapacityExceeded
			);
			let transactions = BlockTransactions::<T>::get();
			ensure!(
				(transactions.len() as u32) < T::MaxBlockTransactions::get(),
				Error::<T>::TooManyTransactions
			);
			ensure!(frame_system::Pallet::<T>::extrinsic_index().is_some(), Error::<T>::BadContext);
			Ok(PreparedReservedStore {
				owner: owner.clone(),
				reservation,
				content_hash,
				size,
				bulletin_ref: BulletinRef {
					block: Self::now(),
					transaction_index: transactions.len() as u32,
				},
			})
		}

		fn do_store_reserved(
			owner: T::AccountId,
			reservation_id: ReservationId,
			cid_config: CidConfig,
			data: Vec<u8>,
		) -> DispatchResult {
			let prepared = Self::prepare_reserved_store_inner(
				&owner,
				reservation_id,
				cid_config.clone(),
				&data,
			)?;
			let chunks: Vec<_> = data.chunks(CHUNK_SIZE).map(|chunk| chunk.to_vec()).collect();
			let root = sp_io::trie::blake2_256_ordered_root(chunks, sp_runtime::StateVersion::V1);
			let extrinsic_index = frame_system::Pallet::<T>::extrinsic_index()
				.expect("reserved-store preflight checked extrinsic context");
			let new_reservation = Self::consume_reserved_capacity(
				reservation_id,
				prepared.reservation,
				prepared.size,
			)?;
			let block_chunks = TransactionInfo::total_chunks(&BlockTransactions::<T>::get())
				.saturating_add(num_chunks(prepared.size));
			BlockTransactions::<T>::mutate(|transactions| {
				transactions
					.try_push(TransactionInfo {
						chunk_root: root,
						content_hash: prepared.content_hash,
						hashing: cid_config.hashing,
						cid_codec: cid_config.codec,
						size: prepared.size,
						extrinsic_index,
						block_chunks,
						kind: TransactionKind::Renew,
					})
					.expect("reserved-store preflight checked block capacity");
			});
			let link = ResourceReservationLink {
				reservation_id,
				content_hash: prepared.content_hash,
				bulletin_ref: prepared.bulletin_ref,
				owner: prepared.owner.clone(),
				size: prepared.size,
				retention_boundary: Self::now().saturating_add(Self::retention_period()),
			};
			ResourceReservationLinks::<T>::insert(reservation_id, prepared.content_hash, &link);
			ResourceLinkByContentHash::<T>::insert(prepared.content_hash, reservation_id);
			ResourceReservationLinkCount::<T>::mutate(|count| *count = count.saturating_add(1));
			ResourceLinkByRef::<T>::insert(
				prepared.bulletin_ref,
				(reservation_id, prepared.content_hash),
			);
			TransactionByContentHash::<T>::insert(
				prepared.content_hash,
				(prepared.bulletin_ref.block, prepared.bulletin_ref.transaction_index),
			);
			let actor = StorageActor::Account(prepared.owner.clone());
			StoredBy::<T>::insert(prepared.bulletin_ref, &actor);
			Self::finish_reserved_consumption(reservation_id, new_reservation);
			// Host operation is deliberately last: everything before this point was preflighted.
			sp_io::transaction_index::index(extrinsic_index, prepared.size, prepared.content_hash);
			Self::deposit_event(Event::StoredContentProvenanceRecorded {
				bulletin_ref: prepared.bulletin_ref,
				actor,
			});
			Self::deposit_event(Event::ReservedContentStored {
				reservation_id,
				content_hash: prepared.content_hash,
				bulletin_ref: prepared.bulletin_ref,
			});
			Ok(())
		}

		/// Read-only validation shared by the transaction extension and dispatch.
		pub fn prepare_reserved_renew(
			owner: &T::AccountId,
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> DispatchResult {
			Self::prepare_reserved_renew_inner(owner, reservation_id, content_hash).map(|_| ())
		}

		fn prepare_reserved_renew_inner(
			owner: &T::AccountId,
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> Result<PreparedReservedRenew<T::AccountId, BlockNumberFor<T>>, DispatchError> {
			let reservation = ResourceReservations::<T>::get(reservation_id)
				.ok_or(Error::<T>::ReservationNotFound)?;
			ensure!(reservation.owner == *owner, Error::<T>::NotReservationOwner);
			ensure!(Self::now() < reservation.expires_at, Error::<T>::ReservationExpired);
			let source = ResourceReservationLinks::<T>::get(reservation_id, content_hash)
				.ok_or(Error::<T>::ContentNotFound)?;
			ensure!(
				ResourceLinkByContentHash::<T>::get(content_hash) == Some(reservation_id),
				Error::<T>::BulletinRefHashMismatch
			);
			ensure!(source.owner == *owner, Error::<T>::NotReservationOwner);
			ensure!(
				ResourceLinkByRef::<T>::get(source.bulletin_ref)
					== Some((reservation_id, content_hash)),
				Error::<T>::BulletinRefHashMismatch
			);
			match StoredBy::<T>::get(source.bulletin_ref) {
				Some(StorageActor::Account(ref stored_owner)) if stored_owner == owner => {},
				Some(StorageActor::LegacyUnknown) | None => {
					return Err(Error::<T>::LegacyContentUnrenewable.into())
				},
				_ => return Err(Error::<T>::StoredContentOwnerMismatch.into()),
			}
			let info = Self::transaction_info(
				source.bulletin_ref.block,
				source.bulletin_ref.transaction_index,
			)
			.ok_or(Error::<T>::ContentNotFound)?;
			ensure!(info.content_hash == content_hash, Error::<T>::BulletinRefHashMismatch);
			ensure!(
				reservation.transactions_remaining > 0,
				Error::<T>::TransactionAllowanceExhausted
			);
			ensure!(
				reservation.bytes_remaining >= u64::from(info.size),
				Error::<T>::BytesAllowanceExhausted
			);
			let transactions = BlockTransactions::<T>::get();
			ensure!(
				(transactions.len() as u32) < T::MaxBlockTransactions::get(),
				Error::<T>::TooManyTransactions
			);
			ensure!(frame_system::Pallet::<T>::extrinsic_index().is_some(), Error::<T>::BadContext);
			Ok(PreparedReservedRenew {
				owner: owner.clone(),
				reservation,
				source,
				bulletin_ref: BulletinRef {
					block: Self::now(),
					transaction_index: transactions.len() as u32,
				},
			})
		}

		fn do_renew_reserved(
			owner: T::AccountId,
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> DispatchResult {
			let prepared =
				Self::prepare_reserved_renew_inner(&owner, reservation_id, content_hash)?;
			let info = Self::transaction_info(
				prepared.source.bulletin_ref.block,
				prepared.source.bulletin_ref.transaction_index,
			)
			.expect("reserved-renew preflight resolved source");
			let extrinsic_index = frame_system::Pallet::<T>::extrinsic_index()
				.expect("reserved-renew preflight checked extrinsic context");
			let new_reservation =
				Self::consume_reserved_capacity(reservation_id, prepared.reservation, info.size)?;
			let block_chunks = TransactionInfo::total_chunks(&BlockTransactions::<T>::get())
				.saturating_add(num_chunks(info.size));
			BlockTransactions::<T>::mutate(|transactions| {
				transactions
					.try_push(TransactionInfo {
						chunk_root: info.chunk_root,
						content_hash,
						hashing: info.hashing,
						cid_codec: info.cid_codec,
						size: info.size,
						extrinsic_index,
						block_chunks,
						kind: TransactionKind::Renew,
					})
					.expect("reserved-renew preflight checked block capacity");
			});
			ResourceLinkByRef::<T>::remove(prepared.source.bulletin_ref);
			let link = ResourceReservationLink {
				bulletin_ref: prepared.bulletin_ref,
				retention_boundary: Self::now().saturating_add(Self::retention_period()),
				..prepared.source
			};
			ResourceReservationLinks::<T>::insert(reservation_id, content_hash, &link);
			ResourceLinkByRef::<T>::insert(prepared.bulletin_ref, (reservation_id, content_hash));
			TransactionByContentHash::<T>::insert(
				content_hash,
				(prepared.bulletin_ref.block, prepared.bulletin_ref.transaction_index),
			);
			let actor = StorageActor::Account(prepared.owner);
			StoredBy::<T>::insert(prepared.bulletin_ref, &actor);
			Self::finish_reserved_consumption(reservation_id, new_reservation);
			Self::invoke_renew_host(extrinsic_index, content_hash);
			Self::deposit_event(Event::StoredContentProvenanceRecorded {
				bulletin_ref: prepared.bulletin_ref,
				actor,
			});
			Self::deposit_event(Event::ReservedContentRenewed {
				reservation_id,
				content_hash,
				bulletin_ref: prepared.bulletin_ref,
			});
			Ok(())
		}

		fn consume_reserved_capacity(
			_reservation_id: ReservationId,
			mut reservation: ResourceReservation<T::AccountId, BlockNumberFor<T>>,
			size: u32,
		) -> Result<ResourceReservation<T::AccountId, BlockNumberFor<T>>, DispatchError> {
			reservation.bytes_remaining = reservation
				.bytes_remaining
				.checked_sub(u64::from(size))
				.ok_or(Error::<T>::BytesAllowanceExhausted)?;
			reservation.transactions_remaining = reservation
				.transactions_remaining
				.checked_sub(1)
				.ok_or(Error::<T>::TransactionAllowanceExhausted)?;
			ReservedPermanentCapacity::<T>::mutate(|reserved| {
				*reserved = reserved.saturating_sub(u64::from(size))
			});
			Self::update_permanent_storage_used(|used| used.saturating_add(u64::from(size)));
			Ok(reservation)
		}

		fn finish_reserved_consumption(
			reservation_id: ReservationId,
			reservation: ResourceReservation<T::AccountId, BlockNumberFor<T>>,
		) {
			if reservation.bytes_remaining == 0 || reservation.transactions_remaining == 0 {
				Self::close_reservation(reservation_id, reservation, ResourceClosure::Exhausted);
			} else {
				ResourceReservations::<T>::insert(reservation_id, reservation);
			}
		}

		pub(crate) fn reserve_resource_capacity(
			reservation_id: ReservationId,
			owner: &T::AccountId,
			purpose: &T::ReservationPurpose,
			bytes: u64,
			transactions: u32,
			expires_at: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure!(bytes > 0, Error::<T>::BytesAllowanceExhausted);
			ensure!(transactions > 0, Error::<T>::TransactionAllowanceExhausted);
			ensure!(expires_at > Self::now(), Error::<T>::ReservationExpired);
			ensure!(
				!ResourceReservations::<T>::contains_key(reservation_id)
					&& !ResourceReservationTombstones::<T>::contains_key(reservation_id),
				Error::<T>::ContentAlreadyLinked
			);
			ensure!(
				ResourceReservationRowCount::<T>::get() < T::MaxReservations::get(),
				Error::<T>::ReservationCapacityExceeded
			);
			let total_capacity = PermanentStorageUsed::<T>::get()
				.checked_add(ReservedPermanentCapacity::<T>::get())
				.and_then(|used| used.checked_add(bytes))
				.ok_or(Error::<T>::ReservationCapacityExceeded)?;
			ensure!(
				total_capacity <= T::MaxPermanentStorageSize::get(),
				Error::<T>::ReservationCapacityExceeded
			);

			let mut blocks = ResourceReservationExpiryBlocks::<T>::get();
			let mut bucket = ResourceReservationExpiryBuckets::<T>::get(expires_at);
			let block_position = match blocks.binary_search(&expires_at) {
				Ok(position) => position,
				Err(position) => {
					ensure!(
						(blocks.len() as u32) < T::MaxReservationExpiryBlocks::get(),
						Error::<T>::ExpiryBlockSetFull
					);
					position
				},
			};
			ensure!(
				(bucket.len() as u32) < T::MaxReservationsPerExpiryBlock::get(),
				Error::<T>::ExpiryBucketFull
			);
			let reservation_position = bucket.binary_search(&reservation_id).unwrap_or_else(|p| p);
			ensure!(
				bucket.binary_search(&reservation_id).is_err(),
				Error::<T>::ContentAlreadyLinked
			);
			if blocks.binary_search(&expires_at).is_err() {
				blocks
					.try_insert(block_position, expires_at)
					.map_err(|_| Error::<T>::ExpiryBlockSetFull)?;
			}
			bucket
				.try_insert(reservation_position, reservation_id)
				.map_err(|_| Error::<T>::ExpiryBucketFull)?;

			let purpose_digest = sp_io::hashing::blake2_256(&purpose.encode());
			ResourceReservations::<T>::insert(
				reservation_id,
				ResourceReservation {
					owner: owner.clone(),
					purpose_digest,
					bytes_remaining: bytes,
					transactions_remaining: transactions,
					created_at: Self::now(),
					expires_at,
				},
			);
			ResourceReservationRowCount::<T>::mutate(|count| *count = count.saturating_add(1));
			ResourceReservationExpiryBlocks::<T>::put(blocks);
			ResourceReservationExpiryBuckets::<T>::insert(expires_at, bucket);
			ReservedPermanentCapacity::<T>::mutate(|reserved| *reserved += bytes);
			Self::deposit_event(Event::ResourceCapacityReserved {
				reservation_id,
				owner: owner.clone(),
				bytes,
				transactions,
				expires_at,
			});
			Ok(())
		}

		pub(crate) fn cancel_resource_capacity(
			owner: &T::AccountId,
			reservation_id: ReservationId,
		) -> DispatchResult {
			let reservation = ResourceReservations::<T>::get(reservation_id)
				.ok_or(Error::<T>::ReservationNotFound)?;
			ensure!(reservation.owner == *owner, Error::<T>::NotReservationOwner);
			Self::close_reservation(reservation_id, reservation, ResourceClosure::Cancelled);
			Ok(())
		}

		fn close_reservation(
			reservation_id: ReservationId,
			reservation: ResourceReservation<T::AccountId, BlockNumberFor<T>>,
			outcome: ResourceClosure,
		) {
			ResourceReservations::<T>::remove(reservation_id);
			Self::remove_from_expiry_index(reservation.expires_at, reservation_id);
			ReservedPermanentCapacity::<T>::mutate(|reserved| {
				*reserved = reserved.saturating_sub(reservation.bytes_remaining)
			});
			ResourceReservationTombstones::<T>::insert(
				reservation_id,
				ResourceReservationTombstone {
					owner: reservation.owner,
					purpose_digest: reservation.purpose_digest,
					final_bytes_remaining: reservation.bytes_remaining,
					final_transactions_remaining: reservation.transactions_remaining,
					outcome,
					closed_at: Self::now(),
				},
			);
			TombstonePruneQueue::<T>::mutate(|queue| {
				if let Err(position) = queue.binary_search(&reservation_id) {
					queue
						.try_insert(position, reservation_id)
						.expect("active plus tombstone cap guarantees prune-queue capacity");
				}
			});
			Self::deposit_event(Event::ResourceCapacityReleased {
				reservation_id,
				unused_bytes: reservation.bytes_remaining,
				unused_transactions: reservation.transactions_remaining,
				outcome,
			});
		}

		fn remove_from_expiry_index(expires_at: BlockNumberFor<T>, reservation_id: ReservationId) {
			let mut bucket = ResourceReservationExpiryBuckets::<T>::get(expires_at);
			if let Ok(position) = bucket.binary_search(&reservation_id) {
				bucket.remove(position);
			}
			if bucket.is_empty() {
				ResourceReservationExpiryBuckets::<T>::remove(expires_at);
				ResourceReservationExpiryBlocks::<T>::mutate(|blocks| {
					if let Ok(position) = blocks.binary_search(&expires_at) {
						blocks.remove(position);
					}
				});
			} else {
				ResourceReservationExpiryBuckets::<T>::insert(expires_at, bucket);
			}
		}

		pub(crate) fn expire_due_resource_capacity(
			now: BlockNumberFor<T>,
			limit: u32,
		) -> Result<Vec<ReservationId>, DispatchError> {
			ensure!(limit <= T::MaxReservations::get(), Error::<T>::CleanupLimitExceeded);
			let mut expired = Vec::new();
			let mut inspected = 0u32;
			while inspected < limit {
				let Some(due_block) = ResourceReservationExpiryBlocks::<T>::get().first().copied()
				else {
					break;
				};
				if due_block > now {
					break;
				}
				inspected = inspected.saturating_add(1);
				let Some(reservation_id) =
					ResourceReservationExpiryBuckets::<T>::get(due_block).first().copied()
				else {
					ResourceReservationExpiryBlocks::<T>::mutate(|blocks| {
						if !blocks.is_empty() {
							blocks.remove(0);
						}
					});
					continue;
				};
				if let Some(reservation) = ResourceReservations::<T>::get(reservation_id) {
					Self::close_reservation(reservation_id, reservation, ResourceClosure::Expired);
					Self::deposit_event(Event::ResourceReservationExpired { reservation_id });
					expired.push(reservation_id);
				} else {
					Self::remove_from_expiry_index(due_block, reservation_id);
				}
			}
			let cursor = ResourceReservationExpiryBlocks::<T>::get().first().copied();
			ResourceReservationExpiryCursor::<T>::put(ResourceExpiryCursor {
				block: cursor,
				offset: 0,
			});
			let remaining = limit.saturating_sub(inspected);
			Self::prune_resource_tombstones(now, remaining);
			Ok(expired)
		}

		pub(crate) fn prune_resource_tombstones(now: BlockNumberFor<T>, limit: u32) -> u32 {
			let mut pruned = 0u32;
			let mut inspected = 0u32;
			let inspection_limit = limit
				.min(T::MaxReservations::get())
				.min(TombstonePruneQueue::<T>::get().len() as u32);
			while inspected < inspection_limit {
				let queue = TombstonePruneQueue::<T>::get();
				if queue.is_empty() {
					break;
				}
				let position = (TombstonePruneCursor::<T>::get() as usize) % queue.len();
				let reservation_id = queue[position];
				inspected = inspected.saturating_add(1);
				let Some(tombstone) = ResourceReservationTombstones::<T>::get(reservation_id)
				else {
					TombstonePruneQueue::<T>::mutate(|items| {
						if position < items.len() {
							items.remove(position);
						}
					});
					TombstonePruneCursor::<T>::put(
						(position % queue.len().saturating_sub(1).max(1)) as u32,
					);
					continue;
				};
				let eligible_at = tombstone.closed_at.saturating_add(T::TombstoneRetention::get());
				let has_links =
					ResourceReservationLinks::<T>::iter_prefix(reservation_id).next().is_some();
				if now < eligible_at || has_links {
					TombstonePruneCursor::<T>::put(((position + 1) % queue.len()) as u32);
					continue;
				}
				let outcome = T::ResourceClaimLifecycle::prune_claim(reservation_id);
				let purpose_matches = outcome.purpose.as_ref().is_some_and(|purpose| {
					sp_io::hashing::blake2_256(&purpose.encode()) == tombstone.purpose_digest
				});
				Self::deposit_event(Event::ResourceTombstonePruned {
					reservation_id,
					claim_removed: outcome.removed && purpose_matches,
				});
				if outcome.id == reservation_id && outcome.removed && purpose_matches {
					ResourceReservationTombstones::<T>::remove(reservation_id);
					TombstonePruneQueue::<T>::mutate(|items| {
						items.remove(position);
					});
					TombstonePruneCursor::<T>::put(0);
					ResourceReservationRowCount::<T>::mutate(|count| {
						*count = count.saturating_sub(1)
					});
					pruned += 1;
				} else {
					TombstonePruneCursor::<T>::put(((position + 1) % queue.len()) as u32);
				}
			}
			pruned
		}

		pub fn stored_content_provenance(
			reference: BulletinRef<BlockNumberFor<T>>,
		) -> StorageActor<T::AccountId> {
			StoredBy::<T>::get(reference).unwrap_or(StorageActor::LegacyUnknown)
		}

		pub fn resource_reservation(
			reservation_id: ReservationId,
		) -> Option<ResourceReservationView<T::AccountId, BlockNumberFor<T>>> {
			ResourceReservations::<T>::get(reservation_id)
				.map(ResourceReservationView::Active)
				.or_else(|| {
					ResourceReservationTombstones::<T>::get(reservation_id)
						.map(ResourceReservationView::Tombstone)
				})
		}

		pub fn resource_reservation_link(
			reservation_id: ReservationId,
			content_hash: ContentHash,
		) -> Option<ResourceReservationLink<T::AccountId, BlockNumberFor<T>>> {
			ResourceReservationLinks::<T>::get(reservation_id, content_hash)
		}

		fn actor_for_caller(
			caller: AuthorizedCallerFor<T>,
			content_hash: ContentHash,
		) -> StorageActor<T::AccountId> {
			match caller {
				AuthorizedCaller::Signed { who, .. } => StorageActor::Account(who),
				AuthorizedCaller::Root => StorageActor::Root,
				AuthorizedCaller::Unsigned => StorageActor::Preimage(content_hash),
			}
		}

		/// Current block number — local shorthand for `frame_system::Pallet::<T>::block_number()`.
		pub(crate) fn now() -> BlockNumberFor<T> {
			frame_system::Pallet::<T>::block_number()
		}

		fn authorization_added(scope: &AuthorizationScopeFor<T>) {
			match scope {
				AuthorizationScope::Account(who) => {
					// Allow nonce storage for transaction replay protection
					frame_system::Pallet::<T>::inc_providers(who);
				},
				AuthorizationScope::Preimage(_) => (),
			}
		}

		fn authorization_removed(scope: &AuthorizationScopeFor<T>) {
			match scope {
				AuthorizationScope::Account(who) => {
					// Cleanup nonce storage. Authorized accounts should be careful to use a short
					// enough lifetime for their store/renew transactions that they aren't at risk
					// of replay when the account is next authorized.
					if let Err(err) = frame_system::Pallet::<T>::dec_providers(who) {
						tracing::warn!(
							target: LOG_TARGET,
							error=?err, ?who,
							"Failed to decrement provider reference count for authorized account leaking reference"
						);
					}
				},
				AuthorizationScope::Preimage(_) => (),
			}
		}

		/// Keep `who` alive in System while it's registered in [`AllowedAuthorizers`],
		/// so a `feeless` authorizer with no balance doesn't get reaped between
		/// dispatches (which would reset its replay-protection nonce).
		pub(crate) fn inc_authorizer_providers(who: &T::AccountId) {
			frame_system::Pallet::<T>::inc_providers(who);
		}

		pub(crate) fn dec_authorizer_providers(who: &T::AccountId) {
			if let Err(err) = frame_system::Pallet::<T>::dec_providers(who) {
				tracing::warn!(
					target: LOG_TARGET, error=?err, ?who,
					"dec_providers failed for allowed authorizer; leaking reference",
				);
			}
		}

		/// Authorize data storage for a scope. Behaviour for an existing entry:
		/// - **Expired-but-present**: re-grant the caps and reset **all** consumed counters
		///   (`bytes`, `bytes_permanent`, `transactions`) to `0`. The new window is fully
		///   independent of the old one. Pre-existing renewed bytes from the old window are tracked
		///   by the chain-wide [`PermanentStorageUsed`] counter and aged out by `on_initialize`
		///   when their `Transactions` block becomes obsolete; they do not spend the new window's
		///   quota.
		/// - **Unexpired Account**: caps are additive — `claim_long_term_storage` (and similar
		///   flows on caller chains) calls this once per claim and expects each to extend the caps.
		///   Consumed counters (`bytes`, `bytes_permanent`, `transactions`) are preserved. Expiry
		///   is left untouched until the authorization expires, at which point the next call
		///   (above) restarts the window.
		/// - **Unexpired Preimage**: caps are replaced (preimage grants are point-in-time);
		///   consumed counters preserved.
		/// - **Missing**: create a fresh entry with all counters at `0`.
		///
		/// When `auth` is `Some(ctx)`, the dispatcher is an [`AllowedAuthorizers`]
		/// entry: `ctx.authorizer`'s per-call budget is decremented (and an early error
		/// returned if the budget can't cover the request), and the new authorization's
		/// expiration is clamped to `ctx.valid_until` if set — a grant cannot outlive
		/// the authorizer that issued it. When `auth` is `None` (Root / XCM / other
		/// non-account authorizers) no budget is consumed and no clamping is applied.
		fn authorize(
			scope: AuthorizationScopeFor<T>,
			transactions_allowance: u32,
			bytes_allowance: u64,
			auth: Option<AuthorizationOriginFor<T>>,
		) -> DispatchResult {
			let now = Self::now();
			let mut expiration = now.saturating_add(T::AuthorizationPeriod::get());

			if let Some(ctx) = auth {
				Self::consume_authorizer_budget(
					&ctx.authorizer,
					transactions_allowance,
					bytes_allowance,
				)?;
				if let Some(vu) = ctx.valid_until {
					expiration = expiration.min(vu);
				}
			}

			Authorizations::<T>::mutate(&scope, |maybe_authorization| {
				if let Some(authorization) = maybe_authorization {
					if authorization.expired(now) {
						// Expired-but-present: re-grant the caps, reset all consumed counters.
						// The new window's `bytes_permanent` quota is independent of any
						// renewed bytes still on chain from the old window.
						authorization.expiration = expiration;
						authorization.extent.bytes = 0;
						authorization.extent.bytes_permanent = 0;
						authorization.extent.transactions = 0;
						authorization.extent.bytes_allowance = bytes_allowance;
						authorization.extent.transactions_allowance = transactions_allowance;
					} else {
						match scope {
							// Account grants are additive within an unexpired window:
							// `claim_long_term_storage` (and similar flows on caller chains)
							// calls this once per claim and expects each to extend the caps.
							// Expiry is left untouched until the authorization expires, at
							// which point the next call (above) creates a fresh entry.
							AuthorizationScope::Account(_) => {
								authorization.extent.bytes_allowance = authorization
									.extent
									.bytes_allowance
									.saturating_add(bytes_allowance);
								authorization.extent.transactions_allowance = authorization
									.extent
									.transactions_allowance
									.saturating_add(transactions_allowance);
							},
							AuthorizationScope::Preimage(_) => {
								authorization.extent.bytes_allowance = bytes_allowance;
								authorization.extent.transactions_allowance =
									transactions_allowance;
							},
						}
					}
				} else {
					*maybe_authorization = Some(Authorization {
						extent: AuthorizationExtent {
							bytes: 0,
							bytes_permanent: 0,
							bytes_allowance,
							transactions: 0,
							transactions_allowance,
						},
						expiration,
					});
					Self::authorization_added(&scope);
				}
			});
			Ok(())
		}

		/// Refresh an existing authorization by extending its expiration. Consumed counters
		/// (`bytes`, `bytes_permanent`, `transactions`) are left untouched — refresh does not
		/// grant additional capacity. To extend the caps, call `authorize_account` (additive
		/// on the unexpired path); to start a fresh quota window, let the authorization
		/// expire and re-authorize.
		///
		/// Same `valid_until` clamp as [`authorize`]: a grant cannot outlive its grantor.
		fn refresh_authorization(
			scope: AuthorizationScopeFor<T>,
			auth: Option<AuthorizationOriginFor<T>>,
		) -> DispatchResult {
			let mut expiration = Self::now().saturating_add(T::AuthorizationPeriod::get());
			if let Some(vu) = auth.and_then(|ctx| ctx.valid_until) {
				expiration = expiration.min(vu);
			}

			Authorizations::<T>::mutate(&scope, |maybe_authorization| {
				if let Some(authorization) = maybe_authorization {
					authorization.expiration = expiration;
					Ok(())
				} else {
					// No previous authorization to refresh.
					Err(Error::<T>::AuthorizationNotFound.into())
				}
			})
		}

		/// Remove an expired authorization.
		fn remove_expired_authorization(scope: AuthorizationScopeFor<T>) -> DispatchResult {
			let authorization =
				Authorizations::<T>::get(&scope).ok_or(Error::<T>::AuthorizationNotFound)?;
			ensure!(authorization.expired(Self::now()), Error::<T>::AuthorizationNotExpired);
			Authorizations::<T>::remove(&scope);
			Self::authorization_removed(&scope);
			Ok(())
		}

		fn authorization_extent(scope: AuthorizationScopeFor<T>) -> AuthorizationExtent {
			let Some(authorization) = Authorizations::<T>::get(&scope) else {
				return AuthorizationExtent::default();
			};
			if authorization.expired(Self::now()) {
				AuthorizationExtent::default()
			} else {
				authorization.extent
			}
		}

		/// Returns the (unused and unexpired) authorization extent for the given account.
		pub fn account_authorization_extent(who: T::AccountId) -> AuthorizationExtent {
			Self::authorization_extent(AuthorizationScope::Account(who))
		}

		/// Active-authorization summary for `who`, shaped for the
		/// [`BulletinTransactionStorageApi`] runtime API. Returns `None` if the
		/// account has no authorization or its authorization has expired.
		///
		/// [`BulletinTransactionStorageApi`]:
		/// pallet_bulletin_transaction_storage_runtime_api::BulletinTransactionStorageApi
		pub fn account_authorization(
			who: T::AccountId,
		) -> Option<AccountAuthorization<BlockNumberFor<T>>> {
			let auth = Authorizations::<T>::get(AuthorizationScope::Account(who))?;
			(!auth.expired(Self::now())).then_some(AccountAuthorization {
				expires_at: auth.expiration,
				bytes_allowance: auth.extent.bytes_allowance,
				bytes_used: auth.extent.bytes,
				bytes_permanent_used: auth.extent.bytes_permanent,
				transactions_allowance: auth.extent.transactions_allowance,
				transactions_used: auth.extent.transactions,
			})
		}

		/// Returns `true` iff a `store(data)` call where `data.len() == data_len`
		/// would currently pass transaction validation for `who`.
		///
		/// Mirrors the preconditions enforced by [`Self::store`] +
		/// [`Self::check_authorization`] (`is_renew = false`):
		///
		/// - `data_len` is within `[1, MaxTransactionSize]`
		/// - `who` has an unexpired authorization entry
		///
		/// `store` saturates against `bytes` / `transactions` and uses the priority
		/// boost (soft limit), so no per-account or chain-wide hard cap applies here.
		pub fn can_store(who: &T::AccountId, data_len: u32) -> bool {
			if !Self::data_size_ok(data_len as usize) {
				return false;
			}
			Self::account_has_active_authorization(who)
		}

		/// Returns `true` iff a `renew(entry)` call would currently pass transaction
		/// validation for `who`.
		///
		/// Mirrors the preconditions enforced by [`Self::renew`] +
		/// [`Self::check_authorization`] (`is_renew = true`):
		///
		/// - `entry` resolves to currently-stored data
		/// - the stored data's size is within `[1, MaxTransactionSize]`
		/// - `who` has an unexpired authorization entry
		/// - per-account hard cap: `bytes_permanent + size <= bytes_allowance`
		/// - chain-wide hard cap: `PermanentStorageUsed + ReservedPermanentCapacity + size <=
		///   MaxPermanentStorageSize`
		pub fn can_renew(who: &T::AccountId, entry: &TransactionRef<BlockNumberFor<T>>) -> bool {
			let Ok(info) = Self::resolve_transaction_ref(entry) else { return false };
			if !Self::data_size_ok(info.size as usize) {
				return false;
			}
			let Some(auth) = Authorizations::<T>::get(AuthorizationScope::Account(who.clone()))
			else {
				return false;
			};
			if auth.expired(Self::now()) {
				return false;
			}
			let size: u64 = info.size.into();
			if !auth.extent.has_permanent_capacity(size) {
				return false;
			}
			Self::has_chain_permanent_capacity(size)
		}

		fn has_chain_permanent_capacity(size: u64) -> bool {
			PermanentStorageUsed::<T>::get()
				.checked_add(ReservedPermanentCapacity::<T>::get())
				.and_then(|committed| committed.checked_add(size))
				.is_some_and(|committed| committed <= T::MaxPermanentStorageSize::get())
		}

		/// Returns `true` if `who` has an authorization entry that has not yet expired,
		/// regardless of how much of the extent remains. The entry is only cleared when
		/// its expiration is reached and someone calls
		/// [`remove_expired_account_authorization`], so a fully-consumed-but-in-window
		/// account still counts as active here. HOP promotion uses this to keep
		/// promoting blobs for an account that has spent all of its store/renew quota.
		pub fn account_has_active_authorization(who: &T::AccountId) -> bool {
			Authorizations::<T>::get(AuthorizationScope::Account(who.clone()))
				.is_some_and(|a| !a.expired(Self::now()))
		}

		/// Returns the (unused and unexpired) authorization extent for the given content hash.
		pub fn preimage_authorization_extent(hash: ContentHash) -> AuthorizationExtent {
			Self::authorization_extent(AuthorizationScope::Preimage(hash))
		}

		/// Validate a signed TransactionStorage call.
		///
		/// Returns `(ValidTransaction, Some(scope))` for store/renew calls (origin should be
		/// transformed to carry authorization info).
		/// Returns `(ValidTransaction, None)` for authorizer calls (origin unchanged).
		/// Returns `Err(InvalidTransaction::Call)` for other calls.
		///
		/// This should be called from a `TransactionExtension` implementation.
		pub fn validate_signed(
			who: &T::AccountId,
			call: &Call<T>,
		) -> Result<(ValidTransaction, Option<AuthorizationScopeFor<T>>), TransactionValidityError>
		{
			let (valid_tx, scope) = Self::check_signed(who, call, CheckContext::Validate)?;
			Ok((valid_tx.ok_or(IMPOSSIBLE)?, scope))
		}

		/// Check the validity of the given call, signed by the given account, and consume
		/// authorization for it.
		///
		/// This is equivalent to `pre_dispatch` but for signed transactions. It should be called
		/// from a `TransactionExtension` implementation.
		pub fn pre_dispatch_signed(
			who: &T::AccountId,
			call: &Call<T>,
		) -> Result<(), TransactionValidityError> {
			let _ = Self::check_signed(who, call, CheckContext::PreDispatch)?;
			Ok(())
		}

		/// Get ByteFee storage information from the outside of this pallet.
		pub fn byte_fee() -> Option<BalanceOf<T>> {
			ByteFee::<T>::get()
		}

		/// Get EntryFee storage information from the outside of this pallet.
		pub fn entry_fee() -> Option<BalanceOf<T>> {
			EntryFee::<T>::get()
		}

		/// Get RetentionPeriod storage information from the outside of this pallet.
		pub fn retention_period() -> BlockNumberFor<T> {
			RetentionPeriod::<T>::get()
		}

		/// Whether `content_hash` is currently stored on-chain — i.e. some
		/// retained transaction in this pallet indexes it.
		///
		/// O(1): one [`TransactionByContentHash`] map read. The map's
		/// lifecycle matches the question's semantics — `store`/`renew`
		/// insert (or overwrite to the latest `(block, index)`), and
		/// `on_initialize` removes the entry when the block it points at
		/// ages out of [`RetentionPeriod`].
		pub fn contains_transaction(content_hash: ContentHash) -> bool {
			TransactionByContentHash::<T>::contains_key(content_hash)
		}

		/// Returns `true` if a blob of the given size can be stored.
		pub fn data_size_ok(size: usize) -> bool {
			(size > 0) && (size <= T::MaxTransactionSize::get() as usize)
		}

		/// Ensures that the given data size is valid for storage.
		fn ensure_data_size_ok(size: usize) -> Result<(), Error<T>> {
			ensure!(Self::data_size_ok(size), Error::<T>::BadDataSize);
			Ok(())
		}

		/// Returns the [`TransactionInfo`] for the specified store/renew transaction.
		pub(crate) fn transaction_info(
			block_number: BlockNumberFor<T>,
			index: u32,
		) -> Option<TransactionInfo> {
			let transactions = Transactions::<T>::get(block_number)?;
			transactions.into_iter().nth(index as usize)
		}

		/// Resolves a [`TransactionRef`] to its [`TransactionInfo`].
		fn resolve_transaction_ref(
			entry: &TransactionRef<BlockNumberFor<T>>,
		) -> Result<TransactionInfo, Error<T>> {
			let (block, index) = match entry {
				TransactionRef::Position { block, index } => (*block, *index),
				TransactionRef::ContentHash(hash) => {
					TransactionByContentHash::<T>::get(hash).ok_or(Error::<T>::RenewedNotFound)?
				},
			};
			Self::transaction_info(block, index).ok_or(Error::<T>::RenewedNotFound)
		}

		/// All transactions stored at the given block, in the current `TransactionInfo` layout.
		///
		/// Shape-tolerant against entries that are still in the pre-v3 layout.
		pub fn transactions_at(
			block: BlockNumberFor<T>,
		) -> Option<BoundedVec<TransactionInfo, T::MaxBlockTransactions>> {
			let raw = sp_io::storage::get(&Transactions::<T>::hashed_key_for(block))?;

			if let Ok(v3) =
				BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::decode(&mut &raw[..])
			{
				return Some(v3);
			}

			let v2 = BoundedVec::<
				crate::migrations::v3::V2TransactionInfo,
				T::MaxBlockTransactions,
			>::decode(&mut &raw[..])
			.ok()?;

			let materialized: Vec<TransactionInfo> = v2
				.into_iter()
				.map(|tx| TransactionInfo {
					chunk_root: tx.chunk_root,
					content_hash: tx.content_hash,
					hashing: tx.hashing,
					cid_codec: tx.cid_codec,
					size: tx.size,
					extrinsic_index: u32::MAX,
					block_chunks: tx.block_chunks,
					kind: TransactionKind::Store,
				})
				.collect();

			BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::try_from(materialized).ok()
		}

		/// Returns `true` if no more store/renew transactions can be included in the current
		/// block.
		pub fn block_transactions_full() -> bool {
			BlockTransactions::<T>::decode_len()
				.is_some_and(|len| len >= T::MaxBlockTransactions::get() as usize)
		}

		/// Check that authorization exists for data of the given size.
		///
		/// Always rejects if the authorization entry is missing or expired.
		///
		/// For `store` (`is_renew == false`): never rejects on insufficient allowance —
		/// `bytes` and `transactions` saturate upward and the
		/// [`extension::AllowanceBasedPriority`] boost is what handles the overshoot (soft
		/// limit).
		///
		/// For `renew` (`is_renew == true`): hard cap. Rejects with
		/// [`PERMANENT_ALLOWANCE_EXCEEDED`] if the per-account check fails
		/// (`bytes_permanent + size > bytes_allowance`) or with
		/// [`CHAIN_PERMANENT_CAP_REACHED`] if the chain-wide check fails
		/// (`PermanentStorageUsed + ReservedPermanentCapacity + size >
		/// MaxPermanentStorageSize`). Arithmetic overflow is rejection, never saturation.
		///
		/// If `consume` is `true` and the checks pass, increments either `bytes` (store) or
		/// `bytes_permanent` (renew) by `size`, and `transactions` by 1 (all saturating).
		/// For renew, the chain-wide `PermanentStorageUsed` counter is also bumped; the
		/// matching decrement happens in `on_initialize` when the obsolete `Transactions`
		/// entry is removed.
		fn check_authorization(
			scope: &AuthorizationScopeFor<T>,
			size: u32,
			consume: bool,
			is_renew: bool,
		) -> Result<(), TransactionValidityError> {
			let size_u64: u64 = size.into();
			let now = Self::now();

			let check = |maybe_authorization: &mut Option<Authorization<_>>|
			 -> Result<(), TransactionValidityError> {
				let Some(authorization) = maybe_authorization else {
					return Err(InvalidTransaction::Payment.into())
				};
				if authorization.expired(now) {
					return Err(InvalidTransaction::Payment.into())
				}
				if is_renew {
					// Per-account hard cap (per-window quota).
					if !authorization.extent.has_permanent_capacity(size_u64) {
						return Err(PERMANENT_ALLOWANCE_EXCEEDED.into())
					}
					// Chain-wide hard cap.
					if !Self::has_chain_permanent_capacity(size_u64) {
						return Err(CHAIN_PERMANENT_CAP_REACHED.into())
					}
				}
				if consume {
					if is_renew {
						authorization.extent.bytes_permanent = authorization
							.extent
							.bytes_permanent
							.saturating_add(size_u64);
					} else {
						authorization.extent.bytes =
							authorization.extent.bytes.saturating_add(size_u64);
					}
					authorization.extent.transactions =
						authorization.extent.transactions.saturating_add(1);
				}
				Ok(())
			};

			let result = if consume {
				Authorizations::<T>::mutate(scope, check)
			} else {
				let mut authorization = Authorizations::<T>::get(scope);
				check(&mut authorization)
			};

			// On a successful renew consume: bump the chain-wide counter. The matching
			// decrement happens in `on_initialize` when the renewed entry's block becomes
			// obsolete and `Transactions[obsolete]` is removed.
			if result.is_ok() && consume && is_renew {
				Self::update_permanent_storage_used(|used| used.saturating_add(size_u64));
			}

			result
		}

		/// Check that authorization with the given scope exists in storage, has expired, and
		/// has no outstanding permanent storage. Mirrors the dispatch-time guard in
		/// [`remove_expired_authorization`] so that `remove_expired_*` calls are rejected at
		/// pool ingress when they cannot succeed (no pool pollution from soon-to-fail txs).
		fn check_authorization_expired(
			scope: &AuthorizationScopeFor<T>,
		) -> Result<(), TransactionValidityError> {
			let Some(authorization) = Authorizations::<T>::get(scope) else {
				return Err(AUTHORIZATION_NOT_FOUND.into());
			};
			if !authorization.expired(Self::now()) {
				return Err(AUTHORIZATION_NOT_EXPIRED.into());
			}
			Ok(())
		}

		fn preimage_store_renew_valid_transaction(content_hash: ContentHash) -> ValidTransaction {
			ValidTransaction::with_tag_prefix("TransactionStorageStoreRenew")
				.and_provides(content_hash)
				.priority(T::StoreRenewPriority::get())
				.longevity(T::StoreRenewLongevity::get())
				.into()
		}

		fn check_store_renew_unsigned(
			size: usize,
			content_hash: impl FnOnce() -> ContentHash,
			context: CheckContext,
			is_renew: bool,
		) -> Result<Option<ValidTransaction>, TransactionValidityError> {
			if !Self::data_size_ok(size) {
				return Err(BAD_DATA_SIZE.into());
			}

			if Self::block_transactions_full() {
				return Err(InvalidTransaction::ExhaustsResources.into());
			}

			let content_hash = content_hash();

			Self::check_authorization(
				&AuthorizationScope::Preimage(content_hash),
				size as u32,
				context.consume_authorization(),
				is_renew,
			)?;

			Ok(context
				.want_valid_transaction()
				.then(|| Self::preimage_store_renew_valid_transaction(content_hash)))
		}

		fn check_unsigned(
			call: &Call<T>,
			context: CheckContext,
		) -> Result<Option<ValidTransaction>, TransactionValidityError> {
			match call {
				Call::<T>::store { data } => Self::check_store_renew_unsigned(
					data.len(),
					|| sp_io::hashing::blake2_256(data),
					context,
					false,
				),
				Call::<T>::store_with_cid_config { cid, data } => Self::check_store_renew_unsigned(
					data.len(),
					|| cid.hashing.hash(data),
					context,
					false,
				),
				Call::<T>::force_renew { entry } => {
					let info =
						Self::resolve_transaction_ref(entry).map_err(|_| RENEWED_NOT_FOUND)?;
					Self::check_store_renew_unsigned(
						info.size as usize,
						|| info.content_hash,
						context,
						true,
					)
				},
				// `renew` (one-shot scheduler) is signed-only — needs `who` to record in
				// `AutoRenewals`. Reject unsigned/preimage submissions.
				Call::<T>::renew { .. } => Err(InvalidTransaction::Call.into()),
				Call::<T>::remove_expired_account_authorization { who } => {
					Self::check_authorization_expired(&AuthorizationScope::Account(who.clone()))?;
					Ok(context.want_valid_transaction().then(|| {
						ValidTransaction::with_tag_prefix(
							"TransactionStorageRemoveExpiredAccountAuthorization",
						)
						.and_provides(who)
						.priority(T::RemoveExpiredAuthorizationPriority::get())
						.longevity(T::RemoveExpiredAuthorizationLongevity::get())
						.into()
					}))
				},
				Call::<T>::remove_expired_preimage_authorization { content_hash } => {
					Self::check_authorization_expired(&AuthorizationScope::Preimage(
						*content_hash,
					))?;
					Ok(context.want_valid_transaction().then(|| {
						ValidTransaction::with_tag_prefix(
							"TransactionStorageRemoveExpiredPreimageAuthorization",
						)
						.and_provides(content_hash)
						.priority(T::RemoveExpiredAuthorizationPriority::get())
						.longevity(T::RemoveExpiredAuthorizationLongevity::get())
						.into()
					}))
				},
				Call::<T>::remove_exhausted_authorizer { who } => {
					let budget = AllowedAuthorizers::<T>::get(who).ok_or(AUTHORIZER_NOT_FOUND)?;
					ensure!(budget.is_inactive(Self::now()), AUTHORIZATION_NOT_EXHAUSTED);
					Ok(context.want_valid_transaction().then(|| {
						ValidTransaction::with_tag_prefix(
							"TransactionStorageRemoveExhaustedAuthorizer",
						)
						.and_provides(who)
						.priority(T::RemoveExpiredAuthorizationPriority::get())
						.longevity(T::RemoveExpiredAuthorizationLongevity::get())
						.into()
					}))
				},
				// Mandatory inherent — always allowed, no pool validation needed.
				Call::<T>::apply_block_inherents { .. } => Ok(None),
				_ => Err(InvalidTransaction::Call.into()),
			}
		}

		fn check_signed(
			who: &T::AccountId,
			call: &Call<T>,
			context: CheckContext,
		) -> Result<
			(Option<ValidTransaction>, Option<AuthorizationScopeFor<T>>),
			TransactionValidityError,
		> {
			let (size, content_hash, is_renew) = match call {
				Call::<T>::store_reserved { reservation_id, cid_config, data } => {
					Self::prepare_reserved_store(who, *reservation_id, cid_config.clone(), data)
						.map_err(|_| InvalidTransaction::Call)?;
					let content_hash = cid_config.hashing.hash(data);
					return Ok((
						context.want_valid_transaction().then(|| {
							ValidTransaction::with_tag_prefix("BulletinReservedStore")
								.and_provides((*reservation_id, content_hash))
								.longevity(T::StoreRenewLongevity::get())
								.into()
						}),
						None,
					));
				},
				Call::<T>::renew_reserved { reservation_id, content_hash } => {
					Self::prepare_reserved_renew(who, *reservation_id, *content_hash)
						.map_err(|_| InvalidTransaction::Call)?;
					return Ok((
						context.want_valid_transaction().then(|| {
							ValidTransaction::with_tag_prefix("BulletinReservedRenew")
								.and_provides((*reservation_id, *content_hash))
								.longevity(T::StoreRenewLongevity::get())
								.into()
						}),
						None,
					));
				},
				Call::<T>::store { data } => {
					let content_hash = sp_io::hashing::blake2_256(data);
					(data.len(), content_hash, false)
				},
				Call::<T>::store_with_cid_config { cid, data } => {
					let content_hash = cid.hashing.hash(data);
					(data.len(), content_hash, false)
				},
				Call::<T>::force_renew { entry } => {
					let info =
						Self::resolve_transaction_ref(entry).map_err(|_| RENEWED_NOT_FOUND)?;
					(info.size as usize, info.content_hash, true)
				},
				Call::<T>::authorize_account { .. }
				| Call::<T>::authorize_preimage { .. }
				| Call::<T>::refresh_account_authorization { .. }
				| Call::<T>::refresh_preimage_authorization { .. } => {
					// Verify that the signer satisfies the Authorizer origin. Budget
					// consumption (for `AllowedAuthorizers` signers on `authorize_*`)
					// happens inside the dispatch body, not here.
					let origin = frame_system::RawOrigin::Signed(who.clone()).into();
					T::Authorizer::ensure_origin(origin)
						.map_err(|_| InvalidTransaction::BadSigner)?;
					return Ok((
						context.want_valid_transaction().then(|| ValidTransaction {
							priority: T::StoreRenewPriority::get(),
							longevity: T::StoreRenewLongevity::get(),
							..Default::default()
						}),
						None,
					));
				},
				Call::<T>::add_authorizer { .. } | Call::<T>::remove_authorizer { .. } => {
					// `AuthorizerRegistrarOrigin` is enforced at dispatch; pool validation is a
					// passthrough.
					return Ok((
						context.want_valid_transaction().then(ValidTransaction::default),
						None,
					));
				},
				Call::<T>::renew { entry } => {
					// Pre-paid one-shot: charges the same as `force_renew`. Cycle delivers
					// without re-charging (see `do_process_auto_renewals`). Reject
					// duplicates before charging — mirrors `enable_auto_renew` below.
					let info =
						Self::resolve_transaction_ref(entry).map_err(|_| RENEWED_NOT_FOUND)?;
					if AutoRenewals::<T>::contains_key(info.content_hash) {
						return Err(AUTO_RENEWAL_ALREADY_ENABLED.into());
					}
					Self::check_authorization(
						&AuthorizationScope::Account(who.clone()),
						info.size,
						context.consume_authorization(),
						true,
					)?;
					let scope = AuthorizationScope::Account(who.clone());
					return Ok((
						context.want_valid_transaction().then(|| {
							ValidTransaction::with_tag_prefix("TransactionStorageRenew")
								.and_provides((who.clone(), info.content_hash))
								.priority(T::StoreRenewPriority::get())
								.longevity(T::StoreRenewLongevity::get())
								.into()
						}),
						Some(scope),
					));
				},
				Call::<T>::enable_auto_renew { content_hash } => {
					// Pre-paid recurring registration. Mirrors one-shot `renew`'s
					// hard-cap charge: `bytes_permanent`, the chain-wide
					// `PermanentStorageUsed` counter, and one tx slot are all consumed
					// here. The first cycle then fires free in
					// `do_process_auto_renewals` (`paid = true` on the inserted
					// `RenewalData`); subsequent cycles charge per-cycle.
					if AutoRenewals::<T>::contains_key(*content_hash) {
						return Err(AUTO_RENEWAL_ALREADY_ENABLED.into());
					}
					let (block, index) = TransactionByContentHash::<T>::get(*content_hash)
						.ok_or(RENEWED_NOT_FOUND)?;
					let info = Self::transaction_info(block, index).ok_or(RENEWED_NOT_FOUND)?;

					Self::check_authorization(
						&AuthorizationScope::Account(who.clone()),
						info.size,
						context.consume_authorization(),
						true,
					)?;

					let scope = AuthorizationScope::Account(who.clone());
					return Ok((
						context.want_valid_transaction().then(|| {
							ValidTransaction::with_tag_prefix("TransactionStorageRenew")
								.and_provides((who.clone(), info.content_hash))
								.priority(T::StoreRenewPriority::get())
								.longevity(T::StoreRenewLongevity::get())
								.into()
						}),
						Some(scope),
					));
				},
				Call::<T>::disable_auto_renew { content_hash } => {
					// Feeless. Pool admission is gated on ownership AND on the prepaid
					// window — the registration must exist, `who` must be its owner,
					// and the next cycle must not be pre-paid (otherwise the owner
					// would be reclaiming a slot they've already charged against their
					// quota). `Some(scope)` triggers the origin rewrite to
					// `Origin::Authorized` expected by the dispatch's `ensure_authorized`.
					let renewal_data =
						AutoRenewals::<T>::get(content_hash).ok_or(AUTO_RENEWAL_NOT_ENABLED)?;
					if &renewal_data.account != who {
						return Err(NOT_AUTO_RENEWAL_OWNER.into());
					}
					if renewal_data.paid {
						return Err(CANNOT_DISABLE_PREPAID_AUTO_RENEWAL.into());
					}
					let scope = AuthorizationScope::Account(who.clone());
					return Ok((
						context.want_valid_transaction().then(|| ValidTransaction {
							priority: T::StoreRenewPriority::get(),
							longevity: T::StoreRenewLongevity::get(),
							..Default::default()
						}),
						Some(scope),
					));
				},
				_ => return Err(InvalidTransaction::Call.into()),
			};

			if !Self::data_size_ok(size) {
				return Err(BAD_DATA_SIZE.into());
			}

			if Self::block_transactions_full() {
				return Err(InvalidTransaction::ExhaustsResources.into());
			}

			// Prefer preimage authorization if available.
			// This allows anyone to store/renew pre-authorized content without consuming their
			// own account authorization.
			let consume = context.consume_authorization();

			let used_preimage_auth = Self::check_authorization(
				&AuthorizationScope::Preimage(content_hash),
				size as u32,
				consume,
				is_renew,
			)
			.is_ok();

			if !used_preimage_auth {
				Self::check_authorization(
					&AuthorizationScope::Account(who.clone()),
					size as u32,
					consume,
					is_renew,
				)?;
			}

			// Only build `ValidTransaction` metadata during pool validation, not block
			// execution. The tx tag/priority differs depending on whether preimage or account
			// authorization was used.
			let (valid_tx, scope) = if context.want_valid_transaction() {
				let (valid_tx, scope) = if used_preimage_auth {
					(
						Self::preimage_store_renew_valid_transaction(content_hash),
						AuthorizationScope::Preimage(content_hash),
					)
				} else {
					// Tag prefix differs per family so store and renew operations don't
					// dedup against each other in the pool.
					let prefix = if is_renew {
						"TransactionStorageRenew"
					} else {
						"TransactionStorageStore"
					};
					(
						ValidTransaction::with_tag_prefix(prefix)
							.and_provides((who, content_hash))
							.priority(T::StoreRenewPriority::get())
							.longevity(T::StoreRenewLongevity::get())
							.into(),
						AuthorizationScope::Account(who.clone()),
					)
				};
				(Some(valid_tx), Some(scope))
			} else {
				(None, None)
			};

			Ok((valid_tx, scope))
		}

		/// Verifies that the provided proof corresponds to a randomly selected chunk from a list of
		/// transactions.
		pub(crate) fn verify_chunk_proof(
			proof: TransactionStorageProof,
			random_hash: &[u8],
			infos: Vec<TransactionInfo>,
		) -> Result<(), Error<T>> {
			// Get the random chunk index - from all transactions in the block = [0..total_chunks).
			let total_chunks: ChunkIndex = TransactionInfo::total_chunks(&infos);
			ensure!(total_chunks != 0, Error::<T>::UnexpectedProof);
			let selected_block_chunk_index = random_chunk(random_hash, total_chunks);

			// Let's find the corresponding transaction and its "local" chunk index for "global"
			// `selected_block_chunk_index`.
			let (tx_info, tx_chunk_index) = {
				// Binary search for the transaction that owns this `selected_block_chunk_index`
				// chunk.
				let tx_index = infos
					.binary_search_by_key(&selected_block_chunk_index, |info| {
						// Each `info.block_chunks` is cumulative count,
						// so last chunk index = count - 1.
						info.block_chunks.saturating_sub(1)
					})
					.unwrap_or_else(|tx_index| tx_index);

				// Get the transaction and its local chunk index.
				let tx_info = infos.get(tx_index).ok_or(Error::<T>::MissingStateData)?;
				// We shouldn't reach this point; we rely on the fact that `fn store` does not allow
				// empty transactions. Without this check, it would fail anyway below with
				// `InvalidProof`.
				ensure!(!tx_info.block_chunks.is_zero(), Error::<T>::BadDataSize);

				// Convert a global chunk index into a transaction-local one.
				let tx_chunks = num_chunks(tx_info.size);
				let prev_chunks = tx_info.block_chunks - tx_chunks;
				let tx_chunk_index = selected_block_chunk_index - prev_chunks;

				(tx_info, tx_chunk_index)
			};

			// Verify the tx chunk proof.
			ensure!(
				sp_io::trie::blake2_256_verify_proof(
					tx_info.chunk_root,
					&proof.proof,
					&encode_index(tx_chunk_index),
					&proof.chunk,
					sp_runtime::StateVersion::V1,
				),
				Error::<T>::InvalidProof
			);

			Ok(())
		}

		/// Backs `#[pallet::feeless_if]` on `authorize_account` and
		/// `refresh_account_authorization`. Goes through `T::Authorizer::ensure_origin`
		/// so Root / XCM (`Ok(None)`) are not feeless via this flag.
		///
		/// Also requires the authorizer's budget to be active (not exhausted or
		/// expired). An inactive authorizer would fail downstream anyway; gating
		/// `feeless` on it prevents spamming free, failing dispatches.
		pub(crate) fn is_feeless_authorizer(origin: &OriginFor<T>) -> bool {
			let Ok(Some(ctx)) = T::Authorizer::ensure_origin(origin.clone()) else {
				return false;
			};
			if !ctx.feeless {
				return false;
			}
			AllowedAuthorizers::<T>::get(&ctx.authorizer)
				.is_some_and(|b| !b.is_inactive(Self::now()))
		}

		/// Atomically decrement `who`'s [`AllowedAuthorizers`] budget by
		/// `transactions` / `bytes`. Returns [`Error::InsufficientAuthorizerBudget`]
		/// when either axis would go negative; on failure the budget is unchanged.
		///
		/// A missing entry is a no-op: callers reach this only via [`authorize`]
		/// once `T::Authorizer::ensure_origin` has already accepted the dispatch,
		/// so `who` is necessarily an `AllowedAuthorizers` entry — but the storage
		/// could have been removed between the origin check and here.
		fn consume_authorizer_budget(
			who: &T::AccountId,
			transactions: u32,
			bytes: u64,
		) -> DispatchResult {
			AllowedAuthorizers::<T>::try_mutate(who, |maybe_budget| {
				let Some(budget) = maybe_budget else { return Ok(()) };
				budget
					.try_consume(transactions, bytes)
					.ok_or(Error::<T>::InsufficientAuthorizerBudget.into())
			})
		}
	}
}

impl<T: Config>
	TwoPhaseStorage<T::AccountId, ReservationId, T::ReservationPurpose, BlockNumberFor<T>>
	for Pallet<T>
{
	fn reserve(
		id: ReservationId,
		owner: &T::AccountId,
		purpose: &T::ReservationPurpose,
		bytes: u64,
		transactions: u32,
		expires_at: BlockNumberFor<T>,
	) -> DispatchResult {
		Self::reserve_resource_capacity(id, owner, purpose, bytes, transactions, expires_at)
	}

	fn cancel(owner: &T::AccountId, id: ReservationId) -> DispatchResult {
		Self::cancel_resource_capacity(owner, id)
	}

	fn expire_due(now: BlockNumberFor<T>, limit: u32) -> Result<Vec<ReservationId>, DispatchError> {
		Self::expire_due_resource_capacity(now, limit)
	}
}

pub mod extension;

#[cfg(any(test, feature = "try-runtime", feature = "runtime-benchmarks"))]
#[allow(dead_code)]
impl<T: Config> Pallet<T> {
	pub(crate) fn do_try_state(n: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
		ensure!(!Self::retention_period().is_zero(), "RetentionPeriod must not be zero");
		Self::check_transactions_integrity()?;
		Self::check_no_stale_transactions(n)?;
		Self::check_authorizations_integrity()?;
		Self::check_permanent_storage_accounting(n)?;
		Self::check_resource_reservation_integrity()?;
		Ok(())
	}

	/// Verify that for each block's transaction list:
	/// - The `block_chunks` field is cumulative: each entry equals the previous cumulative total
	///   plus `num_chunks(size)`.
	fn check_transactions_integrity() -> Result<(), sp_runtime::TryRuntimeError> {
		for (_block, transactions) in Transactions::<T>::iter() {
			let mut cumulative_chunks: ChunkIndex = 0;
			for tx in transactions.iter() {
				let expected_chunks = num_chunks(tx.size);
				cumulative_chunks = cumulative_chunks.saturating_add(expected_chunks);
				ensure!(tx.block_chunks == cumulative_chunks, "tx.block_chunks is not cumulative");
			}

			// The last entry's block_chunks should equal total_chunks for the block.
			let total = TransactionInfo::total_chunks(&transactions);
			ensure!(
				total == cumulative_chunks,
				"total_chunks mismatch with cumulative block_chunks"
			);
		}

		Ok(())
	}

	/// Verify that no `Transactions` entries exist for blocks older than
	/// `current_block - retention_period`. These should have been cleaned up
	/// by `on_initialize`.
	fn check_no_stale_transactions(
		n: BlockNumberFor<T>,
	) -> Result<(), sp_runtime::TryRuntimeError> {
		let period = Self::retention_period();
		let oldest_valid = n.saturating_sub(period);

		for (block, _) in Transactions::<T>::iter() {
			ensure!(block >= oldest_valid, "Stale transaction entry found beyond retention period");
			ensure!(block <= n, "Transaction entry exists for a future block");
		}

		Ok(())
	}

	/// Verify that stored authorizations have a non-zero `bytes_allowance` cap.
	/// The `bytes` (used) counter can exceed `bytes_allowance` — that just disables the
	/// priority boost, it does not remove the entry.
	fn check_authorizations_integrity() -> Result<(), sp_runtime::TryRuntimeError> {
		for (_, authorization) in Authorizations::<T>::iter() {
			ensure!(
				authorization.extent.bytes_allowance > 0,
				"Stored authorization has zero bytes_allowance"
			);
		}

		Ok(())
	}

	/// Verify the chain-wide permanent-storage accounting invariants:
	/// - `PermanentStorageUsed == Σ Renew sizes in Transactions + Σ paid AutoRenewals sizes` — the
	///   paid term covers the prepayment window between `renew` / `enable_auto_renew` charging the
	///   counter and `do_process_auto_renewals` writing the `Renew` entry.
	/// - `PermanentStorageUsed <= MaxPermanentStorageSize`.
	fn check_permanent_storage_accounting(
		_n: BlockNumberFor<T>,
	) -> Result<(), sp_runtime::TryRuntimeError> {
		let used = PermanentStorageUsed::<T>::get();

		let renewed_sum: u64 = Transactions::<T>::iter().fold(0u64, |acc, (_, entries)| {
			entries
				.iter()
				.filter(|t| matches!(t.kind, TransactionKind::Renew))
				.fold(acc, |inner, t| inner.saturating_add(t.size as u64))
		});
		let prepaid_sum: u64 =
			AutoRenewals::<T>::iter()
				.filter(|(_, data)| data.paid)
				.fold(0u64, |acc, (hash, _)| {
					let size = TransactionByContentHash::<T>::get(hash)
						.and_then(|(block, index)| Self::transaction_info(block, index))
						.map_or(0, |info| info.size as u64);
					acc.saturating_add(size)
				});
		ensure!(
			renewed_sum.saturating_add(prepaid_sum) == used,
			"PermanentStorageUsed != Σ renewed sizes + Σ paid auto-renewal sizes",
		);

		ensure!(
			used <= T::MaxPermanentStorageSize::get(),
			"PermanentStorageUsed exceeds MaxPermanentStorageSize",
		);

		Ok(())
	}

	fn check_resource_reservation_integrity() -> Result<(), sp_runtime::TryRuntimeError> {
		let active: Vec<_> = ResourceReservations::<T>::iter().collect();
		let tombstones: Vec<_> = ResourceReservationTombstones::<T>::iter().collect();
		ensure!(
			active.len().saturating_add(tombstones.len()) <= T::MaxReservations::get() as usize,
			"active plus tombstones exceeds MaxReservations"
		);
		ensure!(
			ResourceReservationRowCount::<T>::get() as usize
				== active.len().saturating_add(tombstones.len()),
			"reservation row counter mismatch"
		);
		let reserved_sum = active
			.iter()
			.fold(0u64, |sum, (_, reservation)| sum.saturating_add(reservation.bytes_remaining));
		ensure!(
			reserved_sum == ReservedPermanentCapacity::<T>::get(),
			"ReservedPermanentCapacity does not equal active reservation sum"
		);
		ensure!(
			PermanentStorageUsed::<T>::get().saturating_add(reserved_sum)
				<= T::MaxPermanentStorageSize::get(),
			"permanent used plus reserved capacity exceeds global cap"
		);

		let blocks = ResourceReservationExpiryBlocks::<T>::get();
		ensure!(blocks.windows(2).all(|pair| pair[0] < pair[1]), "expiry blocks not sorted unique");
		for block in blocks.iter() {
			let bucket = ResourceReservationExpiryBuckets::<T>::get(block);
			ensure!(!bucket.is_empty(), "expiry block has empty bucket");
			ensure!(
				bucket.windows(2).all(|pair| pair[0] < pair[1]),
				"expiry bucket not sorted unique"
			);
			for id in bucket {
				let reservation = ResourceReservations::<T>::get(id)
					.ok_or("expiry bucket references missing active reservation")?;
				ensure!(
					reservation.expires_at == *block,
					"reservation expiry does not match bucket"
				);
			}
		}
		for (id, reservation) in &active {
			ensure!(
				ResourceReservationExpiryBuckets::<T>::get(reservation.expires_at)
					.binary_search(id)
					.is_ok(),
				"active reservation absent from expiry index"
			);
		}

		let queue = TombstonePruneQueue::<T>::get();
		ensure!(queue.windows(2).all(|pair| pair[0] < pair[1]), "prune queue not sorted unique");
		ensure!(queue.len() == tombstones.len(), "prune queue/tombstone cardinality mismatch");
		for (id, _) in &tombstones {
			ensure!(queue.binary_search(id).is_ok(), "tombstone absent from prune queue");
		}
		let links: Vec<_> = ResourceReservationLinks::<T>::iter().collect();
		ensure!(
			links.len() <= T::MaxReservationLinks::get() as usize,
			"reservation links exceeds MaxReservationLinks"
		);
		ensure!(
			ResourceReservationLinkCount::<T>::get() as usize == links.len(),
			"reservation link counter mismatch"
		);
		for (id, hash, link) in links {
			ensure!(id == link.reservation_id && hash == link.content_hash, "link key mismatch");
			ensure!(
				ResourceLinkByRef::<T>::get(link.bulletin_ref) == Some((id, hash)),
				"link reverse pointer mismatch"
			);
			ensure!(
				ResourceLinkByContentHash::<T>::get(hash) == Some(id),
				"content-hash reverse pointer mismatch"
			);
		}
		for (reference, (id, hash)) in ResourceLinkByRef::<T>::iter() {
			ensure!(
				ResourceReservationLinks::<T>::get(id, hash)
					.is_some_and(|link| link.bulletin_ref == reference),
				"reverse pointer has no matching link"
			);
		}
		for (hash, id) in ResourceLinkByContentHash::<T>::iter() {
			ensure!(
				ResourceReservationLinks::<T>::contains_key(id, hash),
				"content-hash reverse pointer has no matching link"
			);
		}
		for (reference, _) in StoredBy::<T>::iter() {
			ensure!(
				Self::transaction_info(reference.block, reference.transaction_index).is_some()
					|| (reference.block == Self::now()
						&& BlockTransactions::<T>::get()
							.get(reference.transaction_index as usize)
							.is_some()),
				"provenance points to missing transaction"
			);
		}
		Ok(())
	}
}

/// Sanity-check that the runtime's weight/size configuration is consistent with
/// `MaxBlockTransactions` and `MaxTransactionSize`.
///
/// Verifies that the runtime's weight configuration, block length limits, and
/// `MaxBlockTransactions`/`MaxTransactionSize` constants are mutually consistent.
///
/// The available block weight accounts for:
/// - The `avg_block_initialization` margin that FRAME reserves from `max_total` for on_initialize
///   hooks (e.g. 5% for parachains, 10% for `with_sensible_defaults`).
/// - For parachains, the collator-side PoV cap: collators limit the actual PoV to a percentage of
///   `max_pov_size` to leave headroom for relay-chain state proof overhead. See
///   `cumulus/client/consensus/aura/src/collators/slot_based/block_builder_task.rs`.
///
/// # Parameters
///
/// - `collator_pov_percent`: for parachains, the collator-side PoV cap (e.g. `Some(85)`).
///   Solochains should pass `None`.
///
/// # Panics
///
/// Panics with a descriptive message if any check fails.
#[cfg(any(test, feature = "std"))]
pub fn ensure_weight_sanity<T: Config>(collator_pov_percent: Option<u64>) {
	use frame_support::{dispatch::DispatchClass, weights::Weight};

	let block_weights = <T as frame_system::Config>::BlockWeights::get();
	let normal_length =
		*<T as frame_system::Config>::BlockLength::get().max.get(DispatchClass::Normal);

	let max_block_txs = T::MaxBlockTransactions::get();
	let max_tx_size = T::MaxTransactionSize::get();

	let normal = block_weights.get(DispatchClass::Normal);
	let normal_max_total = normal.max_total.expect("Normal class must have a max_total weight");
	let base_extrinsic = normal.base_extrinsic;
	let max_extrinsic =
		normal.max_extrinsic.expect("Normal class must have a max_extrinsic weight");

	// init_weight = max_total - max_extrinsic - base_extrinsic (the avg_block_initialization
	// reservation that FRAME sets aside for on_initialize hooks).
	let init_weight = normal_max_total.saturating_sub(max_extrinsic).saturating_sub(base_extrinsic);

	let after_init = normal_max_total.saturating_sub(init_weight);
	let effective_normal = if let Some(pov_percent) = collator_pov_percent {
		// Collators cap the PoV to reserve headroom for the relay-chain state proof.
		// Reference: cumulus/client/consensus/aura/src/collators/lookahead.rs
		let pov_limit = block_weights.max_block.proof_size() * pov_percent / 100;
		Weight::from_parts(after_init.ref_time(), after_init.proof_size().min(pov_limit))
	} else {
		after_init
	};

	// 1. MaxTransactionSize must fit within the normal block length limit.
	assert!(
		max_tx_size < normal_length,
		"MaxTransactionSize ({max_tx_size}) >= normal block length ({normal_length}): \
		 a single max-size store extrinsic wouldn't fit by length",
	);

	// 2. A single store(MaxTransactionSize) must fit within max_extrinsic.
	let max_store_dispatch = T::WeightInfo::store(max_tx_size);
	assert!(
		max_store_dispatch.all_lte(max_extrinsic),
		"store({max_tx_size}) dispatch weight {max_store_dispatch:?} exceeds \
		 max_extrinsic {max_extrinsic:?} (which accounts for init overhead + base)",
	);

	// 3. MaxBlockTransactions store calls at an evenly-split size must fit in the effective normal
	//    budget (ref_time). Each extrinsic costs dispatch + base.
	let per_tx_size = normal_length / max_block_txs;
	let store_weight = T::WeightInfo::store(per_tx_size).saturating_add(base_extrinsic);
	let total_store_ref_time = store_weight.ref_time().saturating_mul(max_block_txs as u64);
	assert!(
		total_store_ref_time <= effective_normal.ref_time(),
		"MaxBlockTransactions ({max_block_txs}) store calls at {per_tx_size} bytes each: \
		 total ref_time {total_store_ref_time} exceeds effective normal limit {} \
		 (max_total {} minus init reservation {})",
		effective_normal.ref_time(),
		normal_max_total.ref_time(),
		init_weight.ref_time(),
	);

	// 4. MaxBlockTransactions renew calls must fit by ref_time.
	let renew_weight = T::WeightInfo::renew().saturating_add(base_extrinsic);
	let total_renew_ref_time = renew_weight.ref_time().saturating_mul(max_block_txs as u64);
	assert!(
		total_renew_ref_time <= effective_normal.ref_time(),
		"MaxBlockTransactions ({max_block_txs}) renew calls: \
		 total ref_time {total_renew_ref_time} exceeds effective normal limit {}",
		effective_normal.ref_time(),
	);

	// 5. apply_block_inherents (DispatchClass::Mandatory, once per block) must fit
	// in max block at worst case (proof check + draining MaxBlockTransactions
	// auto-renewals).
	let apply_inherents_weight = T::WeightInfo::apply_block_inherents(max_block_txs);
	assert!(
		apply_inherents_weight.all_lte(block_weights.max_block),
		"apply_block_inherents weight {apply_inherents_weight:?} exceeds max block {:?}",
		block_weights.max_block,
	);

	// 6. on_initialize at the worst-case expiry block (taking
	// `MaxBlockTransactions` items out of `Transactions[obsolete]` and looking up
	// `TransactionByContentHash` + `AutoRenewals` for each) must fit alongside
	// `apply_block_inherents` within `max_block`. Both run on the same block in
	// the mandatory class; their sum is the floor of the mandatory budget for
	// that block.
	let on_init_with_expiry_weight = T::WeightInfo::on_initialize_with_expiry(max_block_txs);
	let mandatory_floor = on_init_with_expiry_weight.saturating_add(apply_inherents_weight);
	assert!(
		mandatory_floor.all_lte(block_weights.max_block),
		"on_initialize_with_expiry({max_block_txs}) + apply_block_inherents({max_block_txs}) \
		 = {mandatory_floor:?} exceeds max block {:?}",
		block_weights.max_block,
	);

	// Diagnostics (visible with --nocapture).
	let max_txs_by_weight = effective_normal.ref_time() / store_weight.ref_time();
	println!("--- transaction_storage weight sanity ---");
	println!("  MaxBlockTransactions:       {max_block_txs}");
	println!(
		"  MaxTransactionSize:         {max_tx_size} bytes ({} MiB)",
		max_tx_size / (1024 * 1024)
	);
	println!("  Normal max_total:           {normal_max_total:?}");
	println!("  Init reservation:           {init_weight:?}");
	if let Some(pov_percent) = collator_pov_percent {
		let pov_limit = block_weights.max_block.proof_size() * pov_percent / 100;
		println!(
			"  Collator PoV cap ({pov_percent}%):      {pov_limit} bytes ({:.1} MiB)",
			pov_limit as f64 / (1024.0 * 1024.0)
		);
	}
	println!("  Effective normal budget:    {effective_normal:?}");
	println!("  max_extrinsic:              {max_extrinsic:?}");
	println!(
		"  Normal length limit:        {normal_length} bytes ({} MiB)",
		normal_length / (1024 * 1024)
	);
	println!("  store(max_size) weight:     {max_store_dispatch:?}");
	println!("  store(even_split) weight:   {store_weight:?} (at {per_tx_size} bytes)");
	println!("  renew weight:               {renew_weight:?}");
	println!("  block_weights.max_block:    {:?}", block_weights.max_block);
	println!("  apply_block_inherents wt:   {apply_inherents_weight:?}");
	println!("  on_initialize_with_expiry:  {on_init_with_expiry_weight:?}");
	println!("  Mandatory floor (sum):      {mandatory_floor:?}");
	println!("  Max store txs by weight:    {max_txs_by_weight}");
	println!("  Max store txs by length:    {}", normal_length / per_tx_size);
}
