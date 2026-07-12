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
	xcm_config::LocationToAccountId, AssetConversion, AssetRate, AssetTxPayment, Assets,
	AssetsFreezer, AssetsHolder, Balances, Broker, ChunksManager, Entity, Feeless, ForeignAssets,
	ForeignAssetsFreezer, HopPromotion, Members, MembersNotifier, Nfts, People, PoolAssets,
	PoolAssetsFreezer, Revive, Runtime, RuntimeCall, RuntimeOrigin, System, TransactionStorage,
	Uniques,
};
use codec::{Decode, Encode};
use cumulus_primitives_core::ParaId;
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

#[test]
fn elastic_scaling_runtime_parameters_target_three_blocks_per_relay_slot() {
	assert_eq!(crate::RELAY_PARENT_OFFSET, 1);
	assert_eq!(crate::BLOCK_PROCESSING_VELOCITY, 3);
	assert_eq!(crate::SLOT_DURATION, 6_000);
	assert_eq!(crate::UNINCLUDED_SEGMENT_CAPACITY, 12);
	assert_eq!(
		<<Runtime as cumulus_pallet_parachain_system::Config>::RelayParentOffset as Get<u32>>::get(
		),
		1
	);
	assert!(<<Runtime as pallet_aura::Config>::AllowMultipleBlocksPerSlot as Get<bool>>::get());
}

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
fn enterprise_assets_support_native_holds_and_freezes() {
	use frame_support::traits::tokens::fungibles::{
		freeze::{Inspect, Mutate},
		UnbalancedHold,
	};
	use pallet_assets::BalanceOnHold;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let asset_id = 8u32;
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

		let hold_reason = crate::RuntimeHoldReason::TransactionStorage(
			pallet_bulletin_transaction_storage::HoldReason::StorageFeeHold,
		);
		assert_ok!(AssetsHolder::set_balance_on_hold(asset_id, &hold_reason, &beneficiary, 400,));
		assert_eq!(AssetsHolder::balance_on_hold(asset_id, &beneficiary), Some(400));

		let freeze_reason =
			crate::RuntimeFreezeReason::Revive(pallet_revive::FreezeReason::PGasMinBalance);
		assert_ok!(AssetsFreezer::set_freeze(asset_id, &freeze_reason, &beneficiary, 700,));
		assert_eq!(AssetsFreezer::balance_frozen(asset_id, &freeze_reason, &beneficiary), 700);
	});
}

