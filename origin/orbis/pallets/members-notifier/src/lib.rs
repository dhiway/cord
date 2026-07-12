// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Members Notifier Pallet.
//!
//! Responsible for sharing ring roots of selected members collection(s) with interested parties.
//! Implements a publisher/notifier side of publish-subscribe pattern, uses XCM to notify
//! subscribers of changes to ring roots.
//!
//! ## Subscription Lifecycle
//!
//! 1. Subscription starts with a governance call to `subscribe` on notifier. The call parameters
//!    specify subscriber parachain id.
//! 2. Notifier sends initial ring roots via XCM Transact that calls `initialize_ring_roots` on the
//!    chosen subscriber.
//! 3. Ring root updates (new/updated/deleted rings) are sent periodically from the notifier and
//!    received by the subscriber via `process_ring_updates`.
//! 4. Subscription ends via a call to `terminate_subscription` on the subscriber that also ends the
//!    subscription on the notifier side.
//!
//! ## Updates distribution mechanism
//!
//! Updates distribution is handled by an offchain worker that submits authorized transactions.
//!
//! When a ring root changes, the update is recorded in a page of pending updates. Once a page is
//! full or enough time has passed, the offchain worker seals the page into a batch. For each
//! subscriber that has not yet received the batch, a separate authorized transaction resolves the
//! actual ring root data for every changed index, packs it into one or more XCM messages sized to
//! fit the HRMP channel, and dispatches them to the subscriber.
//! After all subscribers have been served, the batch is cleared.
//!
//! Since each operation — sealing, per-subscriber send, initialization page, stuck-batch
//! abandonment, catchup delivery — becomes its own extrinsic, the weight of a single block
//! stays predictable. Batches that have not completed within `StuckBatchTimeout` blocks are
//! abandoned by the offchain worker.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
pub mod weights;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

use alloc::{collections::BTreeMap, vec::Vec};
use codec::{Encode, MaxEncodedLen};
use cumulus_primitives_core::{GetChannelInfo, ParaId};
use frame_support::traits::{EnsureOrigin, Get, GetCallName, UnixTime};
use frame_system::offchain::{CreateAuthorizedTransaction, SubmitTransaction};
use indiv_support::traits::{
	Identifier, OnRingRootChange, RingIndex, RingRootOp, RingRootsProvider,
};
use verifiable::GenerateVerifiable;
use xcm::{opaque::latest::SendXcm, prelude::*};

const LOG_TARGET: &str = "runtime::indiv-pallet-members-notifier";

