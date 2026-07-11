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

use crate::{
	xcm_config::LocationToAccountId, Assets, Balances, Broker, Entity, Feeless, People, Revive,
	Runtime, RuntimeCall, RuntimeOrigin, System, TransactionStorage,
};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::CheckIfFeeless,
	traits::{fungible::Mutate, Contains, Get, Hooks, PalletInfoAccess},
};
use pallet_broker::{CoreAssignment, CoreMask, Reservations, Schedule, ScheduleItem};
use polkadot_primitives::AccountId;
use sp_core::crypto::Ss58Codec;
use xcm::prelude::*;
use xcm_runtime_apis::conversions::LocationToAccountHelper;

const ALICE: [u8; 32] = [1u8; 32];

fn decode_hex(input: &str) -> Vec<u8> {
	let input = input.trim();
	assert_eq!(input.len() % 2, 0);
	input
		.as_bytes()
		.chunks_exact(2)
		.map(|pair| {
			let digit = |byte: u8| match byte {
				b'0'..=b'9' => byte - b'0',
				b'a'..=b'f' => byte - b'a' + 10,
				b'A'..=b'F' => byte - b'A' + 10,
				_ => panic!("invalid fixture hex"),
			};
			(digit(pair[0]) << 4) | digit(pair[1])
		})
		.collect()
}

#[test]
fn enterprise_asset_can_be_created_and_managed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let asset_id = 7u32;

		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			asset_id.into(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner),
			asset_id.into(),
			beneficiary.clone().into(),
			1_000,
		));

		assert_eq!(Assets::balance(asset_id, beneficiary), 1_000);
	});
}

#[test]
fn revive_uses_reserved_orbis_evm_chain_id() {
	assert_eq!(<<Runtime as pallet_revive::Config>::ChainId as Get<u64>>::get(), 420_001_006);
	assert!(<<Runtime as pallet_revive::Config>::AllowEVMBytecode as Get<bool>>::get());
	assert_eq!(<Assets as PalletInfoAccess>::index(), 80);
	assert_eq!(<Revive as PalletInfoAccess>::index(), 100);
	// The SDK relay Coretime pallet encodes callbacks to Broker at index 50.
	assert_eq!(<Broker as PalletInfoAccess>::index(), 50);
	assert_eq!(<Entity as PalletInfoAccess>::index(), 53);
	assert_eq!(<People as PalletInfoAccess>::index(), 90);
	assert_eq!(<TransactionStorage as PalletInfoAccess>::index(), 110);
	assert_eq!(<<Runtime as pallet_broker::Config>::MaxReservedCores as Get<u32>>::get(), 50);
}

#[test]
fn solidity_evm_fixture_deploys_and_executes_through_revive() {
	use pallet_revive::{
		test_utils::builder::{BareCallBuilder, BareInstantiateBuilder},
		Code, TransactionLimits,
	};
	let limits = || TransactionLimits::WeightAndDeposit {
		weight_limit: frame_support::weights::Weight::from_parts(500_000_000_000, 10 * 1024 * 1024),
		deposit_limit: 50_000_000_000_000_000,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let account = pallet_revive::test_utils::ALICE;
		let funded =
			<Balances as Mutate<AccountId>>::set_balance(&account, 100_000_000_000_000_000);
		assert_eq!(funded, 100_000_000_000_000_000);
		assert_eq!(Balances::free_balance(&account), funded);
		let revive_account = Revive::account_id();
		<Balances as Mutate<AccountId>>::set_balance(
			&revive_account,
			crate::ExistentialDeposit::get(),
		);
		let code = decode_hex(include_str!("../fixtures/build/Counter.bin"));

		let instantiate = BareInstantiateBuilder::<Runtime>::bare_instantiate(
			RuntimeOrigin::signed(account.clone()),
			Code::Upload(code),
		)
		.transaction_limits(limits())
		.salt(Some([7u8; 32]))
		.build();
		let instantiated = instantiate.result.unwrap();
		assert!(!instantiated.result.did_revert());
		let contract_addr = instantiated.addr;

		let increment = sp_io::hashing::keccak_256(b"increment()")[..4].to_vec();
		let increment_result = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(account.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(increment)
		.build_and_unwrap_result();
		assert!(!increment_result.did_revert());

		let value = sp_io::hashing::keccak_256(b"value()")[..4].to_vec();
		let value_result =
			BareCallBuilder::<Runtime>::bare_call(RuntimeOrigin::signed(account), contract_addr)
				.transaction_limits(limits())
				.data(value)
				.build_and_unwrap_result();
		assert!(!value_result.did_revert());
		assert_eq!(value_result.data.len(), 32);
		assert_eq!(value_result.data[31], 42);
	});
}

