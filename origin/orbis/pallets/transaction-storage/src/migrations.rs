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

use crate::{Config, RetentionPeriod, LOG_TARGET};
use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use polkadot_sdk_frame::{
	prelude::{BlockNumberFor, Weight},
	traits::{Get, OnRuntimeUpgrade, Zero},
};

/// Runtime migration that sets the `RetentionPeriod` storage item to a
/// non-zero `NewValue` value **only if it is currently zero**.
///
/// Idempotent migration: safe to run multiple times
pub struct SetRetentionPeriodIfZero<T, NewValue>(PhantomData<(T, NewValue)>);
impl<T: Config, NewValue: Get<BlockNumberFor<T>>> OnRuntimeUpgrade
	for SetRetentionPeriodIfZero<T, NewValue>
{
	fn on_runtime_upgrade() -> Weight {
		let mut weight = T::DbWeight::get().reads(1);

		// If zero, let's reset.
		if RetentionPeriod::<T>::get().is_zero() {
			RetentionPeriod::<T>::set(NewValue::get());
			weight.saturating_accrue(T::DbWeight::get().writes(1));

			tracing::warn!(
				target: LOG_TARGET,
				new_value = ?NewValue::get(),
				"[SetRetentionPeriodIfZero] RetentionPeriod was zero, resetting to:",
			);
		}

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(
		_state: alloc::vec::Vec<u8>,
	) -> Result<(), polkadot_sdk_frame::deps::sp_runtime::DispatchError> {
		polkadot_sdk_frame::prelude::ensure!(
			!RetentionPeriod::<T>::get().is_zero(),
			"must be migrate to the `NewValue`."
		);

		tracing::info!(target: LOG_TARGET, "SetRetentionPeriodIfZero is OK!");
		Ok(())
	}
}

/// Migration v0→v1: Adds `hashing` and `cid_codec` fields to `TransactionInfo`.
///
/// Handles mixed-format storage safely: the chain was upgraded without migration,
/// so `Transactions` contains both old-format (pre-CID) and new-format (post-CID)
/// entries. Uses raw storage iteration with try-new-then-old decoding to avoid
/// corrupting post-upgrade entries.
///
/// Old entries get defaults: `hashing = Blake2b256`, `cid_codec = RAW_CODEC`.
pub mod v1 {
	use super::*;
	use crate::{
		pallet::{Pallet, Transactions},
		TransactionInfo,
	};
	use bulletin_transaction_storage_primitives::{
		cids::{CidCodec, HashingAlgorithm, RAW_CODEC},
		ContentHash,
	};
	use polkadot_sdk_frame::deps::{
		frame_support::{
			migrations::VersionedMigration,
			storage::{unhashed, StoragePrefixedMap},
			traits::UncheckedOnRuntimeUpgrade,
			BoundedVec,
		},
		sp_io,
		sp_runtime::traits::{BlakeTwo256, Hash},
	};
	use sp_transaction_storage_proof::ChunkIndex;

	/// `TransactionInfo` layout before v1 (no CID fields).
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct OldTransactionInfo {
		pub chunk_root: <BlakeTwo256 as Hash>::Output,
		pub content_hash: <BlakeTwo256 as Hash>::Output,
		pub size: u32,
		pub block_chunks: ChunkIndex,
	}

	/// `TransactionInfo` layout at v1 (mandatory `hashing` and `cid_codec`).
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct V1TransactionInfo {
		pub chunk_root: <BlakeTwo256 as Hash>::Output,
		pub content_hash: ContentHash,
		pub hashing: HashingAlgorithm,
		pub cid_codec: CidCodec,
		pub size: u32,
		pub block_chunks: ChunkIndex,
	}

	/// Version-unchecked migration logic. Wrapped by [`MigrateV0ToV1`] for version gating.
	pub struct VersionUncheckedMigrateV0ToV1<T>(PhantomData<T>);

	impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV0ToV1<T> {
		/// NOTE: This iterates all `Transactions` entries without an upper bound.
		/// The entry count is bounded by `RetentionPeriod` (one per block number).
		/// At the time of deployment the live chain has 126 entries, well within
		/// a single block's weight and PoV limits. If the entry count were ever
		/// close to `RetentionPeriod` (100,800), this would need to be converted
		/// to a multi-block migration.
		fn on_runtime_upgrade() -> Weight {
			let prefix = Transactions::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut migrated: u64 = 0;
			let mut skipped: u64 = 0;
			let mut corrupted: u64 = 0;

			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key.clone();
				let Some(raw) = unhashed::get_raw(&key) else { continue };

				// Try decode as current type first — if it works, the entry is
				// already post-upgrade. Old format (72 bytes/entry) always fails
				// here because the decoder runs out of bytes.
				if BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::decode(&mut &raw[..])
					.is_ok()
				{
					skipped += 1;
					continue;
				}

				// Fall back to old type and transform.
				match BoundedVec::<OldTransactionInfo, T::MaxBlockTransactions>::decode(
					&mut &raw[..],
				) {
					Ok(old_txs) => {
						let new_txs: Vec<V1TransactionInfo> = old_txs
							.into_iter()
							.map(|old| V1TransactionInfo {
								chunk_root: old.chunk_root,
								content_hash: old.content_hash.into(),
								hashing: HashingAlgorithm::Blake2b256,
								cid_codec: RAW_CODEC,
								size: old.size,
								block_chunks: old.block_chunks,
							})
							.collect();
						let Ok(bounded) =
							BoundedVec::<V1TransactionInfo, T::MaxBlockTransactions>::try_from(
								new_txs,
							)
						else {
							// Unreachable: decoded N items from a BoundedVec with the same
							// bound, mapped 1:1. Log defensively and skip.
							polkadot_sdk_frame::deps::frame_support::defensive!(
								"v0->v1: BoundedVec conversion failed"
							);
							continue;
						};
						unhashed::put_raw(&key, &bounded.encode());
						migrated += 1;
					},
					Err(_) => {
						// Corrupted entry — remove to prevent on_finalize panic.
						unhashed::kill(&key);
						corrupted += 1;
						tracing::warn!(
							target: LOG_TARGET,
							"Removed corrupted Transactions entry during v0->v1 migration",
						);
					},
				}
			}

			tracing::info!(
				target: LOG_TARGET,
				migrated,
				skipped,
				corrupted,
				"v0->v1 TransactionInfo migration complete",
			);

			let entries = migrated + skipped + corrupted;
			// 2 reads per entry (next_key + get_raw), 1 for the final next_key returning None.
			// 1 write per migrated (put_raw) or corrupted (kill) entry; skipped = 0 writes.
			T::DbWeight::get()
				.reads(entries.saturating_mul(2).saturating_add(1))
				.saturating_add(T::DbWeight::get().writes(migrated + corrupted))
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			let prefix = Transactions::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut count: u64 = 0;
			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key;
				count += 1;
			}
			tracing::info!(target: LOG_TARGET, count, "pre_upgrade: Transactions entries");
			Ok(count.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(
			state: Vec<u8>,
		) -> Result<(), polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			let old_count =
				u64::decode(&mut &state[..]).map_err(|_| "Failed to decode pre_upgrade state")?;
			// iter() decodes every entry — if any fail, they are skipped and the
			// count drops, which the check below will catch.
			let new_count = Transactions::<T>::iter().count() as u64;
			polkadot_sdk_frame::prelude::ensure!(
				new_count <= old_count,
				"post_upgrade: more entries than before migration"
			);
			tracing::info!(target: LOG_TARGET, old_count, new_count, "post_upgrade: valid");
			Ok(())
		}
	}

	/// Versioned migration v0→v1: adds `hashing` and `cid_codec` to `TransactionInfo`.
	/// Safe for mixed old/new format storage.
	pub type MigrateV0ToV1<T> = VersionedMigration<
		0,
		1,
		VersionUncheckedMigrateV0ToV1<T>,
		Pallet<T>,
		<T as polkadot_sdk_frame::deps::frame_system::Config>::DbWeight,
	>;

	/// Run the v0→v1 `TransactionInfo` migration if the on-chain storage version
	/// is still 0. This covers the `codeSubstitutes` recovery path where the fix
	/// runtime is loaded without triggering `on_runtime_upgrade`.
	///
	/// Returns the weight consumed. On subsequent blocks (version already ≥ 1)
	/// this is a single storage read.
	pub fn maybe_migrate_v0_to_v1<T: Config>() -> Weight {
		use polkadot_sdk_frame::prelude::{GetStorageVersion, StorageVersion};

		let on_chain = Pallet::<T>::on_chain_storage_version();
		if on_chain >= 1 {
			return T::DbWeight::get().reads(1);
		}

		tracing::info!(
			target: LOG_TARGET,
			?on_chain,
			"Running v0→v1 TransactionInfo migration from on_initialize",
		);

		let migration_weight = VersionUncheckedMigrateV0ToV1::<T>::on_runtime_upgrade();

		StorageVersion::new(1).put::<Pallet<T>>();

		// 1 read (version check) + migration weight + 1 write (version bump)
		T::DbWeight::get()
			.reads(1)
			.saturating_add(migration_weight)
			.saturating_add(T::DbWeight::get().writes(1))
	}
}