#[test]
fn foreign_and_pool_assets_are_native_and_sudo_administered() {
	use frame_support::traits::tokens::fungibles::freeze::Mutate;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		let foreign_id = Location::parent();

		assert_noop!(
			ForeignAssets::create(
				RuntimeOrigin::signed(owner.clone()),
				foreign_id.clone(),
				owner.clone().into(),
				1,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(ForeignAssets::force_create(
			RuntimeOrigin::root(),
			foreign_id.clone(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(ForeignAssets::mint(
			RuntimeOrigin::signed(owner.clone()),
			foreign_id.clone(),
			beneficiary.clone().into(),
			500,
		));
		let freeze_reason =
			crate::RuntimeFreezeReason::Revive(pallet_revive::FreezeReason::PGasMinBalance);
		assert_ok!(
			ForeignAssetsFreezer::set_freeze(foreign_id, &freeze_reason, &beneficiary, 300,)
		);

		assert_ok!(PoolAssets::force_create(
			RuntimeOrigin::root(),
			7,
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(PoolAssets::mint(
			RuntimeOrigin::signed(owner),
			7,
			beneficiary.clone().into(),
			1_000,
		));
		assert_ok!(PoolAssetsFreezer::set_freeze(7, &freeze_reason, &beneficiary, 600,));
		assert_eq!(PoolAssets::balance(7, beneficiary), 1_000);
	});
}

#[test]
fn native_unique_and_nft_collections_mint_items() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let beneficiary = AccountId::from([2u8; 32]);
		<Balances as Mutate<AccountId>>::set_balance(&owner, 1_000_000_000_000_000);

		assert_ok!(
			Uniques::create(RuntimeOrigin::signed(owner.clone()), 10, owner.clone().into(),)
		);
		assert_ok!(Uniques::mint(
			RuntimeOrigin::signed(owner.clone()),
			10,
			1,
			beneficiary.clone().into(),
		));
		assert_eq!(Uniques::owner(10, 1), Some(beneficiary.clone()));

		let config = pallet_nfts::CollectionConfig {
			settings: pallet_nfts::CollectionSettings::all_enabled(),
			max_supply: None,
			mint_settings: Default::default(),
		};
		assert_ok!(Nfts::create(
			RuntimeOrigin::signed(owner.clone()),
			owner.clone().into(),
			config,
		));
		assert_ok!(Nfts::mint(
			RuntimeOrigin::signed(owner),
			0,
			1,
			beneficiary.clone().into(),
			None,
		));
		assert_eq!(Nfts::owner(0, 1), Some(beneficiary));
	});
}

#[test]
fn asset_rates_are_location_based_and_sudo_administered() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let asset = Location::new(0, [PalletInstance(80), GeneralIndex(7)]);
		let initial = sp_runtime::FixedU128::from_rational(3, 2);
		let updated = sp_runtime::FixedU128::from_u32(2);

		assert_noop!(
			AssetRate::create(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				Box::new(asset.clone()),
				initial,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(AssetRate::create(RuntimeOrigin::root(), Box::new(asset.clone()), initial));
		assert_eq!(
			pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset),
			Some(initial)
		);
		assert_ok!(AssetRate::update(RuntimeOrigin::root(), Box::new(asset.clone()), updated));
		assert_eq!(
			pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset),
			Some(updated)
		);
		assert_ok!(AssetRate::remove(RuntimeOrigin::root(), Box::new(asset.clone())));
		assert!(pallet_asset_rate::ConversionRateToNative::<Runtime>::get(&asset).is_none());
	});
}

#[test]
fn native_asset_conversion_pool_supports_liquidity_and_swaps() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let owner = AccountId::from(ALICE);
		let receiver = AccountId::from([2u8; 32]);
		let native = Location::parent();
		let local = Location::new(0, [PalletInstance(80), GeneralIndex(21)]);

		<Balances as Mutate<AccountId>>::set_balance(&owner, 10_000 * crate::UNITS);
		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			21u32.into(),
			owner.clone().into(),
			true,
			1,
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(owner.clone()),
			21u32.into(),
			owner.clone().into(),
			10_000,
		));
		assert_ok!(AssetConversion::create_pool(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(native.clone()),
			Box::new(local.clone()),
		));
		assert_ok!(AssetConversion::add_liquidity(
			RuntimeOrigin::signed(owner.clone()),
			Box::new(native.clone()),
			Box::new(local.clone()),
			1_000 * crate::UNITS,
			1_000,
			1,
			1,
			owner.clone(),
		));
		assert_ok!(AssetConversion::swap_exact_tokens_for_tokens(
			RuntimeOrigin::signed(owner),
			vec![Box::new(native), Box::new(local)],
			10 * crate::UNITS,
			1,
			receiver.clone(),
			true,
		));
		assert!(Assets::balance(21, receiver) > 0);
	});
}

#[test]
fn asset_fee_selector_is_preserved_inside_the_feeless_envelope() {
	type AssetCharge = pallet_asset_conversion_tx_payment::ChargeAssetTxPayment<Runtime>;
	type WrappedCharge = pallet_feeless::ChargeOrSkipFeeless<Runtime, AssetCharge>;

	let asset = Location::new(0, [PalletInstance(80), GeneralIndex(21)]);
	let wrapped = WrappedCharge::from(AssetCharge::from(0, Some(asset)));
	let encoded = wrapped.encode();
	let decoded =
		WrappedCharge::decode(&mut encoded.as_slice()).expect("asset fee extension decodes");
	assert_eq!(decoded, wrapped);
}

