// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019  BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

//! Portablegabi: Adds accumulator storage for usage in privacy enhancement
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap};
use frame_system::ensure_signed;
use sp_std::vec::Vec;

use error;

/// The pallet's configuration trait.
pub trait Trait: frame_system::Trait + error::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
		/// The AccumulatorList contains all accumulator. It is a map which
		/// maps an account id and an index to an accumulator
		AccumulatorList get(fn accumulator_list): map hasher(opaque_blake2_256) (T::AccountId, u64) => Option<Vec<u8>>;

		/// The AccumulatorCounter stores for each attester the number of
		/// accumulator updates.
		AccumulatorCount get(fn accumulator_count): map hasher(opaque_blake2_256) T::AccountId => u64;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		fn deposit_event() = default;

		/// Updates the attestation
		#[weight = 1]
		pub fn update_accumulator(origin, accumulator: Vec<u8>) -> DispatchResult {
			let attester = ensure_signed(origin)?;

			// if attester didn't store any accumulators, this will be 0
			// 0 is the default value for new keys.
			let counter = <AccumulatorCount<T>>::get(&attester);

			// new counter value
			let next = <error::Module<T>>::ok_or_deposit_err(
				counter.checked_add(1),
				Self::ERROR_OVERFLOW
			)?;

			// set bytes at index `counter` to accumulator
			// update counter to `next`
			if !<AccumulatorList<T>>::contains_key((attester.clone(), counter)) {
				<AccumulatorList<T>>::insert((attester.clone(), counter), &accumulator);
				<AccumulatorCount<T>>::insert(&attester, next);

				Self::deposit_event(RawEvent::Updated(attester, next, accumulator));
				Ok(())
			} else {
				Self::error(Self::ERROR_INCONSISTENT)
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub const ERROR_BASE: u16 = 4000;
	pub const ERROR_OVERFLOW: error::ErrorType = (Self::ERROR_BASE + 1, "accumulator overflow");
	pub const ERROR_INCONSISTENT: error::ErrorType =
		(Self::ERROR_BASE + 1, "inconsistent accumulator counter");

	/// Create an error using the error module
	pub fn error(error_type: error::ErrorType) -> DispatchResult {
		<error::Module<T>>::error(error_type)
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Trait>::AccountId,
	{
		/// An accumulator has been updated. Therefore an attestation has be revoked
		Updated(AccountId, u64, Vec<u8>),
	}
);

/// tests for this pallet
#[cfg(test)]
mod tests {
	use crate::*;
	use frame_support::{
		assert_ok, impl_outer_origin,
		weights::constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
	};
	use cord_node_runtime::{
		AvailableBlockRatio, BlockHashCount, MaximumBlockLength, MaximumBlockWeight,
		MaximumExtrinsicWeight,
	};
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	#[derive(Clone, Eq, PartialEq, Debug)]
	pub struct Test;

	impl frame_system::Trait for Test {
		type Origin = Origin;
		type Call = ();
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type BlockHashCount = BlockHashCount;
		type MaximumBlockWeight = MaximumBlockWeight;
		type DbWeight = RocksDbWeight;
		type BlockExecutionWeight = BlockExecutionWeight;
		type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
		type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
		type MaximumBlockLength = MaximumBlockLength;
		type AvailableBlockRatio = AvailableBlockRatio;
		type Version = ();

		type PalletInfo = ();
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type BaseCallFilter = ();
		type SystemWeightInfo = ();
	}

	impl Trait for Test {
		type Event = ();
	}

	impl error::Trait for Test {
		type Event = ();
		type ErrorCode = u16;
	}

	type PortablegabiModule = Module<Test>;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap()
			.into()
	}

	#[test]
	fn it_works_for_default_value() {
		new_test_ext().execute_with(|| {
			// Just a dummy test for the dummy function `do_something`
			// calling the `do_something` function with a value 42
			assert_ok!(PortablegabiModule::update_accumulator(
				Origin::signed(1),
				vec![1u8, 2u8, 3u8]
			));
			assert_ok!(PortablegabiModule::update_accumulator(
				Origin::signed(1),
				vec![4u8, 5u8, 6u8]
			));
			assert_ok!(PortablegabiModule::update_accumulator(
				Origin::signed(1),
				vec![7u8, 8u8, 9u8]
			));

			// There should be three accumulators inside the store
			assert_eq!(PortablegabiModule::accumulator_count(1), 3);

			// asserting that the stored value is equal to what we stored
			assert_eq!(
				PortablegabiModule::accumulator_list((1, 0)),
				Some(vec![1u8, 2u8, 3u8])
			);
			assert_eq!(
				PortablegabiModule::accumulator_list((1, 1)),
				Some(vec![4u8, 5u8, 6u8])
			);
			assert_eq!(
				PortablegabiModule::accumulator_list((1, 2)),
				Some(vec![7u8, 8u8, 9u8])
			);
		});
	}
}