/// Migration v1→v2: rewrites `AuthorizationExtent` from `{ transactions, bytes }` to
/// `{ transactions, transactions_allowance, bytes, bytes_permanent, bytes_allowance }`.
/// The old remaining quota becomes the new allowance; consumed counters reset to `0`.
/// Entries with `bytes == 0` are dropped (already unusable; `bytes_allowance > 0` is a
/// v2 invariant).
///
/// `Transactions` is intentionally left at the v1 shape — the stepped `v2→v3` migration
/// decodes it via `V2TransactionInfo` (byte-identical) and converts entries to the
/// current layout in bounded per-block steps. `BlockTransactions` is still translated
/// here defensively (single transient `StorageValue`).
pub mod v2 {
	use super::*;
	use crate::{
		pallet::{Authorizations, BlockTransactions, Pallet},
		Authorization, AuthorizationExtent, TransactionInfo, TransactionKind,
	};
	use bulletin_transaction_storage_primitives::{
		cids::{CidCodec, HashingAlgorithm},
		ContentHash,
	};
	use polkadot_sdk_frame::deps::{
		frame_support::{
			migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade, BoundedVec,
		},
		sp_runtime::traits::{BlakeTwo256, Hash},
	};
	use sp_transaction_storage_proof::ChunkIndex;

