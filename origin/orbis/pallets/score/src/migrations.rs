//! Score introduction migration for the pre-Orbis state where the pallet prefix was absent.

use crate::{Config, ManagerAccount, Pallet, PayoutAccount, STORAGE_VERSION};
#[cfg(feature = "try-runtime")]
use alloc::vec::Vec;
#[cfg(feature = "try-runtime")]
use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::{
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
	weights::Weight,
};

pub struct IntroduceV1<T>(PhantomData<T>);

fn prefix_has_key<T: Config>() -> bool {
	let prefix = sp_io::hashing::twox_128(Pallet::<T>::name().as_bytes());
	sp_io::storage::next_key(&prefix).is_some_and(|key| key.starts_with(&prefix))
}

#[cfg(any(test, feature = "try-runtime"))]
fn prefix_key_count<T: Config>() -> u32 {
	let prefix = sp_io::hashing::twox_128(Pallet::<T>::name().as_bytes());
	let mut previous = prefix.to_vec();
	let mut count = 0u32;
	while let Some(key) = sp_io::storage::next_key(&previous).filter(|key| key.starts_with(&prefix))
	{
		previous = key;
		count = count.saturating_add(1);
	}
	count
}

impl<T: Config> OnRuntimeUpgrade for IntroduceV1<T> {
	fn on_runtime_upgrade() -> Weight {
		if Pallet::<T>::on_chain_storage_version() != StorageVersion::new(0) {
			return T::DbWeight::get().reads(1);
		}
		if prefix_has_key::<T>() {
			return T::DbWeight::get().reads(2);
		}
		if let Some(manager) = T::ManagerAccountDefault::get() {
			ManagerAccount::<T>::put(manager);
		}
		PayoutAccount::<T>::put(T::PayoutAccountDefault::get());
		STORAGE_VERSION.put::<Pallet<T>>();
		T::DbWeight::get().reads_writes(1, 3)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let version = Pallet::<T>::on_chain_storage_version();
		frame_support::ensure!(
			version <= STORAGE_VERSION,
			"Score storage version is newer than v1"
		);
		let keys = prefix_key_count::<T>();
		if version == StorageVersion::new(0) {
			frame_support::ensure!(keys == 0, "pre-Slice2 Score prefix must be absent");
		}
		Ok((version, keys).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let (before, keys): (StorageVersion, u32) =
			Decode::decode(&mut &state[..]).map_err(|_| "invalid Score migration state")?;
		frame_support::ensure!(
			Pallet::<T>::on_chain_storage_version() == STORAGE_VERSION,
			"Score v1 not installed"
		);
		frame_support::ensure!(
			PayoutAccount::<T>::get() == T::PayoutAccountDefault::get(),
			"Score payout default mismatch"
		);
		if before == StorageVersion::new(0) {
			let expected = 2 + u32::from(T::ManagerAccountDefault::get().is_some());
			frame_support::ensure!(
				keys == 0 && prefix_key_count::<T>() == expected,
				"unexpected Score v1 keys"
			);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::Test;

	#[test]
	fn absent_prefix_installs_exact_v1_defaults_and_is_idempotent() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			assert_eq!(prefix_key_count::<Test>(), 0);
			#[cfg(feature = "try-runtime")]
			let pre = IntroduceV1::<Test>::pre_upgrade().unwrap();
			IntroduceV1::<Test>::on_runtime_upgrade();
			assert_eq!(Pallet::<Test>::on_chain_storage_version(), STORAGE_VERSION);
			assert_eq!(ManagerAccount::<Test>::get(), Some(99));
			assert_eq!(PayoutAccount::<Test>::get(), 500);
			assert_eq!(prefix_key_count::<Test>(), 3);
			#[cfg(feature = "try-runtime")]
			IntroduceV1::<Test>::post_upgrade(pre).unwrap();
			let before = prefix_key_count::<Test>();
			IntroduceV1::<Test>::on_runtime_upgrade();
			assert_eq!(prefix_key_count::<Test>(), before);
		});
	}

	#[test]
	fn dirty_v0_prefix_freezes_without_mutation() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			ManagerAccount::<Test>::put(7);
			let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
			IntroduceV1::<Test>::on_runtime_upgrade();
			assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
			assert_eq!(Pallet::<Test>::on_chain_storage_version(), StorageVersion::new(0));
		});
	}
}