#[test]
fn identity_bound_contract_moves_assets_and_persists_its_audit() {
	use pallet_revive::{
		test_utils::builder::{BareCallBuilder, BareInstantiateBuilder},
		AddressMapper, Code, TransactionLimits,
	};
	let limits = || TransactionLimits::WeightAndDeposit {
		weight_limit: frame_support::weights::Weight::from_parts(500_000_000_000, 10 * 1024 * 1024),
		deposit_limit: 50_000_000_000_000_000,
	};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let owner = pallet_revive::test_utils::ALICE;
		let recipient = pallet_revive::test_utils::BOB;
		<Balances as Mutate<AccountId>>::set_balance(&owner, 100_000_000_000_000_000);
		<Balances as Mutate<AccountId>>::set_balance(&recipient, crate::ExistentialDeposit::get());
		<Balances as Mutate<AccountId>>::set_balance(
			&Revive::account_id(),
			crate::ExistentialDeposit::get(),
		);

		let mut identity =
			pallet_cord_identity::legacy::IdentityInfo::<crate::PeopleMaxAdditionalFields>::default(
			);
		identity.display =
			pallet_cord_identity::Data::Raw(b"Alice Orbis".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(identity),
		));
		assert!(People::has_identity(&owner, 1));
		let identity_commitment = sp_io::hashing::blake2_256(owner.as_ref());

		let code = decode_hex(include_str!(
			"../fixtures/build/IdentityAssetAudit.bin"
		));
		let instantiated = BareInstantiateBuilder::<Runtime>::bare_instantiate(
			RuntimeOrigin::signed(owner.clone()),
			Code::Upload(code),
		)
		.transaction_limits(limits())
		.constructor_data(identity_commitment.to_vec())
		.salt(Some([8u8; 32]))
		.build_and_unwrap_result();
		assert!(!instantiated.result.did_revert());
		let contract_addr = instantiated.addr;
		let contract_account =
			<pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<Runtime>>::
				to_fallback_account_id(&contract_addr);

		let asset_id = 7u32;
		assert_ok!(Assets::create(
			RuntimeOrigin::signed(owner.clone()),
			asset_id.into(),
			owner.clone().into(),
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner.clone()),
			asset_id.into(),
			contract_account.clone().into(),
			100,
		));

		let audit_record = b"alice:identity-asset-transfer:40".to_vec();
		let audit = sp_io::hashing::blake2_256(&audit_record);
		let mut asset_addr = [0u8; 20];
		asset_addr[..4].copy_from_slice(&asset_id.to_be_bytes());
		asset_addr[16..18].copy_from_slice(&0x0120u16.to_be_bytes());
		let recipient_addr =
			<pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<Runtime>>::
				to_address(&recipient);
		let mut transfer =
			sp_io::hashing::keccak_256(b"transferAndAudit(address,address,uint256,bytes32)")[..4]
				.to_vec();
		for address in [asset_addr, recipient_addr.0] {
			transfer.extend_from_slice(&[0u8; 12]);
			transfer.extend_from_slice(&address);
		}
		transfer.extend_from_slice(&[0u8; 31]);
		transfer.push(40);
		transfer.extend_from_slice(&audit);
		let transferred = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(owner.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(transfer)
		.build_and_unwrap_result();
		assert!(!transferred.did_revert(), "contract call reverted: {transferred:?}");
		assert_eq!(Assets::balance(asset_id, &contract_account), 60);
		assert_eq!(Assets::balance(asset_id, &recipient), 40);

		let last_audit = BareCallBuilder::<Runtime>::bare_call(
			RuntimeOrigin::signed(owner.clone()),
			contract_addr,
		)
		.transaction_limits(limits())
		.data(sp_io::hashing::keccak_256(b"lastAudit()")[..4].to_vec())
		.build_and_unwrap_result();
		assert_eq!(last_audit.data, audit);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner.clone(),
			1,
			1024,
		));
		let storage_call = pallet_bulletin_transaction_storage::Call::<Runtime>::store {
			data: audit_record.clone(),
		};
		let (_, scope) = TransactionStorage::validate_signed(&owner, &storage_call).unwrap();
		let scope = scope.expect("store calls carry their validated authorization scope");
		assert_ok!(TransactionStorage::pre_dispatch_signed(&owner, &storage_call));
		let authorized = pallet_bulletin_transaction_storage::Origin::<Runtime>::Authorized {
			who: owner,
			scope,
		};
		assert_ok!(TransactionStorage::store(
			RuntimeOrigin::from(authorized),
			audit_record,
		));
		assert!(TransactionStorage::contains_transaction(audit));
		<TransactionStorage as Hooks<u32>>::on_finalize(1);
		assert_eq!(TransactionStorage::transactions_at(1).unwrap()[0].content_hash, audit);
	});
}