	#[derive(Encode, Decode)]
	pub(crate) struct V1AuthorizationExtent {
		pub transactions: u32,
		pub bytes: u64,
	}

	#[derive(Encode, Decode)]
	pub(crate) struct V1Authorization<BlockNumber> {
		pub extent: V1AuthorizationExtent,
		pub expiration: BlockNumber,
	}

	impl<BlockNumber: PartialOrd + Copy> V1Authorization<BlockNumber> {
		fn expired(&self, now: BlockNumber) -> bool {
			now >= self.expiration
		}
	}

	/// `Authorization` layout v1→v2 actually produces — frozen. The `From` impl
	/// below maps it into the current `Authorization` struct; any future field
	/// added to `Authorization` becomes a compile error here, forcing the author
	/// to pick an explicit placeholder rather than silently writing the new shape
	/// from this old migration.
	pub(crate) struct V2Authorization<BlockNumber> {
		pub extent: AuthorizationExtent,
		pub expiration: BlockNumber,
	}

	impl<BlockNumber> From<V2Authorization<BlockNumber>> for Authorization<BlockNumber> {
		fn from(v2: V2Authorization<BlockNumber>) -> Self {
			Self { extent: v2.extent, expiration: v2.expiration }
		}
	}

	/// `TransactionInfo` layout at v1 — same shape as v2 minus the trailing `kind`
	/// field. Used here to decode existing entries during translation.
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct V1TransactionInfo {
		pub chunk_root: <BlakeTwo256 as Hash>::Output,
		pub content_hash: ContentHash,
		pub hashing: HashingAlgorithm,
		pub cid_codec: CidCodec,
		pub size: u32,
		pub block_chunks: ChunkIndex,
	}

