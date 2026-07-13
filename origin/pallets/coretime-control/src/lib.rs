// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replay-safe control envelope for Orbis Broker requests to Origin Coretime.
//!
//! The upstream Broker/Coretime interfaces intentionally remain unchanged. This pallet wraps
//! their cross-chain transport with a monotonic request ID, bounded status history, strict
//! in-order provider application and replay receipts. A duplicate transport delivery is
//! acknowledged but never applied twice.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResult, traits::Get, weights::Weight};
use scale_info::TypeInfo;

pub type RequestId = u64;
pub type CoreCount = u16;

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	Debug,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum ReceiptStatus {
	Accepted,
	Duplicate,
	OutOfOrder,
	Conflict,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	Debug,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct RequestReceipt {
	pub id: RequestId,
	pub count: CoreCount,
	pub status: ReceiptStatus,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	Debug,
	PartialEq,
	Eq,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct OutboundRecord {
	pub count: CoreCount,
	pub status: Option<ReceiptStatus>,
}

/// Orbis-side XCM adapter. A successful return means the message was accepted by the local
/// transport, not that Origin applied it; final state comes from `acknowledge`.
pub trait SendRequest {
	fn send(id: RequestId, count: CoreCount) -> DispatchResult;
}
impl SendRequest for () {
	fn send(_: RequestId, _: CoreCount) -> DispatchResult {
		Ok(())
	}
}

/// Origin-side adapter to the upstream Coretime request implementation.
pub trait ApplyRequest {
	fn apply(count: CoreCount) -> DispatchResult;
}
impl ApplyRequest for () {
	fn apply(_: CoreCount) -> DispatchResult {
		Ok(())
	}
}

/// Origin-side best-effort receipt transport. Failed receipt delivery is recoverable because a
/// duplicate request deterministically regenerates the same receipt.
pub trait SendReceipt {
	fn send(receipt: RequestReceipt);
}
impl SendReceipt for () {
	fn send(_: RequestReceipt) {}
}

pub trait WeightInfo {
	fn request() -> Weight;
	fn retry() -> Weight;
	fn submit() -> Weight;
	fn acknowledge() -> Weight;
	fn set_transport_hold() -> Weight;
	fn release_held() -> Weight;
}

impl WeightInfo for () {
	fn request() -> Weight {
		Weight::zero()
	}
	fn retry() -> Weight {
		Weight::zero()
	}
	fn submit() -> Weight {
		Weight::zero()
	}
	fn acknowledge() -> Weight {
		Weight::zero()
	}
	fn set_transport_hold() -> Weight {
		Weight::zero()
	}
	fn release_held() -> Weight {
		Weight::zero()
	}
}

pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn request() -> Weight {
		Weight::from_parts(50_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	fn retry() -> Weight {
		Weight::from_parts(45_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn submit() -> Weight {
		// Includes conservative headroom for applying the upstream Coretime request and sending
		// the receipt XCM. Replace with generated weights before production activation.
		Weight::from_parts(150_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	fn acknowledge() -> Weight {
		Weight::from_parts(40_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_transport_hold() -> Weight {
		Weight::from_parts(10_000_000, 1_000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn release_held() -> Weight {
		Weight::from_parts(45_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{ensure, pallet_prelude::*, traits::EnsureOrigin};
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		/// Orbis governance/operator origin allowed to create a new request.
		type RequestOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Origin-side para-1006 origin allowed to submit an envelope.
		type BrokerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Orbis-side parent/Origin XCM origin allowed to submit a receipt.
		type ReceiptOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Fast-runtime operator origin for deterministic transport hold/release evidence.
		type TransportControlOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Must be false in production runtimes. The calls remain visible but fail closed.
		#[pallet::constant]
		type TransportControlEnabled: Get<bool>;
		type RequestSender: SendRequest;
		type RequestApplier: ApplyRequest;
		type ReceiptSender: SendReceipt;
		#[pallet::constant]
		type MaxTrackedRequests: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type NextRequestId<T: Config> = StorageValue<_, RequestId, ValueQuery>;

	#[pallet::storage]
	pub type Outbound<T: Config> =
		StorageMap<_, Twox64Concat, RequestId, OutboundRecord, OptionQuery>;

	#[pallet::storage]
	pub type TrackedOutbound<T: Config> =
		StorageValue<_, BoundedVec<RequestId, T::MaxTrackedRequests>, ValueQuery>;

	/// Fast-runtime-only deterministic transport gate. Held request IDs remain bounded by
	/// `TrackedOutbound`; release always reuses the original request envelope.
	#[pallet::storage]
	pub type TransportHeld<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::storage]
	pub type HeldOutbound<T: Config> = StorageMap<_, Twox64Concat, RequestId, (), OptionQuery>;

	#[pallet::storage]
	pub type LastApplied<T: Config> = StorageValue<_, (RequestId, CoreCount), OptionQuery>;

	/// Bounded provider-side replay ledger. Once an entry is pruned, an old delivery remains
	/// fail-closed as `OutOfOrder`, but can no longer be distinguished as `Duplicate`.
	#[pallet::storage]
	pub type Applied<T: Config> = StorageMap<_, Twox64Concat, RequestId, CoreCount, OptionQuery>;

	#[pallet::storage]
	pub type TrackedApplied<T: Config> =
		StorageValue<_, BoundedVec<RequestId, T::MaxTrackedRequests>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RequestSent { id: RequestId, count: CoreCount },
		RequestHeld { id: RequestId, count: CoreCount },
		HeldRequestReleased { id: RequestId, count: CoreCount },
		TransportHoldChanged { held: bool },
		RequestRetried { id: RequestId, count: CoreCount },
		RequestStatus { id: RequestId, count: CoreCount, status: ReceiptStatus },
		ReceiptRecorded { id: RequestId, count: CoreCount, status: ReceiptStatus },
		OutboundPruned { id: RequestId },
		AppliedPruned { id: RequestId },
	}

	#[pallet::error]
	pub enum Error<T> {
		RequestIdExhausted,
		NoTrackingCapacity,
		UnknownRequest,
		UnknownReceipt,
		ReceiptCountMismatch,
		ReceiptStatusConflict,
		TransportControlDisabled,
		RequestNotHeld,
		HeldReleaseOutOfOrder,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create one new monotonic request.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::request())]
		pub fn request_core_count(origin: OriginFor<T>, count: CoreCount) -> DispatchResult {
			T::RequestOrigin::ensure_origin(origin)?;
			Self::send_request(count)
		}

		/// Apply an in-order request once, or deterministically acknowledge a replay/fault without
		/// applying it. `BrokerOrigin` must be wired to para 1006 (or Root in tests).
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::submit())]
		#[frame_support::transactional]
		pub fn submit_request(
			origin: OriginFor<T>,
			id: RequestId,
			count: CoreCount,
		) -> DispatchResult {
			T::BrokerOrigin::ensure_origin(origin)?;
			let status = match Applied::<T>::get(id) {
				Some(applied_count) if applied_count == count => ReceiptStatus::Duplicate,
				Some(_) => ReceiptStatus::Conflict,
				None => match LastApplied::<T>::get() {
					None if id == 0 => {
						T::RequestApplier::apply(count)?;
						Self::track_applied(id, count)?;
						LastApplied::<T>::put((id, count));
						ReceiptStatus::Accepted
					},
					None => ReceiptStatus::OutOfOrder,
					Some((last_id, _)) if id == last_id.saturating_add(1) => {
						T::RequestApplier::apply(count)?;
						Self::track_applied(id, count)?;
						LastApplied::<T>::put((id, count));
						ReceiptStatus::Accepted
					},
					Some((last_id, _)) if id < last_id => ReceiptStatus::OutOfOrder,
					Some(_) => ReceiptStatus::OutOfOrder,
				},
			};
			let receipt = RequestReceipt { id, count, status };
			Self::deposit_event(Event::RequestStatus { id, count, status });
			T::ReceiptSender::send(receipt);
			Ok(())
		}

		/// Record a receipt from Origin. Receipts may arrive out of order; request application may
		/// not. A count mismatch fails closed.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::acknowledge())]
		pub fn acknowledge(
			origin: OriginFor<T>,
			id: RequestId,
			count: CoreCount,
			status: ReceiptStatus,
		) -> DispatchResult {
			T::ReceiptOrigin::ensure_origin(origin)?;
			let recorded_status =
				Outbound::<T>::try_mutate(id, |record| -> Result<ReceiptStatus, DispatchError> {
					let record = record.as_mut().ok_or(Error::<T>::UnknownReceipt)?;
					ensure!(record.count == count, Error::<T>::ReceiptCountMismatch);
					let status = match (record.status, status) {
						// Accepted and Duplicate are equivalent terminal evidence that Origin
						// applied this exact request. Never let a delayed negative receipt
						// downgrade them.
						(
							Some(ReceiptStatus::Accepted | ReceiptStatus::Duplicate),
							ReceiptStatus::Accepted | ReceiptStatus::Duplicate,
						) => ReceiptStatus::Accepted,
						(Some(ReceiptStatus::Accepted | ReceiptStatus::Duplicate), _) => {
							return Err(Error::<T>::ReceiptStatusConflict.into())
						},
						(Some(ReceiptStatus::Conflict), ReceiptStatus::Conflict) => {
							ReceiptStatus::Conflict
						},
						(Some(ReceiptStatus::Conflict), _) => {
							return Err(Error::<T>::ReceiptStatusConflict.into())
						},
						// OutOfOrder is explicitly recoverable by retrying the same request ID.
						_ => status,
					};
					record.status = Some(status);
					Ok(status)
				})?;
			Self::deposit_event(Event::ReceiptRecorded { id, count, status: recorded_status });
			Ok(())
		}

		/// Replay an existing bounded outbound record with the same request ID. This recovers from
		/// delay, reordering, lost receipts and provider restarts without allocating a new nonce.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::retry())]
		pub fn retry_request(origin: OriginFor<T>, id: RequestId) -> DispatchResult {
			T::RequestOrigin::ensure_origin(origin)?;
			let record = Outbound::<T>::get(id).ok_or(Error::<T>::UnknownRequest)?;
			if T::TransportControlEnabled::get() && TransportHeld::<T>::get() {
				HeldOutbound::<T>::insert(id, ());
				Self::deposit_event(Event::RequestHeld { id, count: record.count });
				return Ok(());
			}
			T::RequestSender::send(id, record.count)?;
			Self::deposit_event(Event::RequestRetried { id, count: record.count });
			Ok(())
		}

		/// Enable or disable the deterministic request transport gate. Disabling never sends
		/// implicitly; each retained envelope must be released explicitly and remains auditable.
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::set_transport_hold())]
		pub fn set_transport_hold(origin: OriginFor<T>, held: bool) -> DispatchResult {
			T::TransportControlOrigin::ensure_origin(origin)?;
			ensure!(T::TransportControlEnabled::get(), Error::<T>::TransportControlDisabled);
			TransportHeld::<T>::put(held);
			Self::deposit_event(Event::TransportHoldChanged { held });
			Ok(())
		}

		/// Release one exact held request through the normal runtime-owned XCM sender.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::release_held())]
		#[frame_support::transactional]
		pub fn release_held(origin: OriginFor<T>, id: RequestId) -> DispatchResult {
			T::TransportControlOrigin::ensure_origin(origin)?;
			ensure!(T::TransportControlEnabled::get(), Error::<T>::TransportControlDisabled);
			ensure!(HeldOutbound::<T>::contains_key(id), Error::<T>::RequestNotHeld);
			let first = TrackedOutbound::<T>::get()
				.into_iter()
				.find(|candidate| HeldOutbound::<T>::contains_key(candidate));
			ensure!(first == Some(id), Error::<T>::HeldReleaseOutOfOrder);
			let record = Outbound::<T>::get(id).ok_or(Error::<T>::UnknownRequest)?;
			T::RequestSender::send(id, record.count)?;
			HeldOutbound::<T>::remove(id);
			Self::deposit_event(Event::HeldRequestReleased { id, count: record.count });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Runtime-internal entry used by `pallet_broker::CoretimeInterface` on Orbis.
		#[frame_support::transactional]
		pub fn send_request(count: CoreCount) -> DispatchResult {
			let id = NextRequestId::<T>::get();
			let next = id.checked_add(1).ok_or(Error::<T>::RequestIdExhausted)?;
			Self::track(id, OutboundRecord { count, status: None })?;
			NextRequestId::<T>::put(next);
			if T::TransportControlEnabled::get() && TransportHeld::<T>::get() {
				HeldOutbound::<T>::insert(id, ());
				Self::deposit_event(Event::RequestHeld { id, count });
			} else {
				T::RequestSender::send(id, count)?;
				Self::deposit_event(Event::RequestSent { id, count });
			}
			Ok(())
		}

		fn track(id: RequestId, record: OutboundRecord) -> DispatchResult {
			TrackedOutbound::<T>::try_mutate(|ids| -> DispatchResult {
				if ids.len() == T::MaxTrackedRequests::get() as usize {
					// Never evict a pending or OutOfOrder request: doing so could remove the only
					// request ID capable of unblocking strict provider sequencing.
					let completed = ids.first().copied().ok_or(Error::<T>::NoTrackingCapacity)?;
					let is_terminal = Outbound::<T>::get(completed).is_some_and(|record| {
						matches!(
							record.status,
							Some(
								ReceiptStatus::Accepted
									| ReceiptStatus::Duplicate | ReceiptStatus::Conflict
							)
						)
					});
					ensure!(is_terminal, Error::<T>::NoTrackingCapacity);
					ids.remove(0);
					Outbound::<T>::remove(completed);
					Self::deposit_event(Event::OutboundPruned { id: completed });
				}
				ids.try_push(id).map_err(|_| Error::<T>::NoTrackingCapacity)?;
				Outbound::<T>::insert(id, record);
				Ok(())
			})
		}

		fn track_applied(id: RequestId, count: CoreCount) -> DispatchResult {
			TrackedApplied::<T>::try_mutate(|ids| -> DispatchResult {
				if ids.len() == T::MaxTrackedRequests::get() as usize {
					let oldest = ids.first().copied().ok_or(Error::<T>::NoTrackingCapacity)?;
					ids.remove(0);
					Applied::<T>::remove(oldest);
					Self::deposit_event(Event::AppliedPruned { id: oldest });
				}
				ids.try_push(id).map_err(|_| Error::<T>::NoTrackingCapacity)?;
				Applied::<T>::insert(id, count);
				Ok(())
			})
		}
	}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
