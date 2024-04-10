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

use frame_support::{derive_impl, parameter_types};

use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	BuildStorage, MultiSignature,
};

type Signature = MultiSignature;
type AccountPublic = <Signature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system
	}
);

parameter_types! {
	pub const SS58Prefix: u8 = 29;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

#[allow(dead_code)]
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let t: sp_runtime::Storage =
		frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	#[cfg(feature = "runtime-benchmarks")]
	let keystore = sp_keystore::testing::MemoryKeystore::new();
	#[cfg(feature = "runtime-benchmarks")]
	ext.register_extension(sp_keystore::KeystoreExt(sp_std::sync::Arc::new(keystore)));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