	pub struct VersionUncheckedMigrateV1ToV2<T>(PhantomData<T>);

	impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV1ToV2<T> {
		fn on_runtime_upgrade() -> Weight {
			let mut auth_migrated: u64 = 0;
			let mut auth_dropped: u64 = 0;
			let now = Pallet::<T>::now();
			Authorizations::<T>::translate::<V1Authorization<BlockNumberFor<T>>, _>(
				|_scope, old| {
					if old.extent.bytes == 0 || old.expired(now) {
						auth_dropped = auth_dropped.saturating_add(1);
						return None;
					}
					auth_migrated = auth_migrated.saturating_add(1);
					Some(
						V2Authorization {
							extent: AuthorizationExtent {
								bytes: 0,
								bytes_permanent: 0,
								bytes_allowance: old.extent.bytes,
								transactions: 0,
								transactions_allowance: old.extent.transactions,
							},
							expiration: old.expiration,
						}
						.into(),
					)
				},
			);
			tracing::info!(
				target: LOG_TARGET,
				migrated = auth_migrated,
				dropped = auth_dropped,
				"v1->v2 AuthorizationExtent migration complete",
			);

			// `Transactions` is intentionally not rewritten here — `v2→v3` decodes v1
			// entries via its `V2TransactionInfo` path (same SCALE shape) and converts
			// them in bounded per-block steps. See the module-level doc comment.

			// `BlockTransactions` is transient (cleared in `on_finalize`) so it's
			// almost always empty between blocks, but translate defensively in case
			// the upgrade lands mid-block.
			let mut block_tx_present = 0u64;
			let _ = BlockTransactions::<T>::translate::<
				BoundedVec<V1TransactionInfo, T::MaxBlockTransactions>,
				_,
			>(|maybe_old| {
				let old_vec = maybe_old?;
				block_tx_present = 1;
				let new_vec: alloc::vec::Vec<TransactionInfo> = old_vec
					.into_iter()
					.map(|old| TransactionInfo {
						chunk_root: old.chunk_root,
						content_hash: old.content_hash,
						hashing: old.hashing,
						cid_codec: old.cid_codec,
						size: old.size,
						extrinsic_index: u32::MAX,
						block_chunks: old.block_chunks,
						kind: TransactionKind::Store,
					})
					.collect();
				Some(
					BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::try_from(new_vec)
						.expect("v1->v2: vec re-bounded with same size; qed"),
				)
			});

			tracing::info!(
				target: LOG_TARGET,
				block_transactions_present = block_tx_present,
				"v1->v2 BlockTransactions migration complete",
			);

			let auth_touched = auth_migrated.saturating_add(auth_dropped);
			T::DbWeight::get().reads_writes(
				auth_touched.saturating_add(block_tx_present),
				auth_touched.saturating_add(block_tx_present),
			)
		}
	}

	/// Versioned migration v1→v2: rewrites `AuthorizationExtent` and `TransactionInfo`.
	pub type MigrateV1ToV2<T> = VersionedMigration<
		1,
		2,
		VersionUncheckedMigrateV1ToV2<T>,
		Pallet<T>,
		<T as polkadot_sdk_frame::deps::frame_system::Config>::DbWeight,
	>;
}

/// Migration v2→v3: Adds `extrinsic_index` to `TransactionInfo`.
///
/// Also opportunistically prunes any `Transactions[block]` entries with
/// `block < current_block - RetentionPeriod` — stale leftovers from chains where the
/// retention window was previously longer than it is now (`on_initialize`'s aging-out
/// hook only drops one entry per block going forward; it does not catch up on
/// historical entries that became stale across a retention-period change).
pub mod v3 {
	use super::*;
	use crate::{
		pallet::{Pallet, Transactions},
		RetentionPeriod, TransactionInfo, TransactionKind, WeightInfo,
	};
	use bulletin_transaction_storage_primitives::{
		cids::{CidCodec, HashingAlgorithm},
		ContentHash,
	};
	use polkadot_sdk_frame::deps::{
		frame_support::{
			migrations::{MigrationId, SteppedMigration, SteppedMigrationError},
			weights::WeightMeter,
			BoundedVec,
		},
		sp_io,
		sp_runtime::traits::{BlakeTwo256, Hash},
	};
	use sp_transaction_storage_proof::ChunkIndex;