#[test]
fn people_chunk_hashes_are_initialized_by_sudo_only() {
	use indiv_pallet_chunks_manager::{ChunkPageHashes, RingExponent};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let hashes: frame_support::BoundedVec<
			[u8; 32],
			frame_support::traits::ConstU32<{ indiv_pallet_chunks_manager::MAX_PAGE_COUNT }>,
		> = vec![[1u8; 32], [2u8; 32]].try_into().expect("two hashes are bounded");
		assert_noop!(
			ChunksManager::set_chunk_page_hashes(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				RingExponent::R2e9,
				hashes.clone(),
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(ChunksManager::set_chunk_page_hashes(
			RuntimeOrigin::root(),
			RingExponent::R2e9,
			hashes,
		));
		assert_eq!(ChunkPageHashes::<Runtime>::get(RingExponent::R2e9, 0), Some([1u8; 32]));
		assert_eq!(ChunkPageHashes::<Runtime>::get(RingExponent::R2e9, 1), Some([2u8; 32]));
	});
}

#[test]
fn people_membership_collections_are_native_and_sudo_managed() {
	use indiv_pallet_members::{Collections, OnboardingSize};
	use indiv_support::traits::{AppendOnlyMembers, RingExponent, RingMode};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		let identifier = [7u8; 32];
		assert_ok!(<Members as AppendOnlyMembers>::create_collection(
			Location::here(),
			&identifier,
			5,
			RingMode::AppendOnly,
			RingExponent::R2e9,
			None,
		));
		assert!(Collections::<Runtime>::contains_key(identifier));
		assert_noop!(
			Members::set_onboarding_size(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				identifier,
				10,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(Members::set_onboarding_size(RuntimeOrigin::root(), identifier, 10));
		assert_eq!(OnboardingSize::<Runtime>::get(identifier), 10);
	});
}

#[test]
fn ring_root_changes_are_queued_and_subscriptions_are_sudo_managed() {
	use indiv_pallet_members_notifier::{PageState, PendingUpdates, Subscribers};
	use indiv_support::traits::{OnRingRootChange, RingExponent, RingRootOp};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		pallet_timestamp::Now::<Runtime>::put(1_000);
		let identifier = [8u8; 32];
		<MembersNotifier as OnRingRootChange<
			indiv_pallet_members_notifier::MembersOf<Runtime>,
		>>::on_ring_root_change(identifier, 3, RingRootOp::Deleted);
		assert!(PendingUpdates::<Runtime>::contains_key((
			PageState::<Runtime>::get().write_page,
			identifier,
			3,
		)));

		let collections: frame_support::BoundedVec<
			([u8; 32], RingExponent),
			frame_support::traits::ConstU32<3>,
		> = vec![(identifier, RingExponent::R2e9)].try_into().unwrap();
		assert_noop!(
			MembersNotifier::subscribe(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				ParaId::from(2_000u32),
				collections.clone(),
				60,
			),
			sp_runtime::DispatchError::BadOrigin
		);
		assert_ok!(MembersNotifier::subscribe(
			RuntimeOrigin::root(),
			ParaId::from(2_000u32),
			collections,
			60,
		));
		assert!(Subscribers::<Runtime>::contains_key(ParaId::from(2_000u32)));
	});
}

