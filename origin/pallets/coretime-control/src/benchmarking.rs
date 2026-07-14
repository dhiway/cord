// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_support::traits::EnsureOrigin;

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn request_core_count() -> Result<(), BenchmarkError> {
		let max = T::MaxTrackedRequests::get();
		if max == 0 {
			return Err(BenchmarkError::Weightless);
		}
		let count = 3;
		for raw_id in 0..max {
			let id = RequestId::from(raw_id);
			Outbound::<T>::insert(
				id,
				OutboundRecord { count, status: Some(ReceiptStatus::Accepted) },
			);
			TrackedOutbound::<T>::try_mutate(|ids| ids.try_push(id))
				.map_err(|_| BenchmarkError::Weightless)?;
			HeldRequestIds::<T>::try_mutate(|ids| ids.try_push(id))
				.map_err(|_| BenchmarkError::Weightless)?;
			HeldOutbound::<T>::insert(id, ());
		}
		let id = RequestId::from(max);
		NextRequestId::<T>::put(id);
		TransportHeld::<T>::put(true);
		let origin =
			T::RequestOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, count);

		assert_eq!(NextRequestId::<T>::get(), id + 1);
		assert!(!Outbound::<T>::contains_key(0));
		assert_eq!(Outbound::<T>::get(id).map(|record| record.count), Some(count));
		assert!(HeldOutbound::<T>::contains_key(id));
		Ok(())
	}

	#[benchmark]
	fn submit_request() -> Result<(), BenchmarkError> {
		let max = T::MaxTrackedRequests::get();
		if max == 0 {
			return Err(BenchmarkError::Weightless);
		}
		let count = 3;
		for raw_id in 0..max {
			let id = RequestId::from(raw_id);
			Applied::<T>::insert(id, count);
			TrackedApplied::<T>::try_mutate(|ids| ids.try_push(id))
				.map_err(|_| BenchmarkError::Weightless)?;
		}
		LastApplied::<T>::put((RequestId::from(max - 1), count));
		let id = RequestId::from(max);
		let origin =
			T::BrokerOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, id, count);

		assert_eq!(Applied::<T>::get(id), Some(count));
		assert_eq!(LastApplied::<T>::get(), Some((id, count)));
		assert!(!Applied::<T>::contains_key(0));
		Ok(())
	}

	#[benchmark]
	fn acknowledge() -> Result<(), BenchmarkError> {
		let max = T::MaxTrackedRequests::get();
		if max == 0 {
			return Err(BenchmarkError::Weightless);
		}
		for raw_id in 0..max {
			let held_id = RequestId::from(raw_id);
			HeldRequestIds::<T>::try_mutate(|ids| ids.try_push(held_id))
				.map_err(|_| BenchmarkError::Weightless)?;
			HeldOutbound::<T>::insert(held_id, ());
		}
		let id = 0;
		let count = 3;
		Outbound::<T>::insert(id, OutboundRecord { count, status: None });
		let origin =
			T::ReceiptOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, id, count, ReceiptStatus::Accepted);

		assert_eq!(
			Outbound::<T>::get(id).and_then(|record| record.status),
			Some(ReceiptStatus::Accepted),
		);
		assert!(!HeldOutbound::<T>::contains_key(id));
		assert_eq!(HeldRequestIds::<T>::get().len(), max.saturating_sub(1) as usize);
		Ok(())
	}

	#[benchmark]
	fn retry_request() -> Result<(), BenchmarkError> {
		let max = T::MaxTrackedRequests::get();
		if max == 0 {
			return Err(BenchmarkError::Weightless);
		}
		for raw_id in 0..max.saturating_sub(1) {
			let held_id = RequestId::from(raw_id);
			HeldRequestIds::<T>::try_mutate(|ids| ids.try_push(held_id))
				.map_err(|_| BenchmarkError::Weightless)?;
			HeldOutbound::<T>::insert(held_id, ());
		}
		let id = RequestId::from(max);
		let count = 3;
		Outbound::<T>::insert(id, OutboundRecord { count, status: None });
		TransportHeld::<T>::put(true);
		let origin =
			T::RequestOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, id);

		assert_eq!(Outbound::<T>::get(id).map(|record| record.count), Some(count));
		assert!(HeldOutbound::<T>::contains_key(id));
		assert_eq!(HeldRequestIds::<T>::get().last().copied(), Some(id));
		Ok(())
	}

	#[benchmark]
	fn set_transport_hold() -> Result<(), BenchmarkError> {
		let origin = T::TransportControlOrigin::try_successful_origin()
			.map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, true);

		assert!(TransportHeld::<T>::get());
		Ok(())
	}

	#[benchmark]
	fn release_held() -> Result<(), BenchmarkError> {
		let max = T::MaxTrackedRequests::get();
		if max == 0 {
			return Err(BenchmarkError::Weightless);
		}
		let count = 3;
		for raw_id in 0..max {
			let held_id = RequestId::from(raw_id);
			Outbound::<T>::insert(held_id, OutboundRecord { count, status: None });
			HeldRequestIds::<T>::try_mutate(|ids| ids.try_push(held_id))
				.map_err(|_| BenchmarkError::Weightless)?;
			HeldOutbound::<T>::insert(held_id, ());
		}
		let id = 0;
		TransportHeld::<T>::put(true);
		let origin = T::TransportControlOrigin::try_successful_origin()
			.map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, id);

		assert!(!HeldOutbound::<T>::contains_key(id));
		assert_eq!(HeldRequestIds::<T>::get().len(), max.saturating_sub(1) as usize);
		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