	const MIGRATIONS_ID: &[u8; 24] = b"bulletin-tx-storage-vmig";

	/// `TransactionInfo` layout at v2 (no `extrinsic_index`). Used only for
	/// decoding pre-migration entries; never written.
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct V2TransactionInfo {
		pub chunk_root: <BlakeTwo256 as Hash>::Output,
		pub content_hash: ContentHash,
		pub hashing: HashingAlgorithm,
		pub cid_codec: CidCodec,
		pub size: u32,
		pub block_chunks: ChunkIndex,
	}

	/// Stepped migration from storage version 2 to 3.
	pub struct MigrateV2ToV3<T: Config>(PhantomData<T>);

	impl<T: Config> SteppedMigration for MigrateV2ToV3<T> {
		type Cursor = polkadot_sdk_frame::prelude::BlockNumberFor<T>;
		type Identifier = MigrationId<24>;

		fn id() -> Self::Identifier {
			MigrationId { pallet_id: *MIGRATIONS_ID, version_from: 2, version_to: 3 }
		}

		fn step(
			mut cursor: Option<Self::Cursor>,
			meter: &mut WeightMeter,
		) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
			use polkadot_sdk_frame::prelude::Saturating;

			let required = T::WeightInfo::migrate_v2_to_v3_step();
			if meter.remaining().any_lt(required) {
				return Err(SteppedMigrationError::InsufficientWeight { required });
			}

			let oldest_valid = Pallet::<T>::now().saturating_sub(RetentionPeriod::<T>::get());

			loop {
				if meter.try_consume(required).is_err() {
					break;
				}

				let mut iter = match cursor.as_ref() {
					None => Transactions::<T>::iter_keys(),
					Some(last) =>
						Transactions::<T>::iter_keys_from(Transactions::<T>::hashed_key_for(last)),
				};

				let Some(block_number) = iter.next() else {
					// Never downgrade — this MBM can be re-run against state already
					// at/beyond v3 (e.g. by try-runtime, whose id isn't in `Historic`).
					use polkadot_sdk_frame::prelude::{GetStorageVersion, StorageVersion};
					if Pallet::<T>::on_chain_storage_version() < 3 {
						StorageVersion::new(3).put::<Pallet<T>>();
					}
					cursor = None;
					break;
				};

				let raw_key = Transactions::<T>::hashed_key_for(block_number);

				// Stale leftovers from a previously-longer retention window: drop
				// instead of converting. `on_initialize`'s aging-out only catches
				// up one block at a time, so historical stale entries linger
				// forever otherwise.
				if block_number < oldest_valid {
					sp_io::storage::clear(&raw_key);
					cursor = Some(block_number);
					continue;
				}

				let Some(raw) = sp_io::storage::get(&raw_key) else {
					cursor = Some(block_number);
					continue;
				};

				if BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::decode(&mut &raw[..])
					.is_ok()
				{
					cursor = Some(block_number);
					continue;
				}

				let v2 =
					BoundedVec::<V2TransactionInfo, T::MaxBlockTransactions>::decode(&mut &raw[..])
						.map_err(|_| SteppedMigrationError::Failed)?;

				let v3: BoundedVec<TransactionInfo, T::MaxBlockTransactions> = v2
					.into_iter()
					.map(|old| TransactionInfo {
						chunk_root: old.chunk_root,
						content_hash: old.content_hash,
						hashing: old.hashing,
						cid_codec: old.cid_codec,
						size: old.size,
						extrinsic_index: u32::MAX,
						block_chunks: old.block_chunks,
						kind: TransactionKind::Store,
					})
					.collect::<Vec<_>>()
					.try_into()
					.map_err(|_| SteppedMigrationError::Failed)?;

				Transactions::<T>::insert(block_number, v3);
				cursor = Some(block_number);
			}

