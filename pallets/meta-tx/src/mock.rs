// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Mock setup for tests.

#![cfg(any(test, feature = "runtime-benchmarks"))]

use crate as pallet_meta_tx;
use crate::*;
use frame_support::{
	construct_runtime, derive_impl,
	weights::{FixedFee, NoFee},
};
use sp_core::ConstU8;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup, Verify},
	MultiSignature,
};

pub type Balance = u64;

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

#[cfg(feature = "runtime-benchmarks")]
pub type MetaTxExtension = crate::benchmarking::types::WeightlessExtension<Runtime>;

#[cfg(not(feature = "runtime-benchmarks"))]
pub use tx_ext::*;

#[cfg(not(feature = "runtime-benchmarks"))]
mod tx_ext {
	use super::*;

	pub type UncheckedExtrinsic =
		sp_runtime::generic::UncheckedExtrinsic<AccountId, RuntimeCall, Signature, TxExtension>;

	/// Transaction extension.
	pub type TxExtension = (pallet_verify_signature::VerifySignature<Runtime>, TxBareExtension);

	/// Transaction extension without signature information.
	///
	/// Helper type used to decode the part of the extension which should be signed.
	pub type TxBareExtension = (
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckMortality<Runtime>,
		frame_system::CheckNonce<Runtime>,
		frame_system::CheckWeight<Runtime>,
		pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	);

	pub const META_EXTENSION_VERSION: ExtensionVersion = 0;

	/// Meta transaction extension.
	pub type MetaTxExtension =
		(pallet_verify_signature::VerifySignature<Runtime>, MetaTxBareExtension);

	/// Meta transaction extension without signature information.
	///
	/// Helper type used to decode the part of the extension which should be signed.
	pub type MetaTxBareExtension = (
		MetaTxMarker<Runtime>,
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckMortality<Runtime>,
		frame_system::CheckNonce<Runtime>,
	);
}

impl Config for Runtime {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type Extension = MetaTxExtension;
}

impl pallet_verify_signature::Config for Runtime {
	type Signature = MultiSignature;
	type AccountIdentifier = <Signature as Verify>::Signer;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type AccountData = pallet_balances::AccountData<<Self as pallet_balances::Config>::Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Runtime {
	type ReserveIdentifier = [u8; 8];
	type AccountStore = System;
}

pub const TX_FEE: u32 = 10;

impl pallet_transaction_payment::Config for Runtime {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<1>;
	type WeightToFee = FixedFee<TX_FEE, Balance>;
	type LengthToFee = NoFee<Balance>;
	type FeeMultiplierUpdate = ();
}

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		MetaTx: pallet_meta_tx,
		TxPayment: pallet_transaction_payment,
		VerifySignature: pallet_verify_signature,
	}
);

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = sp_io::TestExternalities::new(Default::default());
	ext.execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
	});
	ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
	ext
}