#[test]
fn people_identity_is_self_claimed_and_sudo_attested() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::from(ALICE);
		let registrar = AccountId::from([3u8; 32]);
		let mut info =
			pallet_cord_identity::legacy::IdentityInfo::<crate::PeopleMaxAdditionalFields>::default(
			);
		info.display = pallet_cord_identity::Data::Raw(b"Alice".to_vec().try_into().unwrap());

		assert_ok!(People::set_identity(RuntimeOrigin::signed(account.clone()), Box::new(info),));
		assert!(People::has_identity(&account, 1));

		assert_noop!(
			People::add_registrar(RuntimeOrigin::signed(account), registrar.clone().into()),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(People::add_registrar(RuntimeOrigin::root(), registrar.into()));
	});
}

#[test]
fn bulletin_storage_is_authorized_indexed_and_content_addressed() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		System::set_extrinsic_index(0);
		let account = AccountId::from(ALICE);
		let data = b"identity-bound audit record".to_vec();

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			account.clone(),
			2,
			1024,
		));
		let authorization = TransactionStorage::account_authorization(account.clone()).unwrap();
		assert_eq!(authorization.transactions_allowance, 2);
		assert_eq!(authorization.bytes_allowance, 1024);
		assert!(TransactionStorage::can_store(&account, data.len() as u32));

		assert_ok!(TransactionStorage::store(RuntimeOrigin::root(), data.clone()));
		let content_hash = sp_io::hashing::blake2_256(&data);
		assert!(TransactionStorage::contains_transaction(content_hash));
		<TransactionStorage as Hooks<u32>>::on_finalize(1);
		let indexed = TransactionStorage::transactions_at(1).unwrap();
		assert_eq!(indexed.len(), 1);
		assert_eq!(indexed[0].content_hash, content_hash);
	});
}

#[test]
fn bulletin_storage_mutations_are_rejected_when_wrapped_or_sent_by_xcm() {
	type XcmSafeCalls =
		<crate::xcm_config::XcmConfig as xcm_executor::Config>::SafeCallFilter;
	let store = RuntimeCall::TransactionStorage(pallet_bulletin_transaction_storage::Call::store {
		data: b"audit".to_vec(),
	});
	assert!(crate::BulletinCallInspector::contains(&store));
	assert!(!XcmSafeCalls::contains(&store));

	let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![store] });
	assert!(crate::BulletinCallInspector::contains(&wrapped));
	assert!(!XcmSafeCalls::contains(&wrapped));

	let ordinary = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
	assert!(!crate::BulletinCallInspector::contains(&ordinary));
	assert!(XcmSafeCalls::contains(&ordinary));
}