			Ok(cursor)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			use polkadot_sdk_frame::deps::frame_support::storage::StoragePrefixedMap;
			let prefix = Transactions::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut count: u64 = 0;
			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key;
				count += 1;
			}
			tracing::info!(target: LOG_TARGET, count, "v2->v3 pre_upgrade: Transactions entries");
			Ok(count.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(
			state: Vec<u8>,
		) -> Result<(), polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			use polkadot_sdk_frame::deps::frame_support::storage::StoragePrefixedMap;

			let old_count =
				u64::decode(&mut &state[..]).map_err(|_| "Failed to decode pre_upgrade state")?;

			let prefix = Transactions::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut new_count: u64 = 0;
			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key.clone();
				let raw = sp_io::storage::get(&key)
					.ok_or("v2->v3 post_upgrade: missing Transactions entry")?;
				BoundedVec::<TransactionInfo, T::MaxBlockTransactions>::decode(&mut &raw[..])
					.map_err(|_| "v2->v3 post_upgrade: remaining entry is not v3")?;
				new_count += 1;
			}

			polkadot_sdk_frame::prelude::ensure!(
				new_count <= old_count,
				"v2->v3 post_upgrade: entry count increased"
			);
			tracing::info!(
				target: LOG_TARGET,
				old_count,
				new_count,
				pruned = old_count.saturating_sub(new_count),
				"v2->v3 post_upgrade: valid"
			);
			Ok(())
		}
	}
}

/// V3 → V4 migration: re-encode each [`AutoRenewals`] entry from
/// `{ account }` (v3) to `{ account, recurring: true, paid: false }` (v4).
///
/// All existing entries were written by the old fee-paying `enable_auto_renew`,
/// which:
///
/// - is the forever-renewal path, so the entries map to `recurring: true`;
/// - did **not** pre-pay the next cycle against the owner's authorization, so they map to `paid:
///   false` — `do_process_auto_renewals` will charge them per-cycle, preserving their on-chain
///   behaviour across the upgrade.
///
/// The new one-shot path (`recurring: false`) and the new prepaid path
/// (`paid: true`, set by both `renew` and the new `enable_auto_renew`) are only
/// reachable through the v4 extrinsics, which can't have written any entries
/// before this migration runs.
pub mod v4 {
	use super::*;
	use crate::{
		pallet::{AutoRenewals, Pallet},
		RenewalData, WeightInfo,
	};
	use bulletin_transaction_storage_primitives::ContentHash;
	use polkadot_sdk_frame::deps::{
		frame_support::{
			migrations::{MigrationId, SteppedMigration, SteppedMigrationError},
			weights::WeightMeter,
		},
		sp_io,
	};

	const MIGRATIONS_ID: &[u8; 24] = b"bulletin-tx-storage-vmig";

	/// `AutoRenewalData` layout at v3 (no `recurring` field). Used only for
	/// decoding pre-migration entries; never written.
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct V3AutoRenewalData<AccountId> {
		pub account: AccountId,
	}

	/// Stepped migration from storage version 3 to 4.
	pub struct MigrateV3ToV4<T: Config>(PhantomData<T>);

	impl<T: Config> SteppedMigration for MigrateV3ToV4<T> {
		type Cursor = ContentHash;
		type Identifier = MigrationId<24>;

		fn id() -> Self::Identifier {
			MigrationId { pallet_id: *MIGRATIONS_ID, version_from: 3, version_to: 4 }
		}

		fn step(
			mut cursor: Option<Self::Cursor>,
			meter: &mut WeightMeter,
		) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
			let required = T::WeightInfo::migrate_v3_to_v4_step();
			if meter.remaining().any_lt(required) {
				return Err(SteppedMigrationError::InsufficientWeight { required });
			}