/// Retry window (in blocks) for offchain-worker transactions: retries within a window are
/// byte-identical and deduplicated by the transaction pool, while a window switch changes
/// the discriminator — and thus the transaction hash — escaping both mempool-level
/// deduplication and the pool rotator's inclusion ban. Kept well under `StuckBatchTimeout`.
const TX_RETRY_WINDOW: u32 = 8;
/// Number of blocks between repeated failure warnings for offchain-worker transactions,
/// so an eventual transaction-pool stall is not logged at each block.
const STALL_WARN_PERIOD: u32 = 32;
/// Finite longevity for offchain-worker transactions so that stranded retries self-evict
/// from the pool. Kept below `StuckBatchTimeout` so a stale `abandon_stuck_batch` for one
/// batch cannot linger into the next batch's lifetime.
const TX_LONGEVITY: u64 = 64;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::DefensiveTruncateFrom};
	use frame_system::pallet_prelude::*;
	use indiv_support::traits::RingExponent;
	use sp_runtime::{
		traits::Zero,
		transaction_validity::{InvalidTransaction, ValidTransaction, ValidTransactionBuilder},
		SaturatedConversion, Saturating,
	};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateAuthorizedTransaction<Call<Self>> {
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// XCM sender for sending updates to parachains.
		type XcmRouter: SendXcm;

		/// Origin allowed to manage subscriptions (subscribe/unsubscribe).
		type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin> + 'static;

		/// Cryptographic system for ring proofs.
		type Crypto: GenerateVerifiable;

		/// Source of time for batch timestamps.
		type Clock: UnixTime;

		/// Maximum number of subscribers allowed.
		#[pallet::constant]
		type MaxSubscribers: Get<u32>;

		/// Maximum updates per batch.
		#[pallet::constant]
		type MaxUpdatesPerBatch: Get<u32>;

		/// Maximum collections a subscriber can subscribe to.
		#[pallet::constant]
		type MaxCollectionsPerSubscriber: Get<u32>;

		/// Maximum number of ring collections.
		#[pallet::constant]
		type MaxCollections: Get<u32>;

		/// Provider of ring roots.
		type RingRootsProvider: RingRootsProvider<<Self::Crypto as GenerateVerifiable>::Members>;

		/// Origin check for replay requests from subscribers.
		/// Returns the ParaId of the requesting subscriber.
		type EnsureSubscriberOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = ParaId> + 'static;

		/// Channel info provider for querying HRMP channel max message size.
		type ChannelInfo: GetChannelInfo;

		/// Benchmarks setup helper.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;

		/// Minimum number of blocks between update batches.
		#[pallet::constant]
		type UpdateTriggerBlocks: Get<BlockNumberFor<Self>>;

		/// Minimum number of updates to trigger immediate batch send.
		#[pallet::constant]
		type UpdateTriggerThreshold: Get<u32>;

		/// Per-index weight surcharge for remote execution on the subscriber chain.
		#[pallet::constant]
		type RequestReplayRemoteWeight: Get<Weight>;

		/// Minimum number of blocks between offchain worker runs.
		#[pallet::constant]
		type OffchainWorkerInterval: Get<BlockNumberFor<Self>>;

		/// Number of blocks after which a stuck batch is abandoned.
		#[pallet::constant]
		type StuckBatchTimeout: Get<BlockNumberFor<Self>>;

		/// Cooldown, in seconds, between replay requests for the same subscriber and collection.
		#[pallet::constant]
		type ReplayCooldownSeconds: Get<u64>;
	}

	/// Subscriber registry with their subscribed collections and initialization state.
	#[pallet::storage]
	pub type Subscribers<T: Config> =
		CountedStorageMap<_, Identity, ParaId, SubscriberInfo<T>, OptionQuery>;

	/// Sequence number for sealed batch.
	/// Incremented when a batch is sealed - ready for distribution.
	/// Serves as the batch identifier for replay and distribution.
	#[pallet::storage]
	pub type SealedBatchSequence<T: Config> = StorageValue<_, SequenceNumber, ValueQuery>;

	/// Paging state: write page, send page, and last update block.
	#[pallet::storage]
	pub type PageState<T: Config> = StorageValue<_, PagingState<BlockNumberFor<T>>, ValueQuery>;

	/// Changed ring root keys, paged.
	/// Key: (page_index, identifier, ring_index). Populated by `OnRingRootChange`,
	/// consumed by `enqueue_updates` one page at a time from `PageState::send_page`.
	#[pallet::storage]
	pub type PendingUpdates<T: Config> = StorageNMap<
		_,
		(NMapKey<Identity, u16>, NMapKey<Identity, Identifier>, NMapKey<Identity, RingIndex>),
		(),
		OptionQuery,
	>;

	/// Per-page count of entries in PendingUpdates.
	#[pallet::storage]
	pub type PageUpdatesCount<T: Config> = StorageMap<_, Identity, u16, u32, ValueQuery>;

	/// State for paginated subscriber initialization.
	#[pallet::storage]
	pub type PendingInit<T: Config> =
		CountedStorageMap<_, Identity, ParaId, PendingInitState<T>, OptionQuery>;

	/// Indices per collection for the sealed (current) batch.
	#[pallet::storage]
	pub type SealedBatchIndices<T: Config> = StorageMap<
		_,
		Identity,
		Identifier,
		BoundedVec<RingIndex, T::MaxUpdatesPerBatch>,
		OptionQuery,
	>;

	/// Current batch distribution state.
	#[pallet::storage]
	pub type CurrentBatch<T: Config> =
		StorageValue<_, BatchDistributionState<BlockNumberFor<T>>, OptionQuery>;

	/// Tracks which subscribers have received the current batch.
	#[pallet::storage]
	pub type SubscribersWithCurrentBatch<T: Config> =
		StorageMap<_, Identity, ParaId, (), OptionQuery>;

	/// Last replay time per (subscriber, collection) pair.
	/// Used to enforce a cooldown between replay requests.
	#[pallet::storage]
	pub type LastReplayTime<T: Config> =
		StorageDoubleMap<_, Identity, ParaId, Identity, Identifier, u64, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A parachain subscribed.
		Subscribed { para_id: ParaId },
		/// A parachain unsubscribed.
		Unsubscribed { para_id: ParaId },
		/// Update batch sent to subscriber.
		UpdatesSent { para_id: ParaId, update_count: u32 },
		/// Sending ring root updates to a subscriber failed.
		UpdateSendFailed { para_id: ParaId },
		/// Replay of ring roots requested by a subscriber.
		ReplayRequested { para_id: ParaId, identifier: Identifier, indices_count: u32 },
		/// A stuck batch was abandoned by the offchain worker.
		BatchAbandoned { sequence: SequenceNumber },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Subscriber not found.
		SubscriberNotFound,
		/// Subscriber already exists.
		AlreadySubscribed,
		/// Maximum subscribers reached.
		TooManySubscribers,
		/// Collections list must be sorted in strictly ascending order with no duplicates.
		InvalidCollectionsList,
		/// Too many ring root updates to fit in a single batch.
		TooManyUpdates,
		/// XCM send failed.
		XcmSendFailed,
		/// Subscriber is not subscribed to the requested collection.
		NotSubscribedToCollection,
		/// Ring root index is out of range.
		InvalidRingIndex,
		/// Requested updates exceed the subscriber's HRMP channel capacity.
		ExceedsChannelCapacity,
		/// No active batch exists.
		NoBatchActive,
		/// No pending initialization for this subscriber.
		NoPendingInit,
		/// Replay cooldown has not elapsed for this subscriber and collection.
		ReplayCooldownActive,
		/// Replay requested with an empty list of ring root indices.
		EmptyRingIndices,
	}

	/// Call declaration for members-subscriber pallet on subscriber chains.
	///
	/// The field order of each variant must mirror the parameter order of the
	/// corresponding dispatchable in `members-subscriber`.
	#[derive(Encode, Decode, DecodeWithMemTracking)]
	pub(crate) enum SubscriberCall<T: Config> {
		#[codec(index = 0)]
		InitializeRingRoots { ring_exponent: RingExponent, roots: RingRootUpdatesBatch<T> },
		#[codec(index = 1)]
		UpdateRingRoots { updates: RingRootUpdatesBatch<T> },
		#[codec(index = 2)]
		TerminateSubscription,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers the parachain as a subscriber.
		/// The initial state will be sent over shortly via XCM.
		///
		/// ## Origin
		/// Requires `ManageOrigin` (governance/root).
		///
		/// ## Parameters
		/// - `subscriber_parachain_id`: The ParaId of the subscribing parachain.
		/// - `members_collections`: List of collection identifiers to subscribe to and their
		///   respective ring exponents.
		/// - `pallet_index`: Pallet index of members-subscriber on the subscriber chain.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::subscribe())]
		pub fn subscribe(
			origin: OriginFor<T>,
			subscriber_parachain_id: ParaId,
			members_collections: BoundedVec<
				(Identifier, RingExponent),
				T::MaxCollectionsPerSubscriber,
			>,
			pallet_index: u8,
		) -> DispatchResult {
			T::ManageOrigin::ensure_origin(origin)?;

			ensure!(
				!Subscribers::<T>::contains_key(subscriber_parachain_id),
				Error::<T>::AlreadySubscribed
			);
			ensure!(
				Subscribers::<T>::count() < T::MaxSubscribers::get(),
				Error::<T>::TooManySubscribers
			);

			// Strictly ascending order required to guarantee uniqueness.
			ensure!(
				members_collections.windows(2).all(|w| w[0].0 < w[1].0),
				Error::<T>::InvalidCollectionsList
			);

			Self::register_for_init(subscriber_parachain_id, members_collections, pallet_index)?;

			Self::deposit_event(Event::Subscribed { para_id: subscriber_parachain_id });

			Ok(())
		}

		/// Unsubscribes a parachain.
		///
		/// ## Origin
		/// - **Self-unsubscribe**: Subscriber parachain via XCM (`EnsureSubscriberOrigin`)
		/// - **Governance unsubscribe**: Requires `ManageOrigin`
		///
		/// ## Parameters
		/// - `subscriber_parachain_id`: The ParaId to unsubscribe. Required for governance, ignored
		///   for self-unsubscribe (derived from XCM origin).
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::unsubscribe())]
		pub fn unsubscribe(
			origin: OriginFor<T>,
			subscriber_parachain_id: Option<ParaId>,
		) -> DispatchResult {
			let (para_id, from_subscriber) = match T::EnsureSubscriberOrigin::try_origin(origin) {
				Ok(id) => (id, true),
				Err(origin) => {
					T::ManageOrigin::ensure_origin(origin)?;
					let id = subscriber_parachain_id.ok_or(Error::<T>::SubscriberNotFound)?;
					(id, false)
				},
			};

			let info = Subscribers::<T>::get(para_id).ok_or(Error::<T>::SubscriberNotFound)?;

			// Only notifying subscriber when unsubscribe is initiated by governance,
			// not when the subscriber itself requested unsubscription.
			if !from_subscriber {
				let _ = Self::send_xcm_call_to_subscriber(
					para_id,
					SubscriberCall::TerminateSubscription,
					info.pallet_index,
				)
				.inspect_err(|e| {
					log::error!(
						target: LOG_TARGET,
						"Failed to send terminate XCM to {para_id:?}: {e:?}",
					);
				});
			}

			for (identifier, _) in info.collections.iter() {
				LastReplayTime::<T>::remove(para_id, identifier);
			}

			Subscribers::<T>::remove(para_id);
			PendingInit::<T>::remove(para_id);

			// Adjusting the current batch if the subscriber was part of it.
			if let Some(mut current_batch) = CurrentBatch::<T>::get() {
				if SubscribersWithCurrentBatch::<T>::contains_key(para_id) {
					// Already processed — so removing the status entry.
					SubscribersWithCurrentBatch::<T>::remove(para_id);
				} else {
					// Only decrementing if this subscriber was counted in
					// remaining_subscribers (last_init_sequence < batch sequence).
					let was_counted = info.last_init_sequence < current_batch.sequence;
					if was_counted && current_batch.remaining_subscribers > 0 {
						current_batch.remaining_subscribers =
							current_batch.remaining_subscribers.saturating_sub(1);
						if current_batch.remaining_subscribers == 0 {
							Self::clear_batch();
						} else {
							CurrentBatch::<T>::put(current_batch);
						}
					}
				}
			}

			Self::deposit_event(Event::Unsubscribed { para_id });

			Ok(())
		}

		/// Requests replay of specific ring roots.
		///
		/// Permissionless — any signed origin can request a replay for any subscriber.
		/// The subscriber parachain is identified by the `subscriber_parachain_id` parameter.
		///
		/// Parameters:
		/// - `subscriber_parachain_id`: The ParaId of the subscriber.
		/// - `identifier`: Collection identifier.
		/// - `ring_root_indices`: List of ring root indices, must be in strictly ascending order.
		#[pallet::call_index(2)]
		#[pallet::weight(
			T::WeightInfo::request_replay(ring_root_indices.len() as u32)
				.saturating_add(
					T::RequestReplayRemoteWeight::get()
						.saturating_mul(ring_root_indices.len() as u64)
				)
		)]
		pub fn request_replay(
			origin: OriginFor<T>,
			subscriber_parachain_id: ParaId,
			identifier: Identifier,
			ring_root_indices: BoundedVec<RingIndex, T::MaxUpdatesPerBatch>,
		) -> DispatchResult {
			ensure_signed(origin)?;
			ensure!(!ring_root_indices.is_empty(), Error::<T>::EmptyRingIndices);
			let para_id = subscriber_parachain_id;

			let subscriber_info =
				Subscribers::<T>::get(para_id).ok_or(Error::<T>::SubscriberNotFound)?;

			ensure!(
				subscriber_info
					.collections
					.binary_search_by_key(&identifier, |&(i, _)| i)
					.is_ok(),
				Error::<T>::NotSubscribedToCollection
			);

			let next = T::RingRootsProvider::next_ring_index(identifier);
			ensure!(
				ring_root_indices.windows(2).all(|w| w[0] < w[1]) &&
					ring_root_indices.last().is_none_or(|&last| last < next),
				Error::<T>::InvalidRingIndex
			);

			let max_per_xcm = Self::get_xcm_update_batch_size(para_id, Self::xcm_overhead())
				.ok_or(Error::<T>::XcmSendFailed)?;
			ensure!(
				ring_root_indices.len() as u32 <= max_per_xcm,
				Error::<T>::ExceedsChannelCapacity
			);

			let now = T::Clock::now().as_secs();
			if let Some(last) = LastReplayTime::<T>::get(para_id, identifier) {
				ensure!(
					now.saturating_sub(last) >= T::ReplayCooldownSeconds::get(),
					Error::<T>::ReplayCooldownActive
				);
			}

			let indices_count = ring_root_indices.len() as u32;

			let raw_updates = Self::resolve_updates_for_indices(identifier, &ring_root_indices);

			let updates: BoundedVec<RingRootUpdate<T>, T::MaxUpdatesPerBatch> =
				raw_updates.try_into().map_err(|_| Error::<T>::TooManyUpdates)?;

			let batch = RingRootUpdatesBatch {
				identifier,
				sequence: SealedBatchSequence::<T>::get(),
				source_time: T::Clock::now().as_secs(),
				updates,
				next_ring_index: next,
			};

			Self::send_updates_to_subscriber(para_id, batch, subscriber_info.pallet_index)?;

			LastReplayTime::<T>::insert(para_id, identifier, now);

			Self::deposit_event(Event::ReplayRequested { para_id, identifier, indices_count });

			Ok(())
		}

		/// Enqueues pending updates into a sealed batch for distribution.
		///
		/// Authorized call submitted by the offchain worker.
		#[pallet::authorize(|source, send_page, _discriminator| {
			Self::authorize_enqueue_updates(source, send_page)
		})]
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::enqueue_updates(T::MaxUpdatesPerBatch::get()))]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_enqueue_updates())]
		pub fn enqueue_updates(
			origin: OriginFor<T>,
			_send_page: u16,
			// Per-window discriminator (the submitting retry window) so stalled
			// offchain-worker retries eventually produce a fresh transaction hash.
			_discriminator: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let page_state = PageState::<T>::get();
			let write_page = page_state.write_page;
			let has_sealed_pages = page_state.send_page != write_page;

			debug_assert!(
				has_sealed_pages || PageUpdatesCount::<T>::get(write_page) > 0,
				"authorize should have rejected: no pending work",
			);

			let actual_send_page = page_state.send_page;

			// No subscribers — cleaning up the send buffer without sealing a batch.
			if Subscribers::<T>::count() == 0 {
				if actual_send_page == write_page {
					PageState::<T>::mutate(|s| s.write_page = s.write_page.wrapping_add(1));
				}
				Self::cleanup_send_buffer(actual_send_page);
				return Ok(Some(T::WeightInfo::enqueue_updates(0)).into());
			}

			let (sequence, source_time) = Self::seal_and_start_batch(actual_send_page, write_page);

			// Collecting changed indices from send page, grouped by collection.
			let mut actual_count: u32 = 0;
			let mut changed_indices: BTreeMap<Identifier, Vec<RingIndex>> = BTreeMap::new();
			for ((identifier, ring_index), ()) in
				PendingUpdates::<T>::iter_prefix((actual_send_page,))
			{
				changed_indices.entry(identifier).or_default().push(ring_index);
				actual_count = actual_count.saturating_add(1);
			}

			// Storing sealed batch indices per collection.
			for (identifier, mut indices) in changed_indices {
				indices.sort_unstable();
				let bounded =
					BoundedVec::<RingIndex, T::MaxUpdatesPerBatch>::defensive_truncate_from(
						indices,
					);
				SealedBatchIndices::<T>::insert(identifier, bounded);
			}

			let remaining_subscribers = Subscribers::<T>::count();

			Self::cleanup_send_buffer(actual_send_page);

			let current_block = frame_system::Pallet::<T>::block_number();
			CurrentBatch::<T>::put(BatchDistributionState {
				sequence,
				source_time,
				sealed_at: current_block,
				remaining_subscribers,
			});

			Ok(Some(T::WeightInfo::enqueue_updates(actual_count)).into())
		}

		/// Sends the current batch to a specific subscriber.
		///
		/// Authorized maintenance call submitted by the offchain worker.
		#[pallet::authorize(|source, para_id, sequence, _discriminator| {
			Self::authorize_send_batch(source, para_id, sequence)
		})]
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::send_batch(T::MaxUpdatesPerBatch::get()))]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_send_batch())]
		pub fn send_batch(
			origin: OriginFor<T>,
			para_id: ParaId,
			sequence: SequenceNumber,
			// Per-window discriminator (the submitting retry window) so stalled
			// offchain-worker retries eventually produce a fresh transaction hash.
			_discriminator: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let mut current_batch = CurrentBatch::<T>::get().ok_or(Error::<T>::NoBatchActive)?;

			let subscriber_info =
				Subscribers::<T>::get(para_id).ok_or(Error::<T>::SubscriberNotFound)?;
			let source_time = current_batch.source_time;

			debug_assert!(
				subscriber_info.last_init_sequence < sequence,
				"authorize should have rejected: subscriber already has this data",
			);

			let mut actual_count: u32 = 0;
			let overhead = Self::xcm_overhead();
			let max_per_xcm =
				Self::get_xcm_update_batch_size(para_id, overhead).unwrap_or(0) as usize;
			let mut send_failed = false;

			if max_per_xcm == 0 {
				log::warn!(
					target: LOG_TARGET,
					"No channel capacity for {para_id:?}, skipping updates",
				);
				Self::deposit_event(Event::UpdateSendFailed { para_id });
			} else {
				for (collection, _) in &subscriber_info.collections {
					let Some(indices) = SealedBatchIndices::<T>::get(collection) else {
						continue;
					};
					if indices.is_empty() {
						continue;
					}

					actual_count = actual_count.saturating_add(indices.len() as u32);
					let updates = Self::resolve_updates_for_indices(*collection, &indices);
					let next = T::RingRootsProvider::next_ring_index(*collection);

					for chunk in updates.chunks(max_per_xcm) {
						let bounded: BoundedVec<RingRootUpdate<T>, T::MaxUpdatesPerBatch> =
							match chunk.to_vec().try_into() {
								Ok(b) => b,
								Err(_) => {
									defensive!("update chunk exceeds MaxUpdatesPerBatch");
									continue;
								},
							};
						if bounded.is_empty() {
							continue;
						}

						let update_count = bounded.len() as u32;
						let batch = RingRootUpdatesBatch {
							identifier: *collection,
							sequence,
							source_time,
							updates: bounded,
							next_ring_index: next,
						};

						match Self::send_updates_to_subscriber(
							para_id,
							batch,
							subscriber_info.pallet_index,
						) {
							Ok(()) => {
								Self::deposit_event(Event::UpdatesSent { para_id, update_count });
							},
							Err(e) => {
								log::warn!(
									target: LOG_TARGET,
									"Update XCM to {para_id:?} failed: {e:?}",
								);
								Self::deposit_event(Event::UpdateSendFailed { para_id });
								send_failed = true;
							},
						}
					}
				}

				// No advance on failure — retry next offchain worker cycle.
				if send_failed {
					return Ok(Some(T::WeightInfo::send_batch(actual_count)).into());
				}
			}

			// Marking subscriber as processed.
			SubscribersWithCurrentBatch::<T>::insert(para_id, ());

			current_batch.remaining_subscribers =
				current_batch.remaining_subscribers.saturating_sub(1);

			if current_batch.remaining_subscribers == 0 {
				Self::clear_batch();
			} else {
				CurrentBatch::<T>::put(current_batch);
			}

			Ok(Some(T::WeightInfo::send_batch(actual_count)).into())
		}

		/// Sends one page of initialization data to a subscriber.
		///
		/// Authorized maintenance call submitted by the offchain worker.
		#[pallet::authorize(|source, para_id, current_collection_index, after_ring_index, _discriminator| {
			Self::authorize_send_init_page(source, para_id, current_collection_index, after_ring_index)
		})]
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::send_init_page(T::MaxUpdatesPerBatch::get()))]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_send_init_page())]
		pub fn send_init_page(
			origin: OriginFor<T>,
			para_id: ParaId,
			_current_collection_index: u32,
			_after_ring_index: Option<RingIndex>,
			// Per-window discriminator (the submitting retry window) so stalled
			// offchain-worker retries eventually produce a fresh transaction hash.
			_discriminator: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let state = PendingInit::<T>::get(para_id).ok_or(Error::<T>::NoPendingInit)?;

			let Some(max_per_xcm) = Self::max_updates_per_xcm(para_id) else {
				log::warn!(
					target: LOG_TARGET,
					"No channel to {para_id:?} for init, will retry next offchain worker cycle",
				);
				return Ok(Some(T::WeightInfo::send_init_page(0)).into());
			};

			let Some(&(current_collection, ring_exponent)) =
				state.collections.get(state.current_collection_index as usize)
			else {
				// All collections fully initialized.
				PendingInit::<T>::remove(para_id);
				return Ok(Some(T::WeightInfo::send_init_page(0)).into());
			};

			let roots = T::RingRootsProvider::get_ring_roots_paginated(
				current_collection,
				state.after_ring_index,
				max_per_xcm,
			);
			let actual_count = roots.len() as u32;
			let might_have_more_in_collection = actual_count == max_per_xcm;
			let last_returned_index = roots.last().map(|(idx, _, _)| *idx);

			let batch = Self::build_init_batch(&state, current_collection, roots);
			if let Err(e) =
				Self::send_initialize_ring_roots(para_id, batch, ring_exponent, state.pallet_index)
			{
				log::warn!(
					target: LOG_TARGET,
					"Init XCM to {para_id:?} failed: {e:?}, will retry next offchain worker cycle",
				);
				// No advance on failure — retry next offchain worker cycle.
				return Ok(Some(T::WeightInfo::send_init_page(actual_count)).into());
			}

			Self::advance_init_pagination(
				para_id,
				&state,
				state.collections.len(),
				might_have_more_in_collection,
				last_returned_index,
			);

			Ok(Some(T::WeightInfo::send_init_page(actual_count)).into())
		}

		/// Abandons a stuck batch that exceeded `StuckBatchTimeout`.
		/// Subscribers that did not receive the batch can recover via `request_replay`.
		///
		/// Authorized maintenance call submitted by the offchain worker when a batch has
		/// been active longer than `StuckBatchTimeout`.
		#[pallet::authorize(|source, _discriminator| {
			Self::authorize_abandon_stuck_batch(source)
		})]
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::abandon_stuck_batch())]
		#[pallet::weight_of_authorize(T::WeightInfo::authorize_abandon_stuck_batch())]
		pub fn abandon_stuck_batch(
			origin: OriginFor<T>,
			// Per-window discriminator (the submitting retry window) so stalled
			// offchain-worker retries eventually produce a fresh transaction hash.
			_discriminator: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure_authorized(origin)?;

			let current_batch = CurrentBatch::<T>::get().ok_or(Error::<T>::NoBatchActive)?;

			let sequence = current_batch.sequence;
			Self::clear_batch();
			Self::deposit_event(Event::BatchAbandoned { sequence });

			Ok(().into())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Drives updates batch distribution via authorized transactions.
		///
		/// Each operation (seal, per-subscriber send, init page, stuck-batch abandonment)
		/// becomes a separate extrinsic with its own bounded weight.
		///
		/// Each cycle:
		/// 1. Seals pending updates into a new batch if no batch is active.
		/// 2. Sends the active batch to subscribers that haven't received it yet.
		/// 3. Continues initialization for new subscribers.
		/// 4. Abandons stuck batches that exceeded `StuckBatchTimeout`.
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// To limit execution frequency.
			if !(block_number % T::OffchainWorkerInterval::get()).is_zero() {
				return;
			}

			// A discriminator of a retry window to enable pool-level deduplication.
			// Switching the window changes the transaction hash, avoiding the pool rotator ban.
			let discriminator = block_number / TX_RETRY_WINDOW.into();

			let current_batch = CurrentBatch::<T>::get();

			// If there's no batch being processed and there's pending work, enqueueing updates.
			if current_batch.is_none() && Self::has_pending_work(block_number) {
				let call = Call::enqueue_updates {
					send_page: PageState::<T>::get().send_page,
					discriminator,
				};
				Self::submit_authorized_transaction(call, block_number);
			}

			// If there's a current batch, sending it to subscribers that didn't receive it yet.
			if let Some(ref current_batch) = current_batch {
				if current_batch.remaining_subscribers > 0 {
					for (para_id, info) in Subscribers::<T>::iter() {
						if !SubscribersWithCurrentBatch::<T>::contains_key(para_id) &&
							info.last_init_sequence < current_batch.sequence
						{
							let call = Call::send_batch {
								para_id,
								sequence: current_batch.sequence,
								discriminator,
							};
							Self::submit_authorized_transaction(call, block_number);
						}
					}
				}

				// Abandoning stuck batch if timed out.
				if block_number.saturating_sub(current_batch.sealed_at) >=
					T::StuckBatchTimeout::get()
				{
					let call = Call::abandon_stuck_batch { discriminator };
					Self::submit_authorized_transaction(call, block_number);
				}
			}

			// Processing pending inits.
			for (para_id, state) in PendingInit::<T>::iter() {
				let call = Call::send_init_page {
					para_id,
					current_collection_index: state.current_collection_index,
					after_ring_index: state.after_ring_index,
					discriminator,
				};
				Self::submit_authorized_transaction(call, block_number);
			}
		}

		#[cfg(feature = "std")]
		fn integrity_test() {
			assert!(
				T::UpdateTriggerThreshold::get() <= T::MaxUpdatesPerBatch::get(),
				"UpdateTriggerThreshold must be <= MaxUpdatesPerBatch"
			);
			assert!(
				<RingRootUpdate<T>>::max_encoded_len() > 0,
				"RingRootUpdate max_encoded_len must be non-zero"
			);
			assert!(
				T::MaxCollectionsPerSubscriber::get() <= T::MaxCollections::get(),
				"MaxCollectionsPerSubscriber must be <= MaxCollections",
			);
			assert!(
				T::MaxCollectionsPerSubscriber::get() <= T::MaxUpdatesPerBatch::get(),
				"MaxCollectionsPerSubscriber must be <= MaxUpdatesPerBatch — required by send_batch bench design",
			);
			assert_ne!(
				core::any::TypeId::of::<T::ManageOrigin>(),
				core::any::TypeId::of::<T::EnsureSubscriberOrigin>(),
				"ManageOrigin must not be the same type as EnsureSubscriberOrigin",
			);
			assert!(
				TX_LONGEVITY < T::StuckBatchTimeout::get().saturated_into::<u64>(),
				"TX_LONGEVITY must be < StuckBatchTimeout so a stale `abandon_stuck_batch` \
				 for one batch self-evicts from the pool before the next batch can be stuck",
			);

			// Weight per extrinsic must fit in `Normal.max_extrinsic`,
			// otherwise an OCW-submitted authorized transaction is silently
			// dropped at the transaction-pool level and the whole
			// flow (init pagination, batch send, etc.) stalls forever.
			let block_weights = <T as frame_system::Config>::BlockWeights::get();
			let normal_max = block_weights
				.per_class
				.get(DispatchClass::Normal)
				.max_extrinsic
				.expect("Normal class must have max_extrinsic configured");
			let assert_fits = |name: &str, w: Weight| {
				assert!(
					w.all_lte(normal_max),
					"`{name}` worst-case weight {w:?} exceeds Normal max_extrinsic \
					 {normal_max:?} — lower MaxUpdatesPerBatch or reduce per-item cost",
				);
			};

			let max_n = T::MaxUpdatesPerBatch::get();
			assert_fits(
				"request_replay",
				T::WeightInfo::request_replay(max_n).saturating_add(
					T::RequestReplayRemoteWeight::get().saturating_mul(max_n.into()),
				),
			);
			assert_fits(
				"enqueue_updates",
				T::WeightInfo::enqueue_updates(max_n)
					.saturating_add(T::WeightInfo::authorize_enqueue_updates()),
			);
			assert_fits(
				"send_batch",
				T::WeightInfo::send_batch(max_n)
					.saturating_add(T::WeightInfo::authorize_send_batch()),
			);
			assert_fits(
				"send_init_page",
				T::WeightInfo::send_init_page(max_n)
					.saturating_add(T::WeightInfo::authorize_send_init_page()),
			);
		}
	}

	impl<T: Config> Pallet<T> {
		fn ensure_local_source(source: TransactionSource) -> Result<(), TransactionValidityError> {
			if !matches!(source, TransactionSource::InBlock | TransactionSource::Local) {
				return Err(CustomInvalidity::TransactionNotLocal.into());
			}
			Ok(())
		}

		pub(crate) fn authorize_enqueue_updates(
			source: TransactionSource,
			send_page: &u16,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			if CurrentBatch::<T>::exists() {
				return Err(CustomInvalidity::BatchInProgress.into());
			}
			let page_state = PageState::<T>::get();
			if *send_page < page_state.send_page {
				return Err(InvalidTransaction::Stale.into());
			}
			if *send_page > page_state.send_page {
				return Err(InvalidTransaction::Future.into());
			}
			if !Self::has_processable_updates(&page_state) {
				return Err(CustomInvalidity::NoPendingWork.into());
			}

			let validity = Self::build_local_validity(
				ValidTransaction::with_tag_prefix("notifier:enqueue")
					.and_provides(page_state.send_page),
			);
			Ok((validity, Weight::zero()))
		}

		pub(crate) fn authorize_send_batch(
			source: TransactionSource,
			para_id: &ParaId,
			sequence: &SequenceNumber,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			let current_batch = CurrentBatch::<T>::get().ok_or(CustomInvalidity::NoBatchActive)?;
			if *sequence < current_batch.sequence {
				return Err(InvalidTransaction::Stale.into());
			}
			if *sequence > current_batch.sequence {
				return Err(InvalidTransaction::Future.into());
			}
			if current_batch.remaining_subscribers == 0 {
				return Err(CustomInvalidity::NoBatchActive.into());
			}
			if SubscribersWithCurrentBatch::<T>::contains_key(para_id) {
				return Err(CustomInvalidity::AlreadySent.into());
			}
			let subscriber_info = Subscribers::<T>::get(para_id)
				.ok_or::<TransactionValidityError>(CustomInvalidity::SubscriberNotFound.into())?;
			if subscriber_info.last_init_sequence >= current_batch.sequence {
				return Err(InvalidTransaction::Stale.into());
			}

			let validity = Self::build_local_validity(
				ValidTransaction::with_tag_prefix("notifier:send")
					.and_provides((*sequence, *para_id)),
			);
			Ok((validity, Weight::zero()))
		}

		pub(crate) fn authorize_send_init_page(
			source: TransactionSource,
			para_id: &ParaId,
			current_collection_index: &u32,
			after_ring_index: &Option<RingIndex>,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			let state = PendingInit::<T>::get(para_id).ok_or(CustomInvalidity::NoPendingInit)?;
			if *current_collection_index < state.current_collection_index {
				return Err(InvalidTransaction::Stale.into());
			}
			if *current_collection_index > state.current_collection_index {
				return Err(InvalidTransaction::Future.into());
			}
			if *after_ring_index < state.after_ring_index {
				return Err(InvalidTransaction::Stale.into());
			}
			if *after_ring_index > state.after_ring_index {
				return Err(InvalidTransaction::Future.into());
			}

			let validity = Self::build_local_validity(
				ValidTransaction::with_tag_prefix("notifier:init").and_provides((
					*para_id,
					state.current_collection_index,
					state.after_ring_index,
				)),
			);
			Ok((validity, Weight::zero()))
		}

		pub(crate) fn authorize_abandon_stuck_batch(
			source: TransactionSource,
		) -> Result<(ValidTransaction, Weight), TransactionValidityError> {
			Self::ensure_local_source(source)?;
			let current_batch = CurrentBatch::<T>::get().ok_or(CustomInvalidity::NoBatchActive)?;
			let now = frame_system::Pallet::<T>::block_number();
			if now.saturating_sub(current_batch.sealed_at) < T::StuckBatchTimeout::get() {
				return Err(CustomInvalidity::BatchNotStuck.into());
			}

			let validity = Self::build_local_validity(
				ValidTransaction::with_tag_prefix("notifier:abandon")
					.and_provides(current_batch.sequence),
			);
			Ok((validity, Weight::zero()))
		}

		/// Finalizes validity properties shared by all offchain-worker transactions.
		///
		/// Priority increases with block height (assigned at validation time) so a fresh
		/// retry strictly outbids an earlier transaction with the same provides tag that
		/// may be stranded in the pool, forcing a replacement.
		/// A finite longevity lets a stranded retry self-evict rather than linger indefinitely.
		/// Propagation is disabled because peers validate gossiped transactions with a source
		/// `External`, which fail `ensure_local_source`.
		fn build_local_validity(builder: ValidTransactionBuilder) -> ValidTransaction {
			let priority = frame_system::Pallet::<T>::block_number().saturated_into::<u64>();
			builder
				.priority(priority)
				.longevity(TX_LONGEVITY)
				.propagate(false)
				.build()
				.expect("tag prefix is not empty; qed")
		}

		/// Fetches ring root updates for a set of changed indices from `RingRootsProvider`.
		///
		/// Indices found in the provider are returned as `Built` updates.
		/// Indices not found are returned as `Deleted`.
		///
		/// Requires `indices` to be sorted in ascending order.
		fn resolve_updates_for_indices(
			identifier: Identifier,
			indices: &[RingIndex],
		) -> Vec<RingRootUpdate<T>> {
			let roots = T::RingRootsProvider::get_ring_roots(identifier, indices);

			let mut updates = Vec::with_capacity(indices.len());
			let mut roots_iter = roots.into_iter().peekable();

			for &idx in indices {
				if let Some((ring_index, root, revision)) =
					roots_iter.next_if(|(root_idx, _, _)| *root_idx == idx)
				{
					updates.push(RingRootUpdate {
						ring_index,
						op: RingRootOp::Built { revision, root },
					});
				} else {
					updates.push(RingRootUpdate { ring_index: idx, op: RingRootOp::Deleted });
				}
			}

			updates
		}

		/// Clears the send page entries, advances send page, and updates last_update_block.
		pub(crate) fn cleanup_send_buffer(send_page: u16) {
			let _ =
				PendingUpdates::<T>::clear_prefix((send_page,), T::MaxUpdatesPerBatch::get(), None);
			PageUpdatesCount::<T>::remove(send_page);
			PageState::<T>::mutate(|s| {
				s.send_page = s.send_page.wrapping_add(1);
				s.last_update_block = frame_system::Pallet::<T>::block_number();
			});
		}

		/// Build an XCM message wrapping an encoded Transact call.
		fn build_xcm_message(encoded_call: Vec<u8>) -> Xcm<()> {
			Xcm(alloc::vec![
				UnpaidExecution { weight_limit: Unlimited, check_origin: None },
				Transact {
					origin_kind: OriginKind::Native,
					call: encoded_call.into(),
					fallback_max_weight: None,
				},
			])
		}

		/// XCM message envelope size: `VersionedXcm<()>` wrapping a `Transact`
		/// that carries an empty `RingRootUpdatesBatch`. Routers compare this
		/// versioned-encoded length against `channel.max_message_size`.
		pub(crate) fn xcm_overhead() -> usize {
			let empty_batch = RingRootUpdatesBatch::<T> {
				identifier: [0u8; 32],
				sequence: 0,
				source_time: 0,
				updates: BoundedVec::default(),
				next_ring_index: 0,
			};
			let encoded_call =
				(u8::MAX, SubscriberCall::<T>::UpdateRingRoots { updates: empty_batch }).encode();
			VersionedXcm::<()>::from(Self::build_xcm_message(encoded_call)).encode().len()
		}

		/// Upper bound on the XCM message size for `n_updates` worst-case
		/// `RingRootUpdate`s.
		#[cfg(feature = "runtime-benchmarks")]
		pub(crate) fn xcm_message_max_size(n_updates: u32) -> usize {
			Self::xcm_overhead() + n_updates as usize * <RingRootUpdate<T>>::max_encoded_len()
		}

		/// Get the maximum XCM payload size for a specific subscriber.
		fn max_xcm_payload_for_subscriber(para_id: ParaId) -> Option<u32> {
			T::ChannelInfo::get_channel_info(para_id).map(|info| info.max_message_size)
		}

		/// Maximum ring root updates that fit in one XCM message.
		pub fn max_updates_per_xcm(para_id: ParaId) -> Option<u32> {
			Self::get_xcm_update_batch_size(para_id, Self::xcm_overhead())
		}

		/// Maximum ring root updates that fit in one XCM message, given pre-computed overhead.
		fn get_xcm_update_batch_size(para_id: ParaId, overhead: usize) -> Option<u32> {
			let max_payload = Self::max_xcm_payload_for_subscriber(para_id)?;

			let update_size = <RingRootUpdate<T>>::max_encoded_len();
			if update_size == 0 {
				return None;
			}

			let available = (max_payload as usize).saturating_sub(overhead);

			let count = (available / update_size) as u32;
			if count == 0 {
				return None;
			}
			Some(count.min(T::MaxUpdatesPerBatch::get()))
		}

		/// Seals the current write page (if needed) and starts a new batch.
		fn seal_and_start_batch(send_page: u16, write_page: u16) -> (SequenceNumber, u64) {
			if send_page == write_page {
				PageState::<T>::mutate(|s| s.write_page = s.write_page.wrapping_add(1));
			}

			let sequence = SealedBatchSequence::<T>::mutate(|n| {
				*n = n.saturating_add(1);
				*n
			});
			let source_time = T::Clock::now().as_secs();

			(sequence, source_time)
		}

		/// Send an XCM call to a subscriber parachain.
		fn send_xcm_call_to_subscriber(
			para_id: ParaId,
			call: SubscriberCall<T>,
			pallet_index: u8,
		) -> DispatchResult {
			use codec::Encode;

			let encoded_call = (pallet_index, call).encode();
			let destination = Location::new(1, [Parachain(para_id.into())]);
			let message = Self::build_xcm_message(encoded_call);

			send_xcm::<T::XcmRouter>(destination, message)
				.map_err(|_| Error::<T>::XcmSendFailed)?;
			Ok(())
		}

		/// Send ring root updates to a subscriber via XCM.
		pub(crate) fn send_updates_to_subscriber(
			para_id: ParaId,
			batch: RingRootUpdatesBatch<T>,
			pallet_index: u8,
		) -> DispatchResult {
			Self::send_xcm_call_to_subscriber(
				para_id,
				SubscriberCall::UpdateRingRoots { updates: batch },
				pallet_index,
			)
		}

		/// Send initial ring roots to a subscriber via XCM.
		fn send_initialize_ring_roots(
			para_id: ParaId,
			batch: RingRootUpdatesBatch<T>,
			ring_exponent: RingExponent,
			pallet_index: u8,
		) -> DispatchResult {
			Self::send_xcm_call_to_subscriber(
				para_id,
				SubscriberCall::InitializeRingRoots { ring_exponent, roots: batch },
				pallet_index,
			)
		}

		/// Registers a subscriber for initialization.
		fn register_for_init(
			para_id: ParaId,
			collections: BoundedVec<(Identifier, RingExponent), T::MaxCollectionsPerSubscriber>,
			pallet_index: u8,
		) -> DispatchResult {
			let batch_sequence = SealedBatchSequence::<T>::get();
			let source_time = T::Clock::now().as_secs();

			let subscriber_info = SubscriberInfo {
				collections: collections.clone(),
				last_init_sequence: batch_sequence,
				pallet_index,
			};
			Subscribers::<T>::insert(para_id, subscriber_info);

			PendingInit::<T>::insert(
				para_id,
				PendingInitState {
					collections,
					current_collection_index: 0,
					after_ring_index: None,
					sequence: batch_sequence,
					source_time,
					pallet_index,
					_phantom: Default::default(),
				},
			);

			Ok(())
		}

		/// Builds a batch from paginated ring roots for initialization.
		fn build_init_batch(
			state: &PendingInitState<T>,
			identifier: Identifier,
			roots: Vec<(u32, MembersOf<T>, u32)>,
		) -> RingRootUpdatesBatch<T> {
			defensive_assert!(
				roots.len() <= T::MaxUpdatesPerBatch::get() as usize,
				"init batch exceeds MaxUpdatesPerBatch"
			);
			let updates: BoundedVec<RingRootUpdate<T>, T::MaxUpdatesPerBatch> =
				BoundedVec::truncate_from(
					roots
						.into_iter()
						.map(|(ring_index, root, revision)| RingRootUpdate {
							ring_index,
							op: RingRootOp::Built { revision, root },
						})
						.collect(),
				);

			RingRootUpdatesBatch {
				identifier,
				sequence: state.sequence,
				source_time: state.source_time,
				updates,
				next_ring_index: T::RingRootsProvider::next_ring_index(identifier),
			}
		}

		/// Advances pagination state after processing one page of init data.
		fn advance_init_pagination(
			para_id: ParaId,
			state: &PendingInitState<T>,
			total_collections: usize,
			might_have_more_in_collection: bool,
			last_returned_index: Option<RingIndex>,
		) {
			if might_have_more_in_collection {
				PendingInit::<T>::mutate(para_id, |maybe_state| {
					if let Some(s) = maybe_state {
						s.after_ring_index = last_returned_index;
					}
				});
			} else if (state.current_collection_index as usize) <
				total_collections.saturating_sub(1)
			{
				PendingInit::<T>::mutate(para_id, |maybe_state| {
					if let Some(s) = maybe_state {
						s.current_collection_index = s.current_collection_index.saturating_add(1);
						s.after_ring_index = None;
					}
				});
			} else {
				PendingInit::<T>::remove(para_id);
			}
		}

		/// Clear all batch-related storage.
		fn clear_batch() {
			CurrentBatch::<T>::kill();
			let _ = SealedBatchIndices::<T>::clear(T::MaxCollections::get(), None);
			let _ = SubscribersWithCurrentBatch::<T>::clear(T::MaxSubscribers::get(), None);
		}

		/// Checks if there are sealed pages or throttle-eligible updates on the write page.
		fn has_processable_updates(page_state: &PagingState<BlockNumberFor<T>>) -> bool {
			if page_state.send_page != page_state.write_page {
				return true;
			}

			let count = PageUpdatesCount::<T>::get(page_state.write_page);
			if count == 0 {
				return false;
			}

			let current_block = frame_system::Pallet::<T>::block_number();
			current_block.saturating_sub(page_state.last_update_block) >=
				T::UpdateTriggerBlocks::get() ||
				count >= T::UpdateTriggerThreshold::get()
		}

		/// Checks if there is pending work for the offchain worker.
		fn has_pending_work(_current_block: BlockNumberFor<T>) -> bool {
			let page_state = PageState::<T>::get();
			Self::has_processable_updates(&page_state)
		}

		/// Submits an authorized transaction from the offchain worker with stall-aware logging.
		///
		/// Failed submissions within a retry window are expected pool-level deduplication
		/// of byte-identical retries; a persistent failure streak signals a possible
		/// transaction-pool stall — surfaced once per `STALL_WARN_PERIOD` blocks, not
		/// every block.
		pub(crate) fn submit_authorized_transaction(
			call: Call<T>,
			block_number: BlockNumberFor<T>,
		) {
			let call_name = call.get_call_name();
			let tx = T::create_authorized_transaction(call.into());
			if SubmitTransaction::<T, _>::submit_transaction(tx).is_ok() {
				log::debug!(
					target: LOG_TARGET,
					"offchain worker: submitted `{call_name}` successfully"
				);
				return;
			}

			if (block_number % STALL_WARN_PERIOD.into()).is_zero() {
				log::warn!(
					target: LOG_TARGET,
					"offchain worker: `{call_name}` repeatedly rejected by the \
					 transaction pool — possible stall"
				);
			} else {
				log::debug!(
					target: LOG_TARGET,
					"offchain worker: failed to submit `{call_name}`"
				);
			}
		}
	}
}

impl<T: Config> OnRingRootChange<MembersOf<T>> for Pallet<T> {
	fn on_ring_root_change(
		identifier: Identifier,
		ring_index: RingIndex,
		_op: RingRootOp<&MembersOf<T>>,
	) {
		let mut page = PageState::<T>::get().write_page;

		if PendingUpdates::<T>::contains_key((page, identifier, ring_index)) {
			return;
		}

		if PageUpdatesCount::<T>::get(page) >= T::MaxUpdatesPerBatch::get() {
			page = page.wrapping_add(1);
			PageState::<T>::mutate(|s| s.write_page = page);
		}

		PendingUpdates::<T>::insert((page, identifier, ring_index), ());
		PageUpdatesCount::<T>::mutate(page, |c| *c = c.saturating_add(1));
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn bench_setup_worst_case() {
		let write_page = PageState::<T>::get().write_page;
		PageUpdatesCount::<T>::insert(write_page, T::MaxUpdatesPerBatch::get());
	}
}
