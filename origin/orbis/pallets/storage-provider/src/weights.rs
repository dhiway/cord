// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

/// Weight contract for the Orbis storage-provider pallet.
///
/// These conservative initial weights keep the feature executable. They must be replaced by the
/// final P7 benchmark output before production activation.
pub trait WeightInfo {
	fn register_provider() -> Weight;
	fn update_provider() -> Weight;
	fn set_provider_status() -> Weight;
	fn remove_provider() -> Weight;
	fn heartbeat() -> Weight;
	fn propose_agreement() -> Weight;
	fn accept_agreement() -> Weight;
	fn cancel_agreement() -> Weight;
	fn issue_challenge() -> Weight;
	fn submit_checkpoint() -> Weight;
	fn timeout_challenge() -> Weight;
	fn request_renewal() -> Weight;
	fn accept_renewal() -> Weight;
	fn expire_agreement() -> Weight;
	fn prune_agreement() -> Weight;
	fn acknowledge_deletion() -> Weight;
	fn commit_provider_root(leaves: u32) -> Weight;
}

impl WeightInfo for () {
	fn register_provider() -> Weight {
		Weight::from_parts(35_000_000, 8_000)
	}
	fn update_provider() -> Weight {
		Weight::from_parts(25_000_000, 6_000)
	}
	fn set_provider_status() -> Weight {
		Weight::from_parts(20_000_000, 5_000)
	}
	fn remove_provider() -> Weight {
		Weight::from_parts(25_000_000, 7_000)
	}
	fn heartbeat() -> Weight {
		Weight::from_parts(15_000_000, 4_000)
	}
	fn propose_agreement() -> Weight {
		Weight::from_parts(45_000_000, 12_000)
	}
	fn accept_agreement() -> Weight {
		Weight::from_parts(45_000_000, 12_000)
	}
	fn cancel_agreement() -> Weight {
		Weight::from_parts(40_000_000, 11_000)
	}
	fn issue_challenge() -> Weight {
		Weight::from_parts(35_000_000, 10_000)
	}
	fn submit_checkpoint() -> Weight {
		Weight::from_parts(40_000_000, 12_000)
	}
	fn timeout_challenge() -> Weight {
		Weight::from_parts(35_000_000, 10_000)
	}
	fn request_renewal() -> Weight {
		Weight::from_parts(20_000_000, 6_000)
	}
	fn accept_renewal() -> Weight {
		Weight::from_parts(25_000_000, 7_000)
	}
	fn expire_agreement() -> Weight {
		Weight::from_parts(35_000_000, 10_000)
	}
	fn prune_agreement() -> Weight {
		Weight::from_parts(40_000_000, 12_000)
	}
	fn acknowledge_deletion() -> Weight {
		Weight::from_parts(55_000_000, 12_000)
	}
	fn commit_provider_root(leaves: u32) -> Weight {
		Weight::from_parts(25_000_000, 7_000)
			.saturating_add(Weight::from_parts(3_000_000, 512).saturating_mul(leaves.into()))
	}
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn register_provider() -> Weight {
		w::<T>(35_000_000, 8_000, 3, 3)
	}
	fn update_provider() -> Weight {
		w::<T>(25_000_000, 6_000, 2, 1)
	}
	fn set_provider_status() -> Weight {
		w::<T>(20_000_000, 5_000, 1, 1)
	}
	fn remove_provider() -> Weight {
		w::<T>(25_000_000, 7_000, 3, 3)
	}
	fn heartbeat() -> Weight {
		w::<T>(15_000_000, 4_000, 1, 1)
	}
	fn propose_agreement() -> Weight {
		w::<T>(45_000_000, 12_000, 7, 6)
	}
	fn accept_agreement() -> Weight {
		w::<T>(45_000_000, 12_000, 7, 5)
	}
	fn cancel_agreement() -> Weight {
		w::<T>(40_000_000, 11_000, 6, 5)
	}
	fn issue_challenge() -> Weight {
		w::<T>(35_000_000, 10_000, 5, 4)
	}
	fn submit_checkpoint() -> Weight {
		w::<T>(40_000_000, 12_000, 5, 4)
	}
	fn timeout_challenge() -> Weight {
		w::<T>(35_000_000, 10_000, 5, 4)
	}
	fn request_renewal() -> Weight {
		w::<T>(20_000_000, 6_000, 2, 1)
	}
	fn accept_renewal() -> Weight {
		w::<T>(25_000_000, 7_000, 3, 1)
	}
	fn expire_agreement() -> Weight {
		w::<T>(35_000_000, 10_000, 5, 4)
	}
	fn prune_agreement() -> Weight {
		w::<T>(40_000_000, 12_000, 7, 7)
	}
	fn acknowledge_deletion() -> Weight {
		w::<T>(55_000_000, 12_000, 5, 3)
	}
	fn commit_provider_root(leaves: u32) -> Weight {
		w::<T>(25_000_000, 7_000, 2, 1)
			.saturating_add(Weight::from_parts(3_000_000, 512).saturating_mul(leaves.into()))
	}
}

fn w<T: frame_system::Config>(ref_time: u64, proof: u64, reads: u64, writes: u64) -> Weight {
	Weight::from_parts(ref_time, proof)
		.saturating_add(T::DbWeight::get().reads(reads))
		.saturating_add(T::DbWeight::get().writes(writes))
}