			loop {
				if meter.try_consume(required).is_err() {
					break;
				}

				let mut iter = match cursor.as_ref() {
					None => AutoRenewals::<T>::iter_keys(),
					Some(last) =>
						AutoRenewals::<T>::iter_keys_from(AutoRenewals::<T>::hashed_key_for(last)),
				};

				let Some(content_hash) = iter.next() else {
					// Never downgrade — this MBM can be re-run against state already
					// at/beyond v4 (e.g. by try-runtime, whose id isn't in `Historic`).
					use polkadot_sdk_frame::prelude::{GetStorageVersion, StorageVersion};
					if Pallet::<T>::on_chain_storage_version() < 4 {
						StorageVersion::new(4).put::<Pallet<T>>();
					}
					cursor = None;
					break;
				};

				let raw_key = AutoRenewals::<T>::hashed_key_for(content_hash);

				let Some(raw) = sp_io::storage::get(&raw_key) else {
					cursor = Some(content_hash);
					continue;
				};

				// Idempotent: if it's already v4, skip.
				if RenewalData::<T::AccountId>::decode(&mut &raw[..]).is_ok() {
					cursor = Some(content_hash);
					continue;
				}

				let v3 = V3AutoRenewalData::<T::AccountId>::decode(&mut &raw[..])
					.map_err(|_| SteppedMigrationError::Failed)?;

				AutoRenewals::<T>::insert(
					content_hash,
					RenewalData { account: v3.account, recurring: true, paid: false },
				);
				cursor = Some(content_hash);
			}

			Ok(cursor)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			use polkadot_sdk_frame::deps::frame_support::storage::StoragePrefixedMap;
			let prefix = AutoRenewals::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut count: u64 = 0;
			// `step` only converts entries still in the v3 layout; pre-existing v4 entries
			// (e.g. prepaid `paid=true`) are left untouched, so `post_upgrade` asserts only
			// on the converted ones.
			let mut to_migrate: Vec<Vec<u8>> = Vec::new();
			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key.clone();
				count += 1;
				let raw = sp_io::storage::get(&key)
					.ok_or("v3->v4 pre_upgrade: missing AutoRenewals entry")?;
				if RenewalData::<T::AccountId>::decode(&mut &raw[..]).is_err() {
					to_migrate.push(key);
				}
			}
			tracing::info!(
				target: LOG_TARGET,
				count,
				to_migrate = to_migrate.len(),
				"v3->v4 pre_upgrade: AutoRenewals entries"
			);
			Ok((count, to_migrate).encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(
			state: Vec<u8>,
		) -> Result<(), polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			use polkadot_sdk_frame::deps::frame_support::storage::StoragePrefixedMap;

			let (old_count, to_migrate) = <(u64, Vec<Vec<u8>>)>::decode(&mut &state[..])
				.map_err(|_| "Failed to decode pre_upgrade state")?;

			// Each converted entry must now be v4, recurring, and unpaid.
			for key in &to_migrate {
				let raw = sp_io::storage::get(key)
					.ok_or("v3->v4 post_upgrade: migrated entry missing")?;
				let decoded = RenewalData::<T::AccountId>::decode(&mut &raw[..])
					.map_err(|_| "v3->v4 post_upgrade: migrated entry is not v4")?;
				polkadot_sdk_frame::prelude::ensure!(
					decoded.recurring,
					"v3->v4 post_upgrade: migrated entry must have recurring=true",
				);
				polkadot_sdk_frame::prelude::ensure!(
					!decoded.paid,
					"v3->v4 post_upgrade: migrated entry must have paid=false",
				);
			}

			let prefix = AutoRenewals::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut new_count: u64 = 0;
			while let Some(key) =
				sp_io::storage::next_key(&previous_key).filter(|k| k.starts_with(&prefix))
			{
				previous_key = key;
				new_count += 1;
			}

			polkadot_sdk_frame::prelude::ensure!(
				new_count == old_count,
				"v3->v4 post_upgrade: entry count changed",
			);
			tracing::info!(
				target: LOG_TARGET,
				old_count,
				new_count,
				migrated = to_migrate.len(),
				"v3->v4 post_upgrade: valid"
			);
			Ok(())
		}
	}
}

