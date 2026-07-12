//! Honour introduction migration for the pre-Orbis state where the pallet prefix was absent.

use crate::{Config, Pallet, STORAGE_VERSION};
#[cfg(feature = "try-runtime")]
use alloc::vec::Vec;
#[cfg(feature = "try-runtime")]
use codec::{Decode, Encode};
use core::marker::PhantomData;
#[cfg(any(test, feature = "try-runtime"))]
use frame_support::traits::PalletInfoAccess;
use frame_support::{
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion},
	weights::Weight,
};

pub struct IntroduceV1<T>(PhantomData<T>);

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
		STORAGE_VERSION.put::<Pallet<T>>();
		T::DbWeight::get().reads_writes(1, 1)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let version = Pallet::<T>::on_chain_storage_version();
		frame_support::ensure!(
			version <= STORAGE_VERSION,
			"Honour storage version is newer than v1"
		);
		let keys = prefix_key_count::<T>();
		if version == StorageVersion::new(0) {
			frame_support::ensure!(keys == 0, "pre-Slice2 Honour prefix must be absent");
		}
		Ok((version, keys).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let (before, keys): (StorageVersion, u32) =
			Decode::decode(&mut &state[..]).map_err(|_| "invalid Honour migration state")?;
		frame_support::ensure!(
			Pallet::<T>::on_chain_storage_version() == STORAGE_VERSION,
			"Honour v1 not installed"
		);
		if before == StorageVersion::new(0) {
			frame_support::ensure!(
				keys == 0 && prefix_key_count::<T>() == 1,
				"unexpected Honour v1 keys"
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
	fn absent_prefix_installs_only_v1_version_and_is_idempotent() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			assert_eq!(prefix_key_count::<Test>(), 0);
			#[cfg(feature = "try-runtime")]
			let pre = IntroduceV1::<Test>::pre_upgrade().unwrap();
			IntroduceV1::<Test>::on_runtime_upgrade();
			assert_eq!(Pallet::<Test>::on_chain_storage_version(), STORAGE_VERSION);
			assert_eq!(prefix_key_count::<Test>(), 1);
			#[cfg(feature = "try-runtime")]
			IntroduceV1::<Test>::post_upgrade(pre).unwrap();
			IntroduceV1::<Test>::on_runtime_upgrade();
			assert_eq!(prefix_key_count::<Test>(), 1);
		});
	}
}
