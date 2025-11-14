// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use super::*;
use alloc::collections::BTreeMap;
use codec::Encode;
use cord_primitives::{AccountId, Signature};
use frame_support::{derive_impl, parameter_types, traits::PalletInfoAccess};
use pallet_entity::{signature::SignatureVerificationError, EntityLookup};
use pallet_token::{EventBlock, Token as TokenTrait};
use sp_core::{sr25519, Pair};
use sp_runtime::{
	traits::{BlakeTwo256, Hash as HashT, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage, MultiSigner,
};
use std::cell::RefCell;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Token: pallet_token,
		Register: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = frame_system::mocking::MockBlock<Self>;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = frame_support::traits::ConstU64<250>;
	type Hash = <BlakeTwo256 as HashT>::Output;
	type Hashing = BlakeTwo256;
	type BlockWeights = ();
	type BlockLength = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type AccountData = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type SystemWeightInfo = ();
	type SS58Prefix = frame_support::traits::ConstU16<38>;
	type OnSetCode = ();
}

impl pallet_token::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type MaxAuthorizationLen = MaxAuthorizationLen;
	type MaxTimelineViewResults = MaxTimelineViewResults;
	type DefaultTimelineViewResults = DefaultTimelineViewResults;
	type MaxAuthorizationTTL = MaxAuthorizationTTL;
}

parameter_types! {
	pub const MaxRawDataLength: u32 = 256;
	pub const MaxAdditionalAttributes: u32 = 8;
	pub const MaxAuthorizationLen: u32 = 64;
	pub const MaxTimelineViewResults: u32 = 16;
	pub const DefaultTimelineViewResults: u32 = 8;
	pub const MaxPacketListResults: u32 = 32;
	pub const MaxAuthorizationTTL: u32 = 30;
}

pub struct MockLookup;

thread_local! {
	pub(crate) static ACCOUNT_TOKENS: RefCell<BTreeMap<AccountId, Ss58Identifier>> = RefCell::new(BTreeMap::new());
	pub(crate) static ACCOUNT_KEYS: RefCell<BTreeMap<AccountId, sr25519::Pair>> = RefCell::new(BTreeMap::new());
}

impl EntityLookup<Test> for MockLookup {
	type Error = ();
	type EntityNym = ();

	fn lookup_token_of(account: &AccountId) -> Result<Ss58Identifier, Self::Error> {
		ACCOUNT_TOKENS.with(|map| map.borrow().get(account).cloned()).ok_or(())
	}

	fn lookup_controller_of(_token: &Ss58Identifier) -> Result<AccountId, Self::Error> {
		Err(())
	}

	fn lookup_history(_token: &Ss58Identifier) -> Vec<(AccountId, EventBlock)> {
		Vec::new()
	}

	fn lookup_nym_of_identifier(_token: &Ss58Identifier) -> Option<Self::EntityNym> {
		None
	}

	fn lookup_identifier_of_nym(_name: &Self::EntityNym) -> Option<Ss58Identifier> {
		None
	}

	fn verify_account_signature(
		account: &AccountId,
		payload: &[u8],
		signature: &Signature,
	) -> Result<Ss58Identifier, SignatureVerificationError> {
		let token = ACCOUNT_TOKENS
			.with(|map| map.borrow().get(account).cloned())
			.ok_or(SignatureVerificationError::SignerInformationNotPresent)?;
		let signer: sp_runtime::AccountId32 = account.clone().into();
		if signature.verify(payload, &signer) {
			Ok(token)
		} else {
			Err(SignatureVerificationError::SignatureInvalid)
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::EntityBinder<Test> for MockLookup {
	fn bind_account(account: &AccountId, token: &Ss58Identifier) {
		ACCOUNT_TOKENS.with(|map| {
			map.borrow_mut().insert(account.clone(), token.clone());
		});
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Token = pallet_token::Pallet<Self>;
	type EntityLookup = MockLookup;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type MaxAuthorizationLen = MaxAuthorizationLen;
	type MaxAuthorizationTTL = MaxAuthorizationTTL;
	type MaxPacketListResults = MaxPacketListResults;
	type Feeless = ();
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn account(seed: u8) -> AccountId {
	let pair = sr25519::Pair::from_seed(&[seed; 32]);
	let signer = MultiSigner::from(pair.public());
	let account: AccountId = signer.into_account();
	ACCOUNT_KEYS.with(|keys| {
		keys.borrow_mut().insert(account.clone(), pair);
	});
	account
}

pub fn bind_account(account: AccountId) -> Ss58Identifier {
	let encoded = account.encode();
	let hash = <Test as frame_system::Config>::Hashing::hash(&encoded);
	let pallet = <Pallet<Test> as PalletInfoAccess>::name();
	let token = <pallet_token::Pallet<Test> as TokenTrait<Test>>::build(hash.as_ref(), pallet)
		.expect("token generation never fails in tests");
	ACCOUNT_TOKENS.with(|map| {
		map.borrow_mut().insert(account.clone(), token.clone());
	});
	token
}