/// V4 → V5 migration for `AllowedAuthorizers`.
///
/// `AuthorizerBudget` went from `{ quota, authorization_period, valid_until }` to
/// `{ quota, valid_until, feeless }`. Without translating, an existing
/// `authorization_period: Some(p)` would silently SCALE-decode as `valid_until: p`,
/// corrupting both fields. Existing entries default to `feeless: true` to match
/// the new genesis default.
///
/// Single-block: `AllowedAuthorizers` is an admin allow-list (single-digit count).
pub mod v5 {
	use super::*;
	use crate::{
		pallet::{AllowedAuthorizers, Pallet},
		AuthorizerBudget, Quota,
	};
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade,
	};

	/// `AuthorizerBudget` layout at v4 (before removing `authorization_period`).
	#[derive(Encode, Decode, Clone, Debug, MaxEncodedLen)]
	pub(crate) struct V4AuthorizerBudget<BlockNumber> {
		pub quota: Option<Quota>,
		pub authorization_period: Option<BlockNumber>,
		pub valid_until: Option<BlockNumber>,
	}

	pub struct VersionUncheckedMigrateV4ToV5<T>(PhantomData<T>);

	impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV4ToV5<T> {
		fn on_runtime_upgrade() -> Weight {
			let mut migrated: u64 = 0;
			AllowedAuthorizers::<T>::translate::<V4AuthorizerBudget<BlockNumberFor<T>>, _>(
				|who, old| {
					migrated = migrated.saturating_add(1);
					// Authorizers registered before v5 never had their System provider
					// reference bumped (the feature was added together with this storage
					// shape). Bring them in line with `add_authorizer` so a `feeless`
					// authorizer with no balance can't be reaped between dispatches.
					Pallet::<T>::inc_authorizer_providers(&who);
					Some(AuthorizerBudget {
						quota: old.quota,
						valid_until: old.valid_until,
						feeless: true,
					})
				},
			);
			tracing::info!(target: LOG_TARGET, migrated, "v4->v5 migration complete");
			// 1 read + 1 write per entry for `AllowedAuthorizers` (via `translate`),
			// plus 1 read + 1 write per entry for `frame_system::Account` (via
			// `inc_providers`).
			T::DbWeight::get().reads_writes(migrated.saturating_mul(2), migrated.saturating_mul(2))
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			use polkadot_sdk_frame::deps::frame_support::storage::StoragePrefixedMap;
			let prefix = AllowedAuthorizers::<T>::final_prefix();
			let mut previous_key = prefix.to_vec();
			let mut count: u64 = 0;
			while let Some(key) = polkadot_sdk_frame::deps::sp_io::storage::next_key(&previous_key)
				.filter(|k| k.starts_with(&prefix))
			{
				previous_key = key;
				count += 1;
			}
			tracing::info!(
				target: LOG_TARGET,
				count,
				"v4->v5 pre_upgrade: AllowedAuthorizers entries",
			);
			Ok(count.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(
			state: Vec<u8>,
		) -> Result<(), polkadot_sdk_frame::deps::sp_runtime::TryRuntimeError> {
			let old_count =
				u64::decode(&mut &state[..]).map_err(|_| "Failed to decode pre_upgrade state")?;
			let new_count = AllowedAuthorizers::<T>::iter().count() as u64;
			polkadot_sdk_frame::prelude::ensure!(
				new_count == old_count,
				"v4->v5 post_upgrade: entry count changed",
			);
			tracing::info!(
				target: LOG_TARGET,
				old_count,
				new_count,
				"v4->v5 post_upgrade: valid",
			);
			Ok(())
		}
	}

	/// Versioned migration v4→v5: drops `authorization_period` from `AuthorizerBudget`.
	pub type MigrateV4ToV5<T> = VersionedMigration<
		4,
		5,
		VersionUncheckedMigrateV4ToV5<T>,
		Pallet<T>,
		<T as polkadot_sdk_frame::deps::frame_system::Config>::DbWeight,
	>;
}