#[test]
fn revive_uses_reserved_orbis_evm_chain_id() {
	assert_eq!(<<Runtime as pallet_revive::Config>::ChainId as Get<u64>>::get(), 420_001_006);
	assert!(<<Runtime as pallet_revive::Config>::AllowEVMBytecode as Get<bool>>::get());
	assert_eq!(<Assets as PalletInfoAccess>::index(), 80);
	assert_eq!(<AssetsFreezer as PalletInfoAccess>::index(), 81);
	assert_eq!(<AssetsHolder as PalletInfoAccess>::index(), 82);
	assert_eq!(<ForeignAssets as PalletInfoAccess>::index(), 83);
	assert_eq!(<PoolAssets as PalletInfoAccess>::index(), 84);
	assert_eq!(<ForeignAssetsFreezer as PalletInfoAccess>::index(), 85);
	assert_eq!(<PoolAssetsFreezer as PalletInfoAccess>::index(), 86);
	assert_eq!(<Uniques as PalletInfoAccess>::index(), 87);
	assert_eq!(<Nfts as PalletInfoAccess>::index(), 88);
	assert_eq!(<AssetRate as PalletInfoAccess>::index(), 89);
	assert_eq!(<AssetConversion as PalletInfoAccess>::index(), 200);
	assert_eq!(<AssetTxPayment as PalletInfoAccess>::index(), 201);
	assert_eq!(<Revive as PalletInfoAccess>::index(), 100);
	// The SDK relay Coretime pallet encodes callbacks to Broker at index 50.
	assert_eq!(<Broker as PalletInfoAccess>::index(), 50);
	assert_eq!(<Entity as PalletInfoAccess>::index(), 53);
	assert_eq!(<People as PalletInfoAccess>::index(), 90);
	assert_eq!(<TransactionStorage as PalletInfoAccess>::index(), 110);
	assert_eq!(<crate::HopPromotion as PalletInfoAccess>::index(), 111);
	assert_eq!(<crate::WeightReclaim as PalletInfoAccess>::index(), 4);
	assert_eq!(<ChunksManager as PalletInfoAccess>::index(), 91);
	assert_eq!(<Members as PalletInfoAccess>::index(), 92);
	assert_eq!(<MembersNotifier as PalletInfoAccess>::index(), 93);
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

		let mut identity = pallet_orbis_people::legacy::IdentityInfo::<
			crate::PeopleMaxAdditionalFields,
		>::default();
		identity.display =
			pallet_orbis_people::Data::Raw(b"Alice Orbis".to_vec().try_into().unwrap());
		assert_ok!(People::set_identity(RuntimeOrigin::signed(owner.clone()), Box::new(identity),));
		assert!(People::has_identity(&owner, 1));
		let identity_commitment = sp_io::hashing::blake2_256(owner.as_ref());

		let code = decode_hex(include_str!("../fixtures/build/IdentityAssetAudit.bin"));
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
		let contract_account = <pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<
			Runtime,
		>>::to_fallback_account_id(&contract_addr);

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
		let recipient_addr = <pallet_revive::AccountId32Mapper<Runtime> as AddressMapper<
			Runtime,
		>>::to_address(&recipient);
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
		assert_ok!(TransactionStorage::store(RuntimeOrigin::from(authorized), audit_record,));
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
			pallet_orbis_people::legacy::IdentityInfo::<crate::PeopleMaxAdditionalFields>::default(
			);
		info.display = pallet_orbis_people::Data::Raw(b"Alice".to_vec().try_into().unwrap());

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
fn hop_promotion_accepts_authorized_signed_submit_intent() {
	use frame_support::traits::BuildGenesisConfig;
	use sp_core::{sr25519, Pair};
	use sp_runtime::{traits::IdentifyAccount, MultiSignature, MultiSigner};

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let now = 1_750_000_000_000u64;
		pallet_timestamp::Now::<Runtime>::put(now);

		let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let signer = MultiSigner::from(pair.public());
		let account = signer.clone().into_account();
		<Balances as Mutate<AccountId>>::set_balance(&account, 1_000_000_000_000);
		assert_ok!(
			TransactionStorage::authorize_account(RuntimeOrigin::root(), account, 1, 1_024,)
		);

		let data = b"orbis hop promotion".to_vec();
		let hash = sp_io::hashing::blake2_256(&data);
		let payload = pallet_bulletin_hop_promotion::signing_payload(&hash, now);
		let signature = MultiSignature::Sr25519(pair.sign(&payload));
		assert!(HopPromotion::authorize_promote(
			sp_runtime::transaction_validity::TransactionSource::Local,
			&signer,
			&signature,
			&now,
			&data,
		)
		.is_ok());
		assert!(!HopPromotion::is_promoted_on_chain(hash));
	});
}