fn full_core_task(task: u32) -> Schedule {
	Schedule::truncate_from(vec![ScheduleItem {
		mask: CoreMask::complete(),
		assignment: CoreAssignment::Task(task),
	}])
}

#[test]
fn orbis_sudo_can_reserve_multiple_full_cores_for_one_parachain() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let orbis = full_core_task(1006);
		let another_para = full_core_task(2000);

		for _ in 0..3 {
			assert_ok!(Broker::reserve(RuntimeOrigin::root(), orbis.clone()));
		}
		assert_ok!(Broker::reserve(RuntimeOrigin::root(), another_para.clone()));

		let reservations = Reservations::<Runtime>::get();
		assert_eq!(reservations.len(), 4);
		assert_eq!(reservations.iter().filter(|schedule| **schedule == orbis).count(), 3);
		assert_eq!(reservations[3], another_para);
	});
}

#[test]
fn broker_allocation_lifecycle_is_sudo_only() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let signed = RuntimeOrigin::signed(AccountId::from(ALICE));
		assert_noop!(
			Broker::reserve(signed.clone(), full_core_task(1006)),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(
			Broker::request_core_count(signed.clone(), 3),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_noop!(Broker::unreserve(signed, 0), sp_runtime::DispatchError::BadOrigin);

		assert_ok!(Broker::reserve(RuntimeOrigin::root(), full_core_task(1006)));
		assert_ok!(Broker::unreserve(RuntimeOrigin::root(), 0));
		assert!(Reservations::<Runtime>::get().is_empty());
	});
}

#[test]
fn fee_free_policy_is_call_scoped_quota_bounded_and_not_batchable() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		System::set_block_number(1);
		let account = AccountId::from(ALICE);
		let origin = RuntimeOrigin::signed(account.clone());
		assert_ok!(Feeless::add_feeless_account(RuntimeOrigin::root(), account.clone()));

		let allowed = RuntimeCall::Entity(pallet_entity::Call::rotate_attributes { ops: vec![] });
		assert!(allowed.is_feeless(&origin));

		let wrapped =
			RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![allowed.clone()] });
		assert!(!wrapped.is_feeless(&origin));

		let ordinary = RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: AccountId::from([9u8; 32]).into(),
			value: 1,
		});
		assert!(!ordinary.is_feeless(&origin));

		for _ in 0..16 {
			assert_ok!(Feeless::consume_feeless_quota(&account));
		}
		assert!(!allowed.is_feeless(&origin));
		assert_noop!(
			Feeless::consume_feeless_quota(&account),
			pallet_feeless::Error::<Runtime>::QuotaExhausted
		);
	});
}