#[test]
fn bulletin_storage_mutations_are_rejected_when_wrapped_or_sent_by_xcm() {
	type XcmSafeCalls = <crate::xcm_config::XcmConfig as xcm_executor::Config>::SafeCallFilter;
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
fn sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery() {
	use codec::Encode;
	use frame_support::traits::BuildGenesisConfig;
	use sp_core::{sr25519, Pair};
	use sp_runtime::{
		generic::Era,
		traits::{Hash, IdentifyAccount, TransactionExtension},
		MultiSignature, MultiSigner,
	};
	const META_EXTENSION_VERSION: u8 = 0;

	type MetaBareExtension = (
		pallet_meta_tx::MetaTxMarker<Runtime>,
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckMortality<Runtime>,
		frame_system::CheckNonce<Runtime>,
		pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
			Runtime,
			crate::BulletinCallInspector,
		>,
		frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	);

	fn account(pair: &sr25519::Pair) -> AccountId {
		MultiSigner::from(pair.public()).into_account()
	}

	fn signed_meta_tx(
		call: RuntimeCall,
		claimed: AccountId,
		signing_pair: &sr25519::Pair,
	) -> pallet_meta_tx::MetaTxFor<Runtime> {
		let bare: MetaBareExtension = (
			pallet_meta_tx::MetaTxMarker::new(),
			frame_system::CheckNonZeroSender::new(),
			frame_system::CheckSpecVersion::new(),
			frame_system::CheckTxVersion::new(),
			frame_system::CheckGenesis::new(),
			frame_system::CheckMortality::from(Era::Immortal),
			frame_system::CheckNonce::from(System::account(&claimed).nonce),
			Default::default(),
			frame_metadata_hash_extension::CheckMetadataHash::new(false),
		);
		let implicit = bare.implicit().expect("test externalities provide implicit data");
		let signature = (META_EXTENSION_VERSION, call.clone(), bare.clone(), implicit)
			.using_encoded(|payload| signing_pair.sign(&sp_io::hashing::blake2_256(payload)));
		let verify = pallet_verify_signature::VerifySignature::new_with_signature(
			MultiSignature::Sr25519(signature),
			claimed,
		);
		let (marker, nonzero, spec, tx, genesis, mortality, nonce, storage, metadata) = bare;
		let extension =
			(verify, marker, nonzero, spec, tx, genesis, mortality, nonce, storage, metadata);
		pallet_meta_tx::MetaTxFor::<Runtime>::new(call, META_EXTENSION_VERSION, extension)
	}

	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		System::set_block_number(1);
		let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
		let alice = account(&alice_pair);
		let bob = account(&bob_pair);
		let alice_balance =
			<Balances as Mutate<AccountId>>::set_balance(&alice, crate::ExistentialDeposit::get());
		let inner = RuntimeCall::System(frame_system::Call::remark_with_event {
			remark: b"identity intent".to_vec(),
		});

		let meta = signed_meta_tx(inner.clone(), alice.clone(), &alice_pair);
		let encoded_len = meta.encoded_size() as u32;
		let outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(meta.clone()),
			meta_tx_encoded_len: encoded_len,
		});
		assert!(
			!outer.is_feeless(&RuntimeOrigin::signed(bob.clone())),
			"the sponsor's outer meta transaction must follow ordinary fee accounting"
		);
		assert_ok!(crate::MetaTx::dispatch(
			RuntimeOrigin::signed(bob.clone()),
			Box::new(meta.clone()),
			encoded_len,
		));
		System::assert_has_event(crate::RuntimeEvent::System(frame_system::Event::Remarked {
			sender: alice.clone(),
			hash: <Runtime as frame_system::Config>::Hashing::hash(b"identity intent"),
		}));
		assert_eq!(System::account_nonce(&alice), 1);
		assert_eq!(Balances::free_balance(&alice), alice_balance);

		assert_noop!(
			crate::MetaTx::dispatch(
				RuntimeOrigin::signed(bob.clone()),
				Box::new(meta),
				encoded_len,
			),
			pallet_meta_tx::Error::<Runtime>::Stale,
		);

		let forged = signed_meta_tx(inner, alice, &bob_pair);
		let forged_len = forged.encoded_size() as u32;
		assert_noop!(
			crate::MetaTx::dispatch(RuntimeOrigin::signed(bob), Box::new(forged), forged_len),
			pallet_meta_tx::Error::<Runtime>::BadProof,
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