#[test]
fn location_conversion_works() {
	let alice_32 = AccountId32 { network: None, id: AccountId::from(ALICE).into() };
	let bob_20 = AccountKey20 { network: None, key: [123u8; 20] };

	// the purpose of hardcoded values is to catch an unintended location conversion logic change.
	struct TestCase {
		description: &'static str,
		location: Location,
		expected_account_id_str: &'static str,
	}

	let test_cases = vec![
		// DescribeTerminus
		TestCase {
			description: "DescribeTerminus Parent",
			location: Location::new(1, Here),
			expected_account_id_str: "5Dt6dpkWPwLaH4BBCKJwjiWrFVAGyYk3tLUabvyn4v7KtESG",
		},
		TestCase {
			description: "DescribeTerminus Sibling",
			location: Location::new(1, [Parachain(1111)]),
			expected_account_id_str: "5Eg2fnssmmJnF3z1iZ1NouAuzciDaaDQH7qURAy3w15jULDk",
		},
		// DescribePalletTerminal
		TestCase {
			description: "DescribePalletTerminal Parent",
			location: Location::new(1, [PalletInstance(50)]),
			expected_account_id_str: "5CnwemvaAXkWFVwibiCvf2EjqwiqBi29S5cLLydZLEaEw6jZ",
		},
		TestCase {
			description: "DescribePalletTerminal Sibling",
			location: Location::new(1, [Parachain(1111), PalletInstance(50)]),
			expected_account_id_str: "5GFBgPjpEQPdaxEnFirUoa51u5erVx84twYxJVuBRAT2UP2g",
		},
		// DescribeAccountId32Terminal
		TestCase {
			description: "DescribeAccountId32Terminal Parent",
			location: Location::new(1, [alice_32]),
			expected_account_id_str: "5DN5SGsuUG7PAqFL47J9meViwdnk9AdeSWKFkcHC45hEzVz4",
		},
		TestCase {
			description: "DescribeAccountId32Terminal Sibling",
			location: Location::new(1, [Parachain(1111), alice_32]),
			expected_account_id_str: "5DGRXLYwWGce7wvm14vX1Ms4Vf118FSWQbJkyQigY2pfm6bg",
		},
		// DescribeAccountKey20Terminal
		TestCase {
			description: "DescribeAccountKey20Terminal Parent",
			location: Location::new(1, [bob_20]),
			expected_account_id_str: "5CJeW9bdeos6EmaEofTUiNrvyVobMBfWbdQvhTe6UciGjH2n",
		},
		TestCase {
			description: "DescribeAccountKey20Terminal Sibling",
			location: Location::new(1, [Parachain(1111), bob_20]),
			expected_account_id_str: "5CE6V5AKH8H4rg2aq5KMbvaVUDMumHKVPPQEEDMHPy3GmJQp",
		},
		// DescribeTreasuryVoiceTerminal
		TestCase {
			description: "DescribeTreasuryVoiceTerminal Parent",
			location: Location::new(1, [Plurality { id: BodyId::Treasury, part: BodyPart::Voice }]),
			expected_account_id_str: "5CUjnE2vgcUCuhxPwFoQ5r7p1DkhujgvMNDHaF2bLqRp4D5F",
		},
		TestCase {
			description: "DescribeTreasuryVoiceTerminal Sibling",
			location: Location::new(
				1,
				[Parachain(1111), Plurality { id: BodyId::Treasury, part: BodyPart::Voice }],
			),
			expected_account_id_str: "5G6TDwaVgbWmhqRUKjBhRRnH4ry9L9cjRymUEmiRsLbSE4gB",
		},
		// DescribeBodyTerminal
		TestCase {
			description: "DescribeBodyTerminal Parent",
			location: Location::new(1, [Plurality { id: BodyId::Unit, part: BodyPart::Voice }]),
			expected_account_id_str: "5EBRMTBkDisEXsaN283SRbzx9Xf2PXwUxxFCJohSGo4jYe6B",
		},
		TestCase {
			description: "DescribeBodyTerminal Sibling",
			location: Location::new(
				1,
				[Parachain(1111), Plurality { id: BodyId::Unit, part: BodyPart::Voice }],
			),
			expected_account_id_str: "5DBoExvojy8tYnHgLL97phNH975CyT45PWTZEeGoBZfAyRMH",
		},
	];

	for tc in test_cases {
		let expected =
			AccountId::from_string(tc.expected_account_id_str).expect("Invalid AccountId string");

		let got = LocationToAccountHelper::<AccountId, LocationToAccountId>::convert_location(
			tc.location.into(),
		)
		.unwrap();

		assert_eq!(got, expected, "{}", tc.description);
	}
}

#[test]
fn xcm_payment_api_works() {
	use crate::{Block, Runtime, RuntimeCall, RuntimeOrigin, WeightToFee};
	parachains_runtimes_test_utils::test_cases::xcm_payment_api_with_native_token_works::<
		Runtime,
		RuntimeCall,
		RuntimeOrigin,
		Block,
		WeightToFee,
	>();
}
