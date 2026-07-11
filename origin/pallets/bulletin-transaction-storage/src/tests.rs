// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tests for transaction-storage pallet.

// Tests still call the deprecated `ValidateUnsigned::{validate_unsigned, pre_dispatch}` directly.
// Migration to `#[pallet::authorize]` is tracked separately; silence here so `-D warnings` in CI
// does not block the SDK bump.
#![allow(deprecated)]

use super::{
	extension::ValidateStorageCalls,
	mock::{
		new_test_ext, run_to_block, MaxPermanentStorageSize, RuntimeCall, RuntimeEvent,
		RuntimeOrigin, StoreRenewPriority, System, Test, TransactionStorage,
	},
	pallet::Origin,
	AllowedAuthorizers, AuthorizationExtent, AuthorizationOrigin, AuthorizationScope,
	AuthorizedCaller, AuthorizerBudget, EnsureAllowedAuthorizers, Event, Quota, TransactionInfo,
	TransactionKind, TransactionRef, AUTHORIZATION_NOT_EXHAUSTED, AUTHORIZATION_NOT_EXPIRED,
	AUTHORIZER_NOT_FOUND, BAD_DATA_SIZE, CHAIN_PERMANENT_CAP_REACHED,
	DEFAULT_MAX_BLOCK_TRANSACTIONS, DEFAULT_MAX_TRANSACTION_SIZE, PERMANENT_ALLOWANCE_EXCEEDED,
	PERMANENT_STORAGE_NEAR_CAP_PERCENT,
};

use crate::{migrations::v1::OldTransactionInfo, mock::RuntimeGenesisConfig};
use bulletin_transaction_storage_primitives::cids::{CidConfig, HashingAlgorithm};
use codec::Encode;
use polkadot_sdk_frame::{
	deps::frame_support::{
		storage::unhashed,
		traits::{GetStorageVersion, Hooks, OnRuntimeUpgrade},
		BoundedVec,
	},
	hashing::blake2_256,
	prelude::*,
	testing_prelude::*,
	traits::StorageVersion,
};
use sp_transaction_storage_proof::{
	num_chunks, random_chunk, registration::build_proof, ChunkIndex, CHUNK_SIZE,
};

type Call = super::Call<Test>;
type Error = super::Error<Test>;

type Authorizations = super::Authorizations<Test>;
type BlockTransactions = super::BlockTransactions<Test>;
type PermanentStorageUsed = super::PermanentStorageUsed<Test>;
type RetentionPeriod = super::RetentionPeriod<Test>;
type Transactions = super::Transactions<Test>;
type TransactionByContentHash = super::TransactionByContentHash<Test>;

fn test_budget(transactions: u32, bytes: u64) -> AuthorizerBudget<u64> {
	AuthorizerBudget {
		quota: Some(Quota { transactions, bytes }),
		valid_until: None,
		feeless: false,
	}
}

const MAX_DATA_SIZE: u32 = DEFAULT_MAX_TRANSACTION_SIZE;

mod runtime_api;

/// Run `enable_auto_renew` through the same pipeline the runtime uses:
/// `pre_dispatch_signed` (charges bytes + tx slot, matches what the extension's
/// `prepare` step does at runtime) followed by `dispatch` with an
/// `Origin::Authorized` (mirrors the rewrite done by the extension's `validate`).
/// Tests that exercise pre-dispatch failures should call `validate_signed`
/// directly instead.
fn enable_auto_renew_via_extension(who: u64, content_hash: super::ContentHash) -> DispatchResult {
	let call = Call::enable_auto_renew { content_hash };
	TransactionStorage::pre_dispatch_signed(&who, &call)
		.expect("pre_dispatch_signed must succeed for the via-extension test helper");
	let origin: RuntimeOrigin =
		Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
	TransactionStorage::enable_auto_renew(origin, content_hash)
}

/// Sibling of `enable_auto_renew_via_extension` for `disable_auto_renew`. Builds the
/// rewritten `Origin::Authorized` directly (skips `pre_dispatch_signed`), since most
/// disable tests want to exercise dispatch-level errors after admission.
fn disable_auto_renew_via_extension(who: u64, content_hash: super::ContentHash) -> DispatchResult {
	let origin: RuntimeOrigin =
		Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
	TransactionStorage::disable_auto_renew(origin, content_hash)
}

/// Sibling of `enable_auto_renew_via_extension` for `renew` (one-shot scheduler).
/// Runs `pre_dispatch_signed` (charges the tx slot) and then dispatches with the
/// rewritten `Origin::Authorized`.
fn renew_via_extension(who: u64, entry: super::TransactionRef<u64>) -> DispatchResult {
	let call = Call::renew { entry: entry.clone() };
	TransactionStorage::pre_dispatch_signed(&who, &call)
		.expect("pre_dispatch_signed must succeed for the via-extension test helper");
	let origin: RuntimeOrigin =
		Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
	TransactionStorage::renew(origin, entry)
}

#[test]
fn discards_data() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		let proof_provider = || {
			let block_num = System::block_number();
			if block_num == 11 {
				let parent_hash = System::parent_hash();
				build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000], vec![0u8; 2000]]).unwrap()
			} else {
				None
			}
		};
		run_to_block(11, proof_provider);
		assert!(Transactions::get(1).is_some());
		let transactions = Transactions::get(1).unwrap();
		assert_eq!(transactions.len(), 2);
		run_to_block(12, proof_provider);
		assert!(Transactions::get(1).is_none());
	});
}

#[test]
fn uses_account_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let caller = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), caller, 0, 2001));
		assert_eq!(
			TransactionStorage::account_authorization_extent(caller),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2001,
				transactions: 0,
				transactions_allowance: 0,
			}
		);
		let call = Call::store { data: vec![0u8; 2000] };
		// A caller without any Authorization entry is still rejected.
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&5, &call),
			InvalidTransaction::Payment,
		);
		assert_ok!(TransactionStorage::pre_dispatch_signed(&caller, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(caller),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2001,
				transactions: 1,
				transactions_allowance: 0,
			}
		);
		// A second store that overshoots the allowance no longer rejects; `bytes` saturates
		// upward and the entry stays put.
		let call = Call::store { data: vec![0u8; 2] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&caller, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(caller),
			AuthorizationExtent {
				bytes: 2002,
				bytes_permanent: 0,
				bytes_allowance: 2001,
				transactions: 2,
				transactions_allowance: 0,
			}
		);
	});
}

#[test]
fn uses_preimage_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![2; 2000];
		let hash = blake2_256(&data);
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), hash, 2002));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2002,
				transactions: 0,
				transactions_allowance: 1,
			}
		);
		// Data with a non-matching hash has no preimage auth → rejected.
		let call = Call::store { data: vec![1; 2000] };
		assert_noop!(TransactionStorage::pre_dispatch(&call), InvalidTransaction::Payment);
		// Matching data consumes allowance but the entry stays (new behaviour).
		let call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch(&call));
		// Entry persists with the remainder (2002 - 2000 = 2 bytes); the
		// transaction count is exhausted so further stores still fail.
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2002,
				transactions: 1,
				transactions_allowance: 1,
			}
		);
		assert_ok!(Into::<RuntimeCall>::into(call).dispatch(RuntimeOrigin::none()));
		run_to_block(3, || None);
		// Renew also uses the same preimage auth; it bumps `bytes_permanent` rather than `bytes`.
		let call = Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch(&call));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 2000,
				bytes_allowance: 2002,
				transactions: 2,
				transactions_allowance: 1,
			}
		);
	});
}

#[test]
fn checks_proof() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(
			RuntimeOrigin::none(),
			vec![0u8; MAX_DATA_SIZE as usize]
		));
		run_to_block(10, || None);
		let parent_hash = System::parent_hash();
		let proof = build_proof(parent_hash.as_ref(), vec![vec![0u8; MAX_DATA_SIZE as usize]])
			.unwrap()
			.unwrap();
		assert_noop!(
			TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), Some(proof)),
			Error::UnexpectedProof,
		);
		run_to_block(11, || None);
		let parent_hash = System::parent_hash();

		let invalid_proof =
			build_proof(parent_hash.as_ref(), vec![vec![0u8; 1000]]).unwrap().unwrap();
		assert_noop!(
			TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), Some(invalid_proof)),
			Error::InvalidProof,
		);

		let proof = build_proof(parent_hash.as_ref(), vec![vec![0u8; MAX_DATA_SIZE as usize]])
			.unwrap()
			.unwrap();
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), Some(proof)));
	});
}

#[test]
fn checks_proof_with_v2_shaped_transactions_entry() {
	use crate::migrations::v3::V2TransactionInfo;

	new_test_ext().execute_with(|| {
		let data = vec![0u8; 2000];

		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);

		// Rewrite the freshly-written v3 entry at block 1 into the old v2 shape to
		// simulate the MBM window where historical `Transactions` entries have not yet
		// been rewritten by `MigrateV2ToV3`, while `check_proof` still executes every block.
		let txs_v3 = Transactions::get(1).expect("block 1 entry stored in v3 shape");
		let txs_v2: Vec<V2TransactionInfo> = txs_v3
			.into_iter()
			.map(|tx| V2TransactionInfo {
				chunk_root: tx.chunk_root,
				content_hash: tx.content_hash,
				hashing: tx.hashing,
				cid_codec: tx.cid_codec,
				size: tx.size,
				block_chunks: tx.block_chunks,
			})
			.collect();
		let bounded: BoundedVec<V2TransactionInfo, ConstU32<DEFAULT_MAX_BLOCK_TRANSACTIONS>> =
			txs_v2.try_into().expect("within bounds");
		unhashed::put_raw(&Transactions::hashed_key_for(1u64), &bounded.encode());

		// Direct decode as the live v3 type now fails.
		assert!(Transactions::get(1).is_none());

		run_to_block(11, || None);
		let parent_hash = System::parent_hash();
		let proof = build_proof(parent_hash.as_ref(), vec![data]).unwrap().unwrap();

		assert_ok!(TransactionStorage::apply_block_inherents(
			RuntimeOrigin::none(),
			Some(proof),
		));
		assert!(
			<super::ProofChecked<Test>>::get(),
			"apply_block_inherents proof step should succeed by using transactions_at() on the v2-shaped entry",
		);
	});
}

#[test]
fn verify_chunk_proof_works() {
	new_test_ext().execute_with(|| {
		// Prepare a bunch of transactions with variable chunk sizes.
		let transactions = vec![
			vec![0u8; CHUNK_SIZE - 1],
			vec![1u8; CHUNK_SIZE],
			vec![2u8; CHUNK_SIZE + 1],
			vec![3u8; 2 * CHUNK_SIZE - 1],
			vec![3u8; 2 * CHUNK_SIZE],
			vec![3u8; 2 * CHUNK_SIZE + 1],
			vec![4u8; 7 * CHUNK_SIZE - 1],
			vec![4u8; 7 * CHUNK_SIZE],
			vec![4u8; 7 * CHUNK_SIZE + 1],
		];
		let expected_total_chunks =
			transactions.iter().map(|t| t.len().div_ceil(CHUNK_SIZE) as u32).sum::<u32>();

		// Store a couple of transactions in one block.
		run_to_block(1, || None);
		for transaction in transactions.clone() {
			assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), transaction));
		}
		run_to_block(2, || None);

		// Read all the block transactions metadata.
		let tx_infos = Transactions::get(1).unwrap();
		let total_chunks = TransactionInfo::total_chunks(&tx_infos);
		assert_eq!(expected_total_chunks, total_chunks);
		assert_eq!(9, tx_infos.len());

		// Verify proofs for all possible chunk indexes.
		for chunk_index in 0..total_chunks {
			// chunk index randomness
			let mut random_hash = [0u8; 32];
			random_hash[..8].copy_from_slice(&(chunk_index as u64).to_be_bytes());
			let selected_chunk_index = random_chunk(random_hash.as_ref(), total_chunks);
			assert_eq!(selected_chunk_index, chunk_index);

			// build/check chunk proof roundtrip
			let proof = build_proof(random_hash.as_ref(), transactions.clone())
				.expect("valid proof")
				.unwrap();
			assert_ok!(TransactionStorage::verify_chunk_proof(
				proof,
				random_hash.as_ref(),
				tx_infos.to_vec(),
			));
		}
	});
}

#[test]
fn renews_data() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		let info = BlockTransactions::get().last().unwrap().clone();
		run_to_block(6, || None);
		assert_ok!(TransactionStorage::force_renew(
			RuntimeOrigin::none(),
			TransactionRef::Position { block: 1, index: 0 },
		));
		let proof_provider = || {
			let block_num = System::block_number();
			if block_num == 11 || block_num == 16 {
				let parent_hash = System::parent_hash();
				build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000]]).unwrap()
			} else {
				None
			}
		};
		run_to_block(16, proof_provider);
		assert!(Transactions::get(1).is_none());
		// Renew preserves chunk_root / content_hash / size from the original entry but
		// stamps `kind = Renew` so on_initialize cleanup can decrement the chain counter.
		let renewed = Transactions::get(6).unwrap().first().unwrap().clone();
		assert_eq!(renewed.chunk_root, info.chunk_root);
		assert_eq!(renewed.content_hash, info.content_hash);
		assert_eq!(renewed.size, info.size);
		assert_eq!(renewed.kind, TransactionKind::Renew);
		run_to_block(17, proof_provider);
		assert!(Transactions::get(6).is_none());
	});
}

#[test]
fn authorization_expires() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		let call = Call::store { data: vec![0; 2000] };
		assert_ok!(TransactionStorage::validate_signed(&who, &call));
		run_to_block(10, || None);
		// validate_signed does not consume — extent unchanged.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		assert_ok!(TransactionStorage::validate_signed(&who, &call));
		run_to_block(11, || None);
		// Expired authorizations report as zero extent.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 0,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		assert_noop!(TransactionStorage::validate_signed(&who, &call), InvalidTransaction::Payment);
	});
}

#[test]
fn expired_authorization_clears() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert!(System::providers(&who).is_zero());
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		assert!(!System::providers(&who).is_zero());

		// User consumes 1000 bytes of the 2000-byte allowance.
		run_to_block(2, || None);
		let store_call = Call::store { data: vec![0; 1000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 1000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 0,
			},
		);

		// Can't remove too early
		run_to_block(10, || None);
		let remove_call = Call::remove_expired_account_authorization { who };
		assert_noop!(TransactionStorage::pre_dispatch(&remove_call), AUTHORIZATION_NOT_EXPIRED);
		assert_noop!(
			Into::<RuntimeCall>::into(remove_call.clone()).dispatch(RuntimeOrigin::none()),
			Error::AuthorizationNotExpired,
		);

		// User has sufficient storage authorization, but it has expired
		run_to_block(11, || None);
		assert!(Authorizations::contains_key(AuthorizationScope::Account(who)));
		assert!(!System::providers(&who).is_zero());
		// User cannot use authorization
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&who, &store_call),
			InvalidTransaction::Payment,
		);
		// Anyone can remove it
		assert_ok!(TransactionStorage::pre_dispatch(&remove_call));
		assert_ok!(Into::<RuntimeCall>::into(remove_call).dispatch(RuntimeOrigin::none()));
		System::assert_has_event(RuntimeEvent::TransactionStorage(
			Event::ExpiredAccountAuthorizationRemoved { who },
		));
		// No longer in storage
		assert!(!Authorizations::contains_key(AuthorizationScope::Account(who)));
		assert!(System::providers(&who).is_zero());
	});
}

#[test]
fn consumed_authorization_stays_over_cap() {
	// `check_authorization` always adds and never removes the entry on overshoot, so the
	// Authorization stays in storage (and the provider reference with it) even when `bytes`
	// exceeds `bytes_allowance`. Only expiration cleans it up.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert!(System::providers(&who).is_zero());
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		assert!(!System::providers(&who).is_zero());

		let call = Call::store { data: vec![0; 1000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 1000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 0,
			},
		);
		// Second consumption saturates at the cap.
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 2,
				transactions_allowance: 0,
			},
		);
		// Third consumption pushes `bytes` over the cap but still succeeds.
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 3000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 3,
				transactions_allowance: 0,
			},
		);
		// Entry is still in storage and the provider reference is still held.
		assert!(Authorizations::contains_key(AuthorizationScope::Account(who)));
		assert!(!System::providers(&who).is_zero());
	});
}

#[test]
fn stores_various_sizes_with_account_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let max = DEFAULT_MAX_TRANSACTION_SIZE as usize;
		let sizes: [usize; 6] = [
			1,           // minimum valid size
			2000,        // small
			max / 4,     // 25%
			max / 2,     // 50%
			max * 3 / 4, // 75%
			max,         // 100% (exactly at limit)
		];
		let total_bytes: u64 = sizes.iter().map(|s| *s as u64).sum();
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			0,
			total_bytes
		));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: total_bytes,
				transactions: 0,
				transactions_allowance: 0,
			},
		);

		for size in sizes {
			let call = Call::store { data: vec![0u8; size] };
			assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
			assert_ok!(Into::<RuntimeCall>::into(call).dispatch(RuntimeOrigin::none()));
		}

		// After using exactly the authorized allowance, bytes == bytes_allowance — entry stays.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: total_bytes,
				bytes_permanent: 0,
				bytes_allowance: total_bytes,
				transactions: 6,
				transactions_allowance: 0,
			},
		);
		assert!(Authorizations::contains_key(AuthorizationScope::Account(who)));
		assert!(!System::providers(&who).is_zero());

		// Zero-size data must be rejected
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1));
		let empty_call = Call::store { data: vec![] };
		assert_noop!(TransactionStorage::pre_dispatch_signed(&who, &empty_call), BAD_DATA_SIZE);
		assert_noop!(
			Into::<RuntimeCall>::into(empty_call).dispatch(RuntimeOrigin::none()),
			Error::BadDataSize,
		);

		// Assert that a payload exceeding the max size fails, even with fresh authorization
		let oversize: usize = max + 1;
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			0,
			oversize as u64
		));
		let too_big_call = Call::store { data: vec![0u8; oversize] };
		// pre_dispatch should reject due to BAD_DATA_SIZE
		assert_noop!(TransactionStorage::pre_dispatch_signed(&who, &too_big_call), BAD_DATA_SIZE);
		// dispatch should also reject with pallet Error::BadDataSize
		assert_noop!(
			Into::<RuntimeCall>::into(too_big_call).dispatch(RuntimeOrigin::none()),
			Error::BadDataSize,
		);
		run_to_block(2, || None);
	});
}

/// `renew` accepts a content-hash variant of [`TransactionRef`] equivalently to
/// the position variant.
#[test]
fn renew_by_content_hash_schedules_one_shot() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Unknown content hash is rejected at dispatch (after origin admission).
		let bogus_hash = [0u8; 32];
		let origin: RuntimeOrigin =
			Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
		assert_noop!(
			TransactionStorage::renew(origin, TransactionRef::ContentHash(bogus_hash)),
			Error::RenewedNotFound,
		);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::ContentHash(content_hash)));

		let entry = AutoRenewals::get(content_hash).unwrap();
		assert_eq!(entry.account, who);
		assert!(!entry.recurring, "renew should register a one-shot entry");
		assert!(entry.paid, "one-shot is prepaid at registration");

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::RenewalEnabled {
			content_hash,
			who,
			recurring: false,
		}));
	});
}

#[test]
fn storage_calls_reject_plain_signed_origin() {
	// Storage-mutating calls must gate on `ensure_authorized` (accepts `Authorized` /
	// `Root` / `None` only). A plain `Signed` origin bypasses the extension pipeline and
	// must be rejected. Catches the class of bug where the gate is dropped on a refactor.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let signed = RuntimeOrigin::signed(42);
		let data = vec![0u8; 2000];
		let cid_config = CidConfig { codec: 0x55, hashing: HashingAlgorithm::Blake2b256 };

		assert_noop!(
			TransactionStorage::store(signed.clone(), data.clone()),
			DispatchError::BadOrigin,
		);
		assert_noop!(
			TransactionStorage::store_with_cid_config(signed.clone(), cid_config, data),
			DispatchError::BadOrigin,
		);
		assert_noop!(
			TransactionStorage::force_renew(
				signed,
				TransactionRef::Position { block: 1, index: 0 },
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn signed_store_prefers_preimage_authorization_over_account() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];
		let content_hash = blake2_256(&data);

		// Setup: user has account authorization
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 0,
				transactions_allowance: 0,
			}
		);

		// Setup: preimage authorization also exists for the same content
		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::root(),
			content_hash,
			2000
		));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(content_hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 1,
			}
		);

		// Store the pre-authorized content using a signed transaction
		let call = Call::store { data: data.clone() };
		assert_ok!(TransactionStorage::validate_signed(&who, &call));
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));

		// Preimage auth was used (bytes incremented), account untouched.
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(content_hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 1,
			},
			"Preimage authorization should be consumed"
		);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 0,
				transactions_allowance: 0,
			},
			"Account authorization should remain unchanged"
		);

		// Different content has no matching preimage auth → falls back to account.
		let other_data = vec![99u8; 1000];
		let other_call = Call::store { data: other_data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &other_call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 1000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			},
			"Account authorization should be used for non-pre-authorized content"
		);
	});
}

#[test]
fn signed_store_falls_back_to_account_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];
		let different_hash = blake2_256(&[0u8; 100]); // Hash for different content

		// Setup: user has account authorization
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 0,
				transactions_allowance: 0,
			}
		);

		// Setup: preimage authorization exists but for DIFFERENT content
		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::root(),
			different_hash,
			1000
		));

		// Store content that doesn't have preimage authorization
		let call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));

		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			},
			"Account authorization should be consumed when no matching preimage auth"
		);
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(different_hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 1000,
				transactions: 0,
				transactions_allowance: 1,
			},
			"Unrelated preimage authorization should remain unchanged"
		);
	});
}

#[test]
fn content_hash_map_cleaned_on_expiry() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		assert!(TransactionByContentHash::get(content_hash).is_some());

		let proof_provider = || {
			let block_num = System::block_number();
			if block_num == 11 {
				let parent_hash = System::parent_hash();
				build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000]]).unwrap()
			} else {
				None
			}
		};

		// Advance past storage period; block 1 data expires at block 12
		run_to_block(12, proof_provider);
		assert!(TransactionByContentHash::get(content_hash).is_none());
	});
}

#[test]
fn signed_renew_uses_account_authorization() {
	// When no preimage authorization exists for the stored content, signed renew falls back
	// to account authorization. (The old test used preimage auth for the store and relied on
	// it being deleted on consumption — which no longer happens, so the setup is reworked to
	// use account auth end-to-end.)
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		// Setup: authorize and store via account authorization.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			},
		);

		run_to_block(3, || None);

		// No preimage authorization exists for the content hash — renew uses account auth.
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));

		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 2000,
				bytes_allowance: 4000,
				transactions: 2,
				transactions_allowance: 0,
			},
			"Account authorization should be consumed for renew when no preimage auth"
		);
	});
}

#[test]
fn content_hash_map_not_cleaned_if_renewed() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));

		// Renew at block 6, which updates the map to point to block 6
		run_to_block(6, || None);
		assert_ok!(TransactionStorage::force_renew(
			RuntimeOrigin::none(),
			TransactionRef::ContentHash(content_hash),
		));
		assert_eq!(TransactionByContentHash::get(content_hash), Some((6, 0)));

		let proof_provider = || {
			let block_num = System::block_number();
			if block_num == 11 || block_num == 16 {
				let parent_hash = System::parent_hash();
				build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000]]).unwrap()
			} else {
				None
			}
		};

		// Block 1 data expires at block 12, but the map should still point to block 6
		run_to_block(12, proof_provider);
		assert_eq!(TransactionByContentHash::get(content_hash), Some((6, 0)));

		// Block 6 data expires at block 17
		run_to_block(17, proof_provider);
		assert!(TransactionByContentHash::get(content_hash).is_none());
	});
}

#[test]
fn signed_renew_prefers_preimage_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];
		let content_hash = blake2_256(&data);

		// Setup: store data using account authorization.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));

		// Account auth now at the cap (still present, just fully used).
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			}
		);

		run_to_block(3, || None);

		// Authorize preimage.
		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::root(),
			content_hash,
			2000
		));

		assert_eq!(
			TransactionStorage::preimage_authorization_extent(content_hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 1,
			}
		);
		// Account auth was unaffected by the preimage authorize.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			}
		);

		// Renew using signed transaction - should prefer preimage authorization
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));

		assert_eq!(
			TransactionStorage::preimage_authorization_extent(content_hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 2000,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 1,
			},
			"Preimage authorization should be consumed for renew"
		);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			},
			"Account authorization should remain unchanged when preimage auth is used"
		);
	});
}

#[test]
fn store_with_cid_config_uses_custom_hashing() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![42u8; 2000];

		// Store with default config (Blake2b256 + raw codec 0x55)
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		let default_info = BlockTransactions::get().last().unwrap().clone();
		assert_eq!(default_info.hashing, HashingAlgorithm::Blake2b256);
		assert_eq!(default_info.cid_codec, 0x55);

		// Store with explicit SHA2-256 config
		let sha2_config = CidConfig { codec: 0x55, hashing: HashingAlgorithm::Sha2_256 };
		assert_ok!(TransactionStorage::store_with_cid_config(
			RuntimeOrigin::none(),
			sha2_config.clone(),
			data.clone(),
		));
		let sha2_info = BlockTransactions::get().last().unwrap().clone();
		assert_eq!(sha2_info.hashing, HashingAlgorithm::Sha2_256);
		assert_eq!(sha2_info.cid_codec, 0x55);
		// Content hashes differ because different hashing algorithms are used
		assert_ne!(default_info.content_hash, sha2_info.content_hash);

		// Store with explicit Blake2b256 config (same as default but explicitly set)
		let blake2_config = CidConfig { codec: 0x55, hashing: HashingAlgorithm::Blake2b256 };
		assert_ok!(TransactionStorage::store_with_cid_config(
			RuntimeOrigin::none(),
			blake2_config.clone(),
			data.clone(),
		));
		let blake2_info = BlockTransactions::get().last().unwrap().clone();
		assert_eq!(blake2_info.hashing, HashingAlgorithm::Blake2b256);
		assert_eq!(blake2_info.cid_codec, 0x55);
		assert_eq!(default_info.content_hash, blake2_info.content_hash);

		// Finalize block 1 and verify Transactions storage
		run_to_block(2, || None);
		let txs = Transactions::get(1).expect("transactions should be stored for block 1");
		assert_eq!(txs.len(), 3);
		assert_eq!(txs[0].hashing, HashingAlgorithm::Blake2b256);
		assert_eq!(txs[0].cid_codec, 0x55);
		assert_eq!(txs[1].hashing, HashingAlgorithm::Sha2_256);
		assert_eq!(txs[1].cid_codec, 0x55);
		assert_eq!(txs[2].hashing, HashingAlgorithm::Blake2b256);
		assert_eq!(txs[2].cid_codec, 0x55);
	});
}

#[test]
fn preimage_authorize_store_with_cid_config_and_renew() {
	new_test_ext().execute_with(|| {
		let data = vec![42u8; 2000];
		let sha2_config = CidConfig { codec: 0x55, hashing: HashingAlgorithm::Sha2_256 };
		let sha2_hash = polkadot_sdk_frame::hashing::sha2_256(&data);

		// check_unsigned / check_store_renew_unsigned use the CID config's hashing
		// algorithm for preimage authorization lookup.
		// Authorizing with blake2 hash should NOT work for store_with_cid_config(sha2).
		let blake2_hash = blake2_256(&data);
		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::root(),
			blake2_hash,
			2000
		));
		let store_call =
			Call::store_with_cid_config { cid: sha2_config.clone(), data: data.clone() };
		run_to_block(1, || None);
		assert_noop!(TransactionStorage::pre_dispatch(&store_call), InvalidTransaction::Payment);

		// Authorize preimage with SHA2 hash (matching the CID config's algorithm).
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), sha2_hash, 2000));

		// store_with_cid_config goes through check_unsigned → check_store_renew_unsigned.
		assert_ok!(TransactionStorage::pre_dispatch(&store_call));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(sha2_hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 1,
			}
		);
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));

		// sha2 preimage consumed to cap; entry stays.
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(sha2_hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 1,
				transactions_allowance: 1,
			}
		);
		// Blake2 authorization should remain unconsumed.
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(blake2_hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 1,
			}
		);

		// Finalize block so Transactions storage is populated.
		run_to_block(3, || None);

		// Verify stored entry uses SHA2-256 and content_hash matches.
		let txs = Transactions::get(1).expect("transactions stored at block 1");
		assert_eq!(txs.len(), 1);
		assert_eq!(txs[0].hashing, HashingAlgorithm::Sha2_256);
		assert_eq!(txs[0].cid_codec, 0x55);
		assert_eq!(txs[0].content_hash, sha2_hash);

		// Renew with the sha2 preimage auth still present — succeeds, accumulates on
		// `bytes_permanent` while leaving `bytes` (store-only) untouched.
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch(&renew_call));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(sha2_hash),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 2000,
				bytes_allowance: 2000,
				transactions: 2,
				transactions_allowance: 1,
			}
		);
	});
}

#[test]
fn validate_signed_account_authorization_has_provides_tag() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1u64;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));

		let call = Call::store { data: vec![0u8; 2000] };

		// validate_signed still doesn't consume authorization (correct behaviour).
		for _ in 0..2 {
			assert_ok!(TransactionStorage::validate_signed(&who, &call));
		}
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);

		let (vt, _) = TransactionStorage::validate_signed(&who, &call).unwrap();
		assert!(!vt.provides.is_empty(), "validate_signed must emit a `provides` tag");

		// Two calls with the same signer + content produce identical tags, confirming
		// that the mempool will deduplicate them.
		let (vt2, _) = TransactionStorage::validate_signed(&who, &call).unwrap();
		assert_eq!(vt.provides, vt2.provides);

		// Both pre_dispatch calls succeed: the entry stays and `bytes` saturates upward.
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 4000,
				bytes_permanent: 0,
				bytes_allowance: 2000,
				transactions: 2,
				transactions_allowance: 0,
			},
		);

		// Now test the preimage-authorized path: signed preimage tags must match unsigned
		// preimage tags so the pool deduplicates across both submission types.
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);
		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::root(),
			content_hash,
			2000,
		));
		// Re-authorize account so validate_signed can fall through if needed.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));

		let (signed_vt, _) = TransactionStorage::validate_signed(&who, &call).unwrap();
		let unsigned_vt = <TransactionStorage as ValidateUnsigned>::validate_unsigned(
			TransactionSource::External,
			&call,
		)
		.unwrap();
		assert_eq!(
			signed_vt.provides, unsigned_vt.provides,
			"signed preimage path must produce the same tag as unsigned preimage path"
		);

		// A different signer submitting the same pre-authorized content must get the same
		// tag, proving dedup is content-based, not signer-based.
		let other_who = 2u64;
		let (other_vt, _) = TransactionStorage::validate_signed(&other_who, &call).unwrap();
		assert_eq!(
			signed_vt.provides, other_vt.provides,
			"different signers with same preimage-authorized content must share the same tag"
		);
	});
}

// ---- Migration tests ----

/// Write old-format `OldTransactionInfo` entries as raw bytes into the `Transactions`
/// storage slot for `block_num`. Uses synthetic field values — the migration re-encodes
/// fields 1:1 without validating chunk roots or content hashes.
fn insert_old_format_transactions(block_num: u64, count: u32) {
	use polkadot_sdk_frame::deps::sp_runtime::traits::{BlakeTwo256, Hash};

	let old_txs: Vec<OldTransactionInfo> = (0..count)
		.map(|i| OldTransactionInfo {
			chunk_root: BlakeTwo256::hash(&[i as u8]),
			content_hash: BlakeTwo256::hash(&[i as u8 + 100]),
			size: 2000,
			block_chunks: (i + 1) * 8,
		})
		.collect();
	let bounded: BoundedVec<OldTransactionInfo, ConstU32<DEFAULT_MAX_BLOCK_TRANSACTIONS>> =
		old_txs.try_into().expect("within bounds");
	let key = Transactions::hashed_key_for(block_num);
	unhashed::put_raw(&key, &bounded.encode());
}

#[test]
fn migration_v1_old_entries_only() {
	new_test_ext().execute_with(|| {
		// Simulate pre-migration state: on-chain version 0
		StorageVersion::new(0).put::<TransactionStorage>();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(0));

		// Insert old-format entries at blocks 1, 2, 3
		insert_old_format_transactions(1, 2);
		insert_old_format_transactions(2, 1);
		insert_old_format_transactions(3, 3);

		// Can't decode with new type
		assert!(Transactions::get(1).is_none());
		assert!(Transactions::get(2).is_none());
		assert!(Transactions::get(3).is_none());

		// But raw bytes exist
		assert!(Transactions::contains_key(1));
		assert!(Transactions::contains_key(2));
		assert!(Transactions::contains_key(3));

		// Chain v0→v1 → v1→v2 → v2→v3 to bring entries to the current layout:
		// v1→v2 stamps `kind = Store` and `extrinsic_index = u32::MAX`; v2→v3 then
		// observes the final layout and is a version-bump no-op for these entries.
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(1));
		crate::migrations::v2::MigrateV1ToV2::<Test>::on_runtime_upgrade();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(2));
		drive_v2_to_v3_migration();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(3));

		let txs1 = Transactions::get(1).expect("should decode after v0→v3 chain");
		assert_eq!(txs1.len(), 2);
		for tx in txs1.iter() {
			assert_eq!(tx.hashing, HashingAlgorithm::Blake2b256);
			assert_eq!(tx.cid_codec, 0x55);
			assert_eq!(tx.size, 2000);
			assert_eq!(tx.kind, TransactionKind::Store, "pre-v2 entries default to Store");
			assert_eq!(tx.extrinsic_index, u32::MAX);
		}

		let txs2 = Transactions::get(2).expect("should decode");
		assert_eq!(txs2.len(), 1);

		let txs3 = Transactions::get(3).expect("should decode");
		assert_eq!(txs3.len(), 3);
	});
}

#[test]
fn migration_v1_new_entries_only() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(0).put::<TransactionStorage>();
		run_to_block(1, || None);

		// Store via normal (new-format) code path
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		run_to_block(2, || None);

		let original = Transactions::get(1).expect("should decode");
		assert_eq!(original.len(), 1);

		// Run migration
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();

		// Entry unchanged
		let after = Transactions::get(1).expect("should decode");
		assert_eq!(original, after);
	});
}

#[test]
fn migration_v1_mixed_entries() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(0).put::<TransactionStorage>();

		// Old-format entry at block 5
		insert_old_format_transactions(5, 2);
		assert!(Transactions::get(5).is_none());

		// New-format entry at block 10
		run_to_block(10, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![42u8; 500]));
		run_to_block(11, || None);
		let new_entry_before = Transactions::get(10).expect("new format decodes");

		// Chain v0→v1 → v1→v2 → v2→v3 so all entries (old and new) reach the current
		// `TransactionInfo` layout (kind + extrinsic_index sentinels).
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();
		crate::migrations::v2::MigrateV1ToV2::<Test>::on_runtime_upgrade();
		drive_v2_to_v3_migration();

		// Old entry transformed all the way to current layout — decodable as `TransactionInfo`.
		let old_entry_after = Transactions::get(5).expect("should decode after v0→v3 chain");
		assert_eq!(old_entry_after.len(), 2);
		for tx in old_entry_after.iter() {
			assert_eq!(tx.kind, TransactionKind::Store);
			assert_eq!(tx.extrinsic_index, u32::MAX);
		}

		// New entry was already in v1 layout (it was just stored); v1→v2 tail-extended it
		// with `kind = Store`. Field-by-field equality with the pre-migration v1 entry
		// won't hold (the kind field is new), but the original fields must round-trip.
		let new_entry_after = Transactions::get(10).expect("still decodes");
		assert_eq!(new_entry_after.len(), new_entry_before.len());
		assert_eq!(new_entry_after[0].chunk_root, new_entry_before[0].chunk_root);
		assert_eq!(new_entry_after[0].content_hash, new_entry_before[0].content_hash);
		assert_eq!(new_entry_after[0].size, new_entry_before[0].size);
		assert_eq!(new_entry_after[0].kind, TransactionKind::Store);
	});
}

#[test]
fn migration_v1_version_updated() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(0).put::<TransactionStorage>();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(0));
		assert_eq!(TransactionStorage::in_code_storage_version(), StorageVersion::new(5));

		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();

		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(1));
	});
}

#[test]
fn migration_v1_idempotent() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(0).put::<TransactionStorage>();
		insert_old_format_transactions(1, 1);

		// First run: migrates old entries to v1 format
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(1));
		// v1 format is not decodable as v2 TransactionInfo, but raw bytes exist
		let key = Transactions::hashed_key_for(1u64);
		let raw_after_first = unhashed::get_raw(&key).expect("raw bytes exist");

		// Second run: noop (version already 1)
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(1));
		let raw_after_second = unhashed::get_raw(&key).expect("raw bytes still exist");

		assert_eq!(raw_after_first, raw_after_second);
	});
}

#[test]
fn migration_v1_empty_storage() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(0).put::<TransactionStorage>();
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(0));

		// No Transactions entries exist
		assert_eq!(Transactions::iter().count(), 0);

		// Run migration
		crate::migrations::v1::MigrateV0ToV1::<Test>::on_runtime_upgrade();

		// Version updated, no entries created
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(1));
		assert_eq!(Transactions::iter().count(), 0);
	});
}

// ---- try_state tests ----

#[test]
fn try_state_passes_on_empty_storage() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

#[test]
fn try_state_passes_after_store_and_finalize() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![1u8; 500]));
		run_to_block(2, || None);
		// After finalization, ephemeral storage is cleared and transactions are persisted
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

#[test]
fn try_state_passes_through_retention_lifecycle() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		let proof_provider = || {
			let block_num = System::block_number();
			if block_num == 11 {
				let parent_hash = System::parent_hash();
				build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000]]).unwrap()
			} else {
				None
			}
		};
		// Run past retention period; block 1 transactions get cleaned up at block 12
		run_to_block(12, proof_provider);
		assert!(Transactions::get(1).is_none());
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

#[test]
fn try_state_passes_with_active_authorizations() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 10000));
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));

		// Partially consume authorization
		let call = Call::store { data: vec![0; 1000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &call));
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

#[test]
fn try_state_detects_zero_authorization_allowance() {
	// The only invariant left on stored authorizations is that `bytes_allowance > 0`; `bytes`
	// (used) can be any value (including over cap) since consumption saturates upward.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);

		// Authorization SCALE layout: extent(AuthorizationExtent), expiration(u64)
		// AuthorizationExtent SCALE layout: transactions(u32), transactions_allowance(u32),
		// bytes(u64), bytes_permanent(u64), bytes_allowance(u64)
		let corrupted_auth = (0u32, 0u32, 0u64, 0u64, 0u64, 100u64); // all zero counters, bytes_allowance=0, expiration=100
		let key = Authorizations::hashed_key_for(AuthorizationScope::Account(1u64));
		unhashed::put_raw(&key, &corrupted_auth.encode());

		assert_err!(
			TransactionStorage::do_try_state(System::block_number()),
			"Stored authorization has zero bytes_allowance"
		);
	});
}

#[test]
fn try_state_detects_zero_retention_period() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);

		// Set RetentionPeriod to zero
		RetentionPeriod::put(0u64);

		assert_err!(
			TransactionStorage::do_try_state(System::block_number()),
			"RetentionPeriod must not be zero"
		);
	});
}

#[test]
fn try_state_passes_with_preimage_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let hash = blake2_256(&[1u8; 32]);
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), hash, 5000));
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

/// Happy path for the hard-side invariant: a real `renew` keeps `PermanentStorageUsed`
/// equal to the sum of renewed `Transactions` entries' sizes.
#[test]
fn try_state_passes_after_renew() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data: vec![42u8; 2000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		run_to_block(3, || None);
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));
		assert_ok!(Into::<RuntimeCall>::into(renew_call).dispatch(RuntimeOrigin::none()));
		// Force the renewed entry into the persistent `Transactions` map by advancing a
		// block so `on_finalize` flushes `BlockTransactions`.
		run_to_block(4, || None);
		assert_eq!(PermanentStorageUsed::get(), 2000);
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

/// `renew` / `enable_auto_renew` charge `PermanentStorageUsed` up front but the
/// matching `Renew` entry only lands at the next retention boundary; `try_state`
/// must reconcile the counter against paid registrations in the meantime.
#[test]
fn try_state_passes_during_paid_auto_renewal_prepayment_window() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let data = vec![42u8; 2000];
		let content_hash = blake2_256(&data);
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		run_to_block(3, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::ContentHash(content_hash)));
		assert_eq!(PermanentStorageUsed::get(), 2000);
		assert!(AutoRenewals::get(content_hash).unwrap().paid);

		run_to_block(4, || None);
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

/// As above, for `enable_auto_renew`.
#[test]
fn try_state_passes_during_enable_auto_renew_prepayment_window() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let data = vec![42u8; 2000];
		let content_hash = blake2_256(&data);
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		run_to_block(3, || None);

		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		assert_eq!(PermanentStorageUsed::get(), 2000);
		assert!(AutoRenewals::get(content_hash).unwrap().paid);

		run_to_block(4, || None);
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

/// `PermanentStorageUsed` desync from `Σ size of renewed Transactions entries` is caught.
#[test]
fn try_state_detects_permanent_used_mismatch_with_transactions() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		// Bump the counter without writing any matching renewed `Transactions` entry.
		PermanentStorageUsed::put(2000);
		assert_err!(
			TransactionStorage::do_try_state(System::block_number()),
			"PermanentStorageUsed != Σ renewed sizes + Σ paid auto-renewal sizes"
		);
	});
}

/// `PermanentStorageUsed` over `MaxPermanentStorageSize` is caught.
#[test]
fn try_state_detects_permanent_used_exceeds_chain_cap() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		// Seed a renewed `Transactions` entry of 2000 bytes so the counter is reconciled
		// with stored state (matches the new invariant), then squeeze the cap below it.
		let dummy = TransactionInfo {
			chunk_root: Default::default(),
			content_hash: [0u8; 32],
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: 2000,
			extrinsic_index: u32::MAX,
			block_chunks: num_chunks(2000),
			kind: TransactionKind::Renew,
		};
		Transactions::insert(
			1u64,
			BoundedVec::<TransactionInfo, _>::try_from(vec![dummy]).unwrap(),
		);
		PermanentStorageUsed::put(2000);
		MaxPermanentStorageSize::set(&500);
		assert_err!(
			TransactionStorage::do_try_state(System::block_number()),
			"PermanentStorageUsed exceeds MaxPermanentStorageSize"
		);
	});
}

// ---- ValidateStorageCalls extension tests ----

#[test]
fn ensure_authorized_extracts_custom_origin() {
	new_test_ext().execute_with(|| {
		let who: u64 = 42;

		// 1. Authorized origin with Account scope
		let authorized_origin: RuntimeOrigin =
			Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
		assert_eq!(
			TransactionStorage::ensure_authorized(authorized_origin),
			Ok(AuthorizedCaller::Signed { who, scope: AuthorizationScope::Account(who) }),
		);

		// 2. Authorized origin with Preimage scope
		let content_hash = [0u8; 32];
		let preimage_origin: RuntimeOrigin = Origin::<Test>::Authorized {
			who: 99,
			scope: AuthorizationScope::Preimage(content_hash),
		}
		.into();
		assert_eq!(
			TransactionStorage::ensure_authorized(preimage_origin),
			Ok(AuthorizedCaller::Signed {
				who: 99,
				scope: AuthorizationScope::Preimage(content_hash)
			}),
		);

		// 3. Root origin → Root
		assert_eq!(
			TransactionStorage::ensure_authorized(RuntimeOrigin::root()),
			Ok(AuthorizedCaller::Root),
		);

		// 4. None origin → Unsigned
		assert_eq!(
			TransactionStorage::ensure_authorized(RuntimeOrigin::none()),
			Ok(AuthorizedCaller::Unsigned),
		);

		// 5. Plain signed origin → BadOrigin
		assert_eq!(
			TransactionStorage::ensure_authorized(RuntimeOrigin::signed(123)),
			Err(DispatchError::BadOrigin),
		);
	});
}

#[test]
fn authorize_storage_extension_transforms_origin() {
	use polkadot_sdk_frame::{
		prelude::TransactionSource,
		traits::{DispatchInfoOf, TransactionExtension, TxBaseImplication},
	};

	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let caller = 1u64;
		let data = vec![0u8; 16];

		// Give caller account authorization
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), caller, 0, 16));

		// Create the store call
		let call: RuntimeCall = Call::store { data }.into();
		let info: DispatchInfoOf<RuntimeCall> = Default::default();
		let origin = RuntimeOrigin::signed(caller);

		// Run ValidateStorageCalls::validate - this should transform the origin
		let ext = ValidateStorageCalls::<Test>::default();
		let result = ext.validate(
			origin,
			&call,
			&info,
			0,
			(),
			&TxBaseImplication(&call),
			TransactionSource::External,
		);

		assert!(result.is_ok());
		let (valid_tx, val, transformed_origin) = result.unwrap();

		// Verify the transaction is valid with correct priority
		assert_eq!(valid_tx.priority, StoreRenewPriority::get());

		// Verify val contains the signer
		assert_eq!(val, Some(caller));

		// Verify the origin was transformed and can be extracted with ensure_authorized
		let origin_for_prepare = transformed_origin.clone();
		assert_eq!(
			TransactionStorage::ensure_authorized(transformed_origin),
			Ok(AuthorizedCaller::Signed {
				who: caller,
				scope: AuthorizationScope::Account(caller)
			}),
		);

		// Run prepare — this should call pre_dispatch_signed and add to the used counter.
		let ext2 = ValidateStorageCalls::<Test>::default();
		assert_ok!(ext2.prepare(val, &origin_for_prepare, &call, &info, 0));

		// After prepare: 16 bytes used, entry at cap (not removed).
		assert_eq!(
			TransactionStorage::account_authorization_extent(caller),
			AuthorizationExtent {
				bytes: 16,
				bytes_permanent: 0,
				bytes_allowance: 16,
				transactions: 1,
				transactions_allowance: 0,
			},
		);
	});
}

#[test]
fn authorize_storage_extension_transforms_origin_with_preimage_auth() {
	use polkadot_sdk_frame::{
		prelude::TransactionSource,
		traits::{DispatchInfoOf, TransactionExtension, TxBaseImplication},
	};

	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let caller = 1u64;
		let data = vec![0u8; 16];
		let content_hash = blake2_256(&data);

		// Give preimage authorization (not account authorization)
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), content_hash, 16));

		// Create the store call
		let call: RuntimeCall = Call::store { data }.into();
		let info: DispatchInfoOf<RuntimeCall> = Default::default();
		let origin = RuntimeOrigin::signed(caller);

		// Run ValidateStorageCalls::validate
		let ext = ValidateStorageCalls::<Test>::default();
		let result = ext.validate(
			origin,
			&call,
			&info,
			0,
			(),
			&TxBaseImplication(&call),
			TransactionSource::External,
		);

		assert!(result.is_ok());
		let (_, val, transformed_origin) = result.unwrap();

		// Verify val contains the signer
		assert_eq!(val, Some(caller));

		// Verify the origin carries preimage authorization
		assert_eq!(
			TransactionStorage::ensure_authorized(transformed_origin),
			Ok(AuthorizedCaller::Signed {
				who: caller,
				scope: AuthorizationScope::Preimage(content_hash)
			}),
		);
	});
}

#[test]
fn authorize_storage_extension_passes_through_non_storage_calls() {
	use polkadot_sdk_frame::{
		prelude::{TransactionSource, ValidTransaction},
		traits::{AsSystemOriginSigner, DispatchInfoOf, TransactionExtension, TxBaseImplication},
	};

	new_test_ext().execute_with(|| {
		let caller = 1u64;

		// Create a non-TransactionStorage call (using System::remark as example)
		let call: RuntimeCall = frame_system::Call::remark { remark: vec![] }.into();
		let info: DispatchInfoOf<RuntimeCall> = Default::default();
		let origin = RuntimeOrigin::signed(caller);

		// Run ValidateStorageCalls::validate - should pass through unchanged
		let ext = ValidateStorageCalls::<Test>::default();
		let result = ext.validate(
			origin.clone(),
			&call,
			&info,
			0,
			(),
			&TxBaseImplication(&call),
			TransactionSource::External,
		);

		assert!(result.is_ok());
		let (valid_tx, val, returned_origin) = result.unwrap();

		// Verify passthrough behavior
		assert_eq!(valid_tx, ValidTransaction::default());
		assert_eq!(val, None);

		// Origin should still be a signed origin (not transformed)
		assert!(returned_origin.as_system_origin_signer().is_some());
		assert_eq!(returned_origin.as_system_origin_signer().unwrap(), &caller);
	});
}

/// Helper: initialize block N with proper extrinsic context for manual on_initialize + dispatch.
fn init_block(n: u64) {
	System::set_block_number(n);
	System::reset_events();
	// Set extrinsic index so sp_io::transaction_index::renew works
	unhashed::put::<u32>(b":extrinsic_index", &0);
	<TransactionStorage as polkadot_sdk_frame::traits::Hooks<u64>>::on_initialize(n);
}

type AutoRenewals = super::AutoRenewals<Test>;
type PendingAutoRenewals = super::PendingAutoRenewals<Test>;

#[test]
fn add_authorizer_inserts_overwrites_and_emits_event() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 42u64;
		// First insert.
		assert_ok!(TransactionStorage::add_authorizer(
			RuntimeOrigin::root(),
			who,
			test_budget(100, 1024),
		));
		assert_eq!(AllowedAuthorizers::<Test>::get(who).unwrap(), test_budget(100, 1024));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AuthorizerAdded { who }));

		// Second call with a different budget overwrites the first.
		assert_ok!(TransactionStorage::add_authorizer(
			RuntimeOrigin::root(),
			who,
			test_budget(200, 2048),
		));
		assert_eq!(AllowedAuthorizers::<Test>::get(who).unwrap(), test_budget(200, 2048));
	});
}

#[test]
fn remove_authorizer_removes_emits_event_and_ignores_absent() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 42u64;
		AllowedAuthorizers::<Test>::insert(who, test_budget(100, 1024));

		// Present → remove, emits event.
		assert_ok!(TransactionStorage::remove_authorizer(RuntimeOrigin::root(), who));
		assert!(!AllowedAuthorizers::<Test>::contains_key(who));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AuthorizerRemoved {
			who,
		}));

		// Absent → no-op success, no phantom event (state unchanged).
		let events_before = System::events().len();
		assert_ok!(TransactionStorage::remove_authorizer(RuntimeOrigin::root(), who));
		assert_eq!(System::events().len(), events_before);
	});
}

#[test]
fn add_remove_authorizer_reject_non_manager_origin() {
	new_test_ext().execute_with(|| {
		let who = 42u64;
		AllowedAuthorizers::<Test>::insert(who, test_budget(100, 1024));
		assert_noop!(
			TransactionStorage::add_authorizer(
				RuntimeOrigin::signed(1),
				who,
				test_budget(100, 1024),
			),
			DispatchError::BadOrigin,
		);
		assert_noop!(
			TransactionStorage::remove_authorizer(RuntimeOrigin::signed(1), who),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn ensure_allowed_authorizers_origin_rules() {
	new_test_ext().execute_with(|| {
		let registered = 7u64;
		AllowedAuthorizers::<Test>::insert(registered, test_budget(100, 1024));
		// Signed by a registered account → accepted, returns the full
		// `AuthorizationOrigin` carrying the authorizer, `valid_until` and `feeless`
		// (both `None`/`false` for `test_budget`).
		assert_eq!(
			EnsureAllowedAuthorizers::<Test>::try_origin(RuntimeOrigin::signed(registered)).ok(),
			Some(Some(AuthorizationOrigin {
				authorizer: registered,
				valid_until: None,
				feeless: false,
			})),
		);
		// Signed by an unregistered account, Root, and None all rejected.
		assert!(EnsureAllowedAuthorizers::<Test>::try_origin(RuntimeOrigin::signed(99)).is_err());
		assert!(EnsureAllowedAuthorizers::<Test>::try_origin(RuntimeOrigin::root()).is_err());
		assert!(EnsureAllowedAuthorizers::<Test>::try_origin(RuntimeOrigin::none()).is_err());
	});
}

#[test]
fn genesis_populates_allowed_authorizers() {
	let t = RuntimeGenesisConfig {
		system: Default::default(),
		transaction_storage: crate::GenesisConfig::<Test> {
			retention_period: 10,
			byte_fee: 2,
			entry_fee: 200,
			account_authorizations: vec![],
			preimage_authorizations: vec![],
			allowed_authorizers: vec![(1, 100, 1024), (2, 200, 2048)],
		},
	}
	.build_storage()
	.unwrap();
	TestExternalities::new(t).execute_with(|| {
		// Genesis authorizers default to `feeless: true`; root can re-add them
		// later to flip `feeless` or set a `valid_until`.
		let expected = |tx, by| AuthorizerBudget { feeless: true, ..test_budget(tx, by) };
		assert_eq!(AllowedAuthorizers::<Test>::iter().count(), 2);
		assert_eq!(AllowedAuthorizers::<Test>::get(1).unwrap(), expected(100, 1024));
		assert_eq!(AllowedAuthorizers::<Test>::get(2).unwrap(), expected(200, 2048));
	});
}

#[test]
fn enable_auto_renew_works() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Authorize and store. Note: store accepts unsigned origin (or the custom
		// Origin::Authorized set by ValidateStorageCalls extension). Plain signed origin
		// is rejected by ensure_authorized().
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		// Enable auto-renew via the full extension pipeline.
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Verify storage
		let renewal_data = AutoRenewals::get(content_hash).unwrap();
		assert_eq!(renewal_data.account, who);

		// Verify event
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::RenewalEnabled {
			content_hash,
			who,
			recurring: true,
		}));

		// Enabling again must be rejected at pre-dispatch (before bytes are
		// charged) with `AUTO_RENEWAL_ALREADY_ENABLED`. Charging twice would
		// leak `PermanentStorageUsed` because dispatch would not add a Renew
		// entry.
		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(crate::AUTO_RENEWAL_ALREADY_ENABLED.into()),
		);
	});
}

#[test]
fn enable_auto_renew_rejects_invalid() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;

		// Enabling for non-existent content hash is rejected at the extension level.
		let bogus_hash = blake2_256(&[99u8; 100]);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		let call = Call::enable_auto_renew { content_hash: bogus_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(crate::RENEWED_NOT_FOUND.into()),
		);

		// Enabling without account authorization fails. `check_authorization`
		// (with `is_renew = true`) folds both missing and expired authorizations
		// into `InvalidTransaction::Payment`, matching the one-shot `renew` path.
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		let unauthorized_user = 99;
		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&unauthorized_user, &call).map(|_| ()),
			Err(InvalidTransaction::Payment.into()),
		);
	});
}

#[test]
fn disable_auto_renew_works() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let owner = 1;
		let other = 2;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner,
			10,
			100_000
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(owner, content_hash));

		// Even the owner cannot disable while the registration is in its prepaid
		// window: `enable_auto_renew` charges the next cycle up front, and we
		// don't let that prepayment be reclaimed.
		assert_noop!(
			disable_auto_renew_via_extension(owner, content_hash),
			Error::CannotDisablePrepaidAutoRenewal,
		);

		// Fire the first cycle to consume the prepayment, after which the
		// registration sits at `paid = false` and the owner can disable.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid, "cycle should consume prepayment");

		// Non-owner is still rejected after the prepayment is consumed.
		assert_noop!(
			disable_auto_renew_via_extension(other, content_hash),
			Error::NotAutoRenewalOwner,
		);

		// Owner can now disable.
		assert_ok!(disable_auto_renew_via_extension(owner, content_hash));

		assert!(AutoRenewals::get(content_hash).is_none());
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalDisabled {
			content_hash,
			who: owner,
		}));
	});
}

/// Root bypasses the owner check AND the prepaid-window check (governance/cleanup
/// path) — even a fresh registration with `paid: true` can be torn down by Root.
#[test]
fn disable_auto_renew_root_override() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let owner = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner,
			10,
			100_000
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(owner, content_hash));
		// Registration is still in its prepaid window — Root must override anyway.
		assert!(AutoRenewals::get(content_hash).unwrap().paid);

		assert_ok!(TransactionStorage::disable_auto_renew(RuntimeOrigin::root(), content_hash));
		assert!(AutoRenewals::get(content_hash).is_none());
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalDisabled {
			content_hash,
			who: owner,
		}));
	});
}

#[test]
fn disable_auto_renew_fails_if_not_enabled() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let content_hash = blake2_256(&[99u8; 100]);

		assert_noop!(
			disable_auto_renew_via_extension(who, content_hash),
			Error::AutoRenewalNotEnabled,
		);
	});
}

/// `disable_auto_renew` is feeless: admission is gated in `check_signed` so spam is
/// bounded even without a token fee. Verify pool-level rejection mirrors the dispatch
/// errors — unknown content hash returns `AUTO_RENEWAL_NOT_ENABLED`, non-owner returns
/// `NOT_AUTO_RENEWAL_OWNER`, owner during prepaid window returns
/// `CANNOT_DISABLE_PREPAID_AUTO_RENEWAL`, and the owner is admitted only after the
/// first cycle has consumed the prepayment.
#[test]
fn disable_auto_renew_validate_signed_gates_on_ownership() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let owner = 1;
		let other = 2;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Unknown content hash: pool rejects with AUTO_RENEWAL_NOT_ENABLED.
		let bogus_hash = blake2_256(&[99u8; 100]);
		let call = Call::disable_auto_renew { content_hash: bogus_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&owner, &call).map(|_| ()),
			Err(crate::AUTO_RENEWAL_NOT_ENABLED.into()),
		);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner,
			10,
			100_000
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(owner, content_hash));

		// Non-owner: pool rejects with NOT_AUTO_RENEWAL_OWNER.
		let call = Call::disable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&other, &call).map(|_| ()),
			Err(crate::NOT_AUTO_RENEWAL_OWNER.into()),
		);

		// Owner during the prepaid window: rejected with CANNOT_DISABLE_PREPAID_AUTO_RENEWAL.
		assert_eq!(
			TransactionStorage::validate_signed(&owner, &call).map(|_| ()),
			Err(crate::CANNOT_DISABLE_PREPAID_AUTO_RENEWAL.into()),
		);

		// Fire the first cycle to consume the prepayment, then re-check pool admission.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			owner,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);

		// Owner post-cycle: pool admits.
		assert_ok!(TransactionStorage::validate_signed(&owner, &call));
	});
}

#[test]
fn auto_renewal_lifecycle() {
	new_test_ext().execute_with(|| {
		// Block 1: store data and enable auto-renew
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Registration is feeless and does not move the data: `TransactionByContentHash`
		// still points at the original `Store` at block 1.
		assert_eq!(TransactionByContentHash::get(content_hash), Some((1, 0)));
		assert!(Transactions::get(1).is_some());

		// Build proof provider for the retention boundary.
		let proof_provider = move || {
			let block_num = System::block_number();
			let period: u64 = RetentionPeriod::get();
			let target = block_num.saturating_sub(period);
			if target > 0 && Transactions::get(target).is_some() {
				let parent_hash = System::parent_hash();
				let txs = Transactions::get(target).unwrap();
				let data_vec: Vec<Vec<u8>> = txs.iter().map(|_| data.clone()).collect();
				build_proof(parent_hash.as_ref(), data_vec).unwrap()
			} else {
				None
			}
		};

		// Advance up to (but not including) block 12 — `run_to_block` handles
		// proofs and on_finalize for each block. The block-1 entry expires at
		// block 12; we want to stop before its on_initialize so we can drive
		// `apply_block_inherents` manually below.
		run_to_block(11, proof_provider);

		// Block 12: on_initialize takes `Transactions[1]` and schedules the
		// auto-renewal into `PendingAutoRenewals`.
		init_block(12);

		// Verify PendingAutoRenewals was populated
		let pending = PendingAutoRenewals::get();
		assert_eq!(pending.len(), 1);
		assert_eq!(pending[0].0, content_hash);

		// Process auto-renewals (simulating the mandatory extrinsic). Refresh
		// authorization first since the block-1 grant expired at block 11.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));

		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// Verify PendingAutoRenewals is now empty
		assert!(PendingAutoRenewals::get().is_empty());

		// Data was renewed into the current block.
		assert_eq!(TransactionByContentHash::get(content_hash), Some((12, 0)));

		// Verify event
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));

		// Old block-1 entry was taken in on_initialize.
		assert!(Transactions::get(1).is_none());

		// Recurring registration should still exist
		assert!(AutoRenewals::get(content_hash).is_some());
	});
}

/// A duplicate `store(data)` of content that already has an active
/// auto-renewal registration must not cause the owner to be billed for
/// an extra renewal cycle. The shadow `Transactions[S1]` entry left
/// behind by the duplicate store at `S2` is skipped by the
/// canonical-reference check in `on_initialize`, so only the canonical
/// (`S2`) chain fires renewals.
#[test]
fn duplicate_store_does_not_double_charge_auto_renew() {
	new_test_ext().execute_with(|| {
		// `new_test_ext` configures RetentionPeriod = 10.
		let alice = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// S1 = block 1: Alice stores and registers auto-renew at block 2.
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(alice, content_hash));
		assert_eq!(TransactionByContentHash::get(content_hash), Some((1, 0)));
		assert!(AutoRenewals::get(content_hash).unwrap().paid, "cycle 1 is prepaid");

		// S2 = block 5: anyone re-stores the same data. The map now
		// points at the latest (5, 0); the entry in Transactions[1]
		// becomes a stale shadow.
		run_to_block(5, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		assert_eq!(TransactionByContentHash::get(content_hash), Some((5, 0)));

		// Build proofs for the two age-out boundaries: block 11 ages
		// the proof for Transactions[1], block 15 for Transactions[5].
		let data_for_proof = data.clone();
		let proof_provider = move || {
			let block_num = System::block_number();
			let period: u64 = RetentionPeriod::get();
			let target = block_num.saturating_sub(period);
			if target > 0 && Transactions::get(target).is_some() {
				let parent_hash = System::parent_hash();
				let txs = Transactions::get(target).unwrap();
				let data_vec: Vec<Vec<u8>> = txs.iter().map(|_| data_for_proof.clone()).collect();
				build_proof(parent_hash.as_ref(), data_vec).unwrap()
			} else {
				None
			}
		};

		// Advance to block 11 so block-1's storage proof is checked
		// normally and Transactions[5] survives in the map.
		run_to_block(11, proof_provider.clone());

		// S1 + RP + 1 = 12. on_initialize takes Transactions[1] but
		// finds the entry is no longer the canonical reference for
		// `hash`, so it does *not* queue an auto-renewal. Without the
		// fix, this would queue one and consume Alice's prepayment.
		init_block(12);
		assert!(Transactions::get(1).is_none(), "stale Transactions[1] is taken");
		assert_eq!(
			TransactionByContentHash::get(content_hash),
			Some((5, 0)),
			"canonical pointer to (5, 0) is untouched"
		);
		assert!(
			PendingAutoRenewals::get().is_empty(),
			"stale shadow must not schedule an auto-renewal"
		);
		// Prepayment is intact — the legitimate cycle at block 16 will consume it.
		assert!(AutoRenewals::get(content_hash).unwrap().paid);

		// Drain the (empty) inherent so `on_finalize`'s pending-empty
		// invariant holds and block 12 finalizes cleanly.
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(System::events().iter().all(|r| !matches!(
			r.event,
			RuntimeEvent::TransactionStorage(Event::DataAutoRenewed { .. })
		)));

		// Advance to block 15 so the proof for Transactions[5] is
		// submitted before it ages out.
		run_to_block(15, proof_provider);

		// S2 + RP + 1 = 16. on_initialize takes Transactions[5] —
		// this *is* the canonical reference, so exactly one renewal
		// is queued and Alice's prepayment is consumed.
		init_block(16);
		let pending = PendingAutoRenewals::get();
		assert_eq!(pending.len(), 1, "exactly one auto-renewal for the canonical chain");
		assert_eq!(pending[0].0, content_hash);
		assert_eq!(pending[0].2.account, alice);

		// Refresh authorization (the block-5 grant expired at block 15)
		// and fire the cycle. It should succeed against the prepayment.
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: alice,
		}));
		// Cycle 1 ran free against the prepayment; subsequent cycles will charge.
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		assert_eq!(TransactionByContentHash::get(content_hash), Some((16, 0)));
	});
}

#[test]
fn auto_renewal_consumes_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// `store` is unsigned, so it does not consume authorization.
		// `enable_auto_renew` is feeless but pre-pays the first cycle, mirroring
		// one-shot `renew`: `bytes_permanent`, the chain-wide
		// `PermanentStorageUsed`, and one tx slot are all charged at registration.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 3, 6000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Registration prepaid the first cycle: `bytes_permanent = size` and one
		// tx slot consumed.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 2000,
				bytes_allowance: 6000,
				transactions: 1,
				transactions_allowance: 3,
			},
		);

		// First auto-renewal cycle fires when `Transactions[1]` ages out at
		// block 12. The block-1 grant expired at block 11, so the re-grant below
		// replaces it with fresh counters; the cycle then fires free against the
		// fresh auth thanks to `paid: true`.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 3, 6000));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// Cycle 1 (prepaid) leaves the fresh authorization untouched.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 6000,
				transactions: 0,
				transactions_allowance: 3,
			},
		);
		// Prepayment is consumed; subsequent cycles charge per-cycle.
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);

		// Cycle 2: carry `BlockTransactions[12]` → `Transactions[12]` so the
		// block-12 renew can age out at block 23.
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}
		init_block(23);
		// The block-12 grant expired at block 22; re-grant fresh counters so the
		// cycle can charge against a known baseline.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 3, 6000));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// Cycle 2 (paid = false) charged `size` bytes_permanent + 1 tx slot.
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 2000,
				bytes_allowance: 6000,
				transactions: 1,
				transactions_allowance: 3,
			},
		);
	});
}

#[test]
fn auto_renewal_fails_when_authorization_exhausted() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Authorize generously initially so the prepaid registration fits.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 5, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Cycle 1 (prepaid) fires free at block 12. Re-authorize first because
		// the block-1 grant expired at block 11.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 5, 100_000));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid, "cycle should consume prepayment");

		// Carry block-12 renew into Transactions[12] for the next age-out.
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Cycle 2 at block 23 runs against a refreshed authorization with no
		// headroom — `bytes_permanent + size > bytes_allowance` fires.
		init_block(23);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 1, 1000));
		let pending = PendingAutoRenewals::get();
		assert_eq!(pending.len(), 1, "Should have pending renewal");

		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// Should have failed — event emitted and auto-renewal removed.
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash,
			account: who,
		}));
		assert!(AutoRenewals::get(content_hash).is_none(), "Auto-renewal should be removed");
	});
}

#[test]
fn process_auto_renewals_rejects_signed_origin() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_noop!(
			TransactionStorage::apply_block_inherents(RuntimeOrigin::signed(1), None),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn process_auto_renewals_noop_when_empty() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		// Calling with no pending renewals should succeed (no-op)
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(PendingAutoRenewals::get().is_empty());
	});
}

#[test]
fn pending_auto_renewals_populated_only_for_registered_items() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data1 = vec![0u8; 2000];
		let data2 = vec![1u8; 2000];
		let hash1 = blake2_256(&data1);
		let _hash2 = blake2_256(&data2);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data1));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data2));
		run_to_block(2, || None);

		// Only enable auto-renew for hash1, not hash2
		assert_ok!(enable_auto_renew_via_extension(who, hash1));

		// Block 12: block-1 entries age out. on_initialize iterates them and
		// schedules only the entry with an `AutoRenewals` registration.
		init_block(12);

		let pending = PendingAutoRenewals::get();
		assert_eq!(pending.len(), 1, "Only hash1 should be pending");
		assert_eq!(pending[0].0, hash1);
	});
}

#[test]
fn auto_renew_permissionless_transfer() {
	// Alice stores and enables auto-renew, waits out the prepaid window so the
	// first cycle consumes her prepayment, then disables. Bob enables instead.
	// Anyone can take over keeping data alive on Bulletin, permissionlessly —
	// but only after the original registrant's pre-paid cycle has fired.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let alice = 1;
		let bob = 2;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Authorize and store as Alice
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);

		// Alice enables auto-renew (prepays the first cycle).
		assert_ok!(enable_auto_renew_via_extension(alice, content_hash));
		let renewal = AutoRenewals::get(content_hash).unwrap();
		assert_eq!(renewal.account, alice);
		assert!(renewal.paid);

		// Alice cannot disable during the prepaid window.
		assert_noop!(
			disable_auto_renew_via_extension(alice, content_hash),
			Error::CannotDisablePrepaidAutoRenewal,
		);

		// Fire the first cycle to consume Alice's prepayment.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		// Carry the cycle-12 renew out of `BlockTransactions` so the next
		// `enable_auto_renew` can resolve the `(12, 0)` index against
		// `Transactions[12]` (mirrors what `on_finalize` does in a live chain).
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Alice can now disable.
		assert_ok!(disable_auto_renew_via_extension(alice, content_hash));
		assert!(AutoRenewals::get(content_hash).is_none());

		// Bob authorizes and enables auto-renew for the same content. The renew
		// at block 12 made `(12, 0)` the latest `TransactionByContentHash` entry,
		// so `enable_auto_renew` resolves cleanly.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), bob, 10, 100_000));
		assert_ok!(enable_auto_renew_via_extension(bob, content_hash));

		let renewal = AutoRenewals::get(content_hash).unwrap();
		assert_eq!(renewal.account, bob, "Bob should now own the auto-renewal");
		assert!(renewal.paid, "Bob's registration prepays his first cycle");

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::RenewalEnabled {
			content_hash,
			who: bob,
			recurring: true,
		}));
	});
}

#[test]
fn process_auto_renewals_continues_on_per_item_failure() {
	// Verify that if one renewal fails (e.g. block full), the remaining items are still processed.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;

		// Store MaxBlockTransactions items to fill the block later
		let max_txns = <<Test as crate::Config>::MaxBlockTransactions as Get<u32>>::get();
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			max_txns + 10,
			100_000_000
		));

		let mut hashes = Vec::new();
		for i in 0..3u8 {
			let data = vec![i; 2000];
			let content_hash = blake2_256(&data);
			hashes.push(content_hash);
			assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		}
		run_to_block(2, || None);

		// Enable auto-renew for all three (feeless registration — only consumes
		// one tx slot each).
		for hash in &hashes {
			assert_ok!(enable_auto_renew_via_extension(who, *hash));
		}

		// Block-1 entries age out at block 12; on_initialize schedules each as
		// a pending auto-renewal. Refresh authorization (block-1 grant expired
		// at block 11).
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			max_txns + 10,
			100_000_000
		));

		// Verify PendingAutoRenewals was populated with 3 items
		let pending = PendingAutoRenewals::get();
		assert_eq!(pending.len(), 3);

		// Fill block with (max - 1) dummy transactions so only 1 renewal fits
		BlockTransactions::mutate(|txns| {
			for _ in 0..(max_txns - 1) {
				let _ = txns.try_push(TransactionInfo {
					chunk_root: Default::default(),
					size: 100,
					content_hash: [0u8; 32],
					hashing: crate::HashingAlgorithm::Blake2b256,
					cid_codec: 0x55,
					extrinsic_index: 0,
					block_chunks: 0,
					kind: crate::TransactionKind::Store,
				});
			}
		});

		// Process auto-renewals — should NOT return an error even though 2 of 3 fail
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// PendingAutoRenewals should be fully consumed
		assert!(PendingAutoRenewals::get().is_empty());

		// First item should have succeeded (DataAutoRenewed event).
		// Index is max_txns - 1 because the block already has max_txns - 1 items (0-indexed).
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: max_txns - 1,
			content_hash: hashes[0],
			account: who,
		}));

		// Remaining items should have failed (AutoRenewalFailed events)
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash: hashes[1],
			account: who,
		}));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash: hashes[2],
			account: who,
		}));

		// Auto-renewal registrations should be removed for failed items
		assert!(AutoRenewals::get(hashes[1]).is_none());
		assert!(AutoRenewals::get(hashes[2]).is_none());
	});
}

/// `paid = true` cycle rejected by the per-block slot cap refunds chain-wide
/// `PermanentStorageUsed`. Per-account `bytes_permanent` / `transactions` are
/// intentionally left burned — see the inline rationale in `do_process_auto_renewals`.
#[test]
fn paid_cycle_refunds_on_block_slot_cap() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![7u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		assert_eq!(PermanentStorageUsed::get(), 2000);
		let auth = Authorizations::get(AuthorizationScope::Account(who)).expect("auth exists");
		let permanent_before = auth.extent.bytes_permanent;
		let transactions_before = auth.extent.transactions;

		init_block(12);
		assert_eq!(PendingAutoRenewals::get().len(), 1);

		// Fill `BlockTransactions` so the paid drain has no slot to land in.
		let max_txns = <<Test as crate::Config>::MaxBlockTransactions as Get<u32>>::get();
		BlockTransactions::mutate(|txns| {
			for _ in 0..max_txns {
				let _ = txns.try_push(TransactionInfo {
					chunk_root: Default::default(),
					size: 100,
					content_hash: [0u8; 32],
					hashing: crate::HashingAlgorithm::Blake2b256,
					cid_codec: 0x55,
					extrinsic_index: 0,
					block_chunks: 0,
					kind: crate::TransactionKind::Store,
				});
			}
		});

		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash,
			account: who,
		}));
		assert!(AutoRenewals::get(content_hash).is_none());

		assert_eq!(PermanentStorageUsed::get(), 0);
		let auth = Authorizations::get(AuthorizationScope::Account(who)).expect("auth exists");
		assert_eq!(auth.extent.bytes_permanent, permanent_before);
		assert_eq!(auth.extent.transactions, transactions_before);

		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

/// `renew` registers a one-shot renewal — `AutoRenewals[hash]` is created with
/// `recurring = false` and `RenewalEnabled { recurring: false }` fires.
#[test]
fn renew_schedules_one_shot() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::Position { block: 1, index: 0 }));

		let entry = AutoRenewals::get(content_hash).unwrap();
		assert_eq!(entry.account, who);
		assert!(!entry.recurring, "renew should register a one-shot entry");
		assert!(entry.paid, "one-shot is prepaid at registration");

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::RenewalEnabled {
			content_hash,
			who,
			recurring: false,
		}));
	});
}

/// A one-shot fires exactly once: after the renewal cycle the `AutoRenewals` entry is
/// removed, even on success. Distinct from forever auto-renewal which keeps firing.
#[test]
fn one_shot_fires_once_then_unregisters() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(renew_via_extension(who, TransactionRef::Position { block: 1, index: 0 }));

		// Fire the renewal cycle.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		// DataAutoRenewed fired AND the registration was consumed.
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));
		assert!(
			AutoRenewals::get(content_hash).is_none(),
			"one-shot registration must be removed after firing"
		);
	});
}

/// `renew` (one-shot) charges `bytes_permanent` + `PermanentStorageUsed` + 1 tx slot
/// at registration — same hard-cap accounting as `force_renew`.
#[test]
fn renew_prepays_at_registration() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		run_to_block(2, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::Position { block: 1, index: 0 }));

		let auth = Authorizations::get(AuthorizationScope::Account(who)).unwrap();
		assert_eq!(auth.extent.bytes_permanent, 2000);
		assert_eq!(auth.extent.transactions, 1);
		assert_eq!(PermanentStorageUsed::get(), 2000);
	});
}

/// Pre-payment caps spam: a second one-shot that would push `bytes_permanent` past
/// `bytes_allowance` is rejected at pool ingress.
#[test]
fn renew_rejects_when_quota_exhausted() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let hash_a = blake2_256(&[0u8; 2000][..]);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 2000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![0u8; 2000]));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![1u8; 2000]));
		run_to_block(2, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::ContentHash(hash_a)));

		let call = Call::renew { entry: TransactionRef::Position { block: 1, index: 1 } };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(crate::PERMANENT_ALLOWANCE_EXCEEDED.into()),
		);
	});
}

/// One-shot cycle delivers the Renew entry without re-charging auth (slot pre-paid
/// at registration). Contrast with recurring `enable_auto_renew` which charges
/// per cycle — see `auto_renewal_consumes_authorization`.
#[test]
fn one_shot_cycle_does_not_recharge_auth() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 3, 6000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(renew_via_extension(who, TransactionRef::Position { block: 1, index: 0 }));

		// Read raw `Authorizations` directly — the public extent helper masks expired entries.
		let before = Authorizations::get(AuthorizationScope::Account(who)).unwrap();
		init_block(12);
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));
		let after = Authorizations::get(AuthorizationScope::Account(who)).unwrap();
		assert_eq!((after.extent, after.expiration), (before.extent, before.expiration));
	});
}

/// Once a registration exists (one-shot or recurring), neither `renew` nor
/// `enable_auto_renew` for the same hash may overwrite it.
#[test]
fn renew_and_enable_auto_renew_conflict() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		assert_ok!(renew_via_extension(who, TransactionRef::Position { block: 1, index: 0 }));
		let permanent_used_before = PermanentStorageUsed::get();

		// Duplicate `renew` is rejected at the extension before any charge.
		let dup_call = Call::renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &dup_call).map(|_| ()),
			Err(crate::AUTO_RENEWAL_ALREADY_ENABLED.into()),
		);
		assert_eq!(PermanentStorageUsed::get(), permanent_used_before);

		// Defensive dispatch-level guard still rejects if the extension is bypassed.
		let origin: RuntimeOrigin =
			Origin::<Test>::Authorized { who, scope: AuthorizationScope::Account(who) }.into();
		assert_noop!(
			TransactionStorage::renew(origin, TransactionRef::Position { block: 1, index: 0 }),
			Error::AutoRenewalAlreadyEnabled,
		);

		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(crate::AUTO_RENEWAL_ALREADY_ENABLED.into()),
		);
	});
}

/// `renew` requires an authorized-signed origin: Root and Unsigned are rejected with
/// `BadOrigin` (registration would have no account to record).
#[test]
fn renew_rejects_unsigned_and_root_origin() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![0u8; 2000];
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		let entry = TransactionRef::Position { block: 1, index: 0 };
		assert_noop!(
			TransactionStorage::renew(RuntimeOrigin::none(), entry.clone()),
			DispatchError::BadOrigin,
		);
		assert_noop!(
			TransactionStorage::renew(RuntimeOrigin::root(), entry),
			DispatchError::BadOrigin,
		);
	});
}

/// Run a normal block lifecycle past expiry without invoking `apply_block_inherents`.
///
/// `on_initialize` populates `PendingAutoRenewals`; `on_finalize` then enforces that the
/// inherent ran, asserting that the storage is empty. The mock's `run_to_block` always
/// invokes the inherent, hiding this safeguard. This test bypasses the helper to confirm
/// the assert actually fires when an auto-renewal is pending and the inherent is missing.
#[test]
#[should_panic(expected = "All pending auto-renewals must be processed by apply_block_inherents")]
fn on_finalize_panics_when_inherent_missing() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		// `run_to_block`'s own on_finalize will flush BlockTransactions into
		// `Transactions[block]` as it advances through subsequent blocks.

		let proof_provider = move || {
			let block_num = System::block_number();
			let period: u64 = RetentionPeriod::get();
			let target = block_num.saturating_sub(period);
			if target > 0 && Transactions::get(target).is_some() {
				let parent_hash = System::parent_hash();
				let txs = Transactions::get(target).unwrap();
				let data_vec: Vec<Vec<u8>> = txs.iter().map(|_| data.clone()).collect();
				build_proof(parent_hash.as_ref(), data_vec).unwrap()
			} else {
				None
			}
		};

		// Run normally up to (and including) block 12 — proofs supplied via the inherent.
		// The block-1 `Store` entry expires at block 12 but the latest-entry guard
		// skips it (force-renew at block 2 is the latest), so no PendingAutoRenewals
		// build up here. The force-renewed entry ages out at block 13.
		run_to_block(12, proof_provider);

		// Manually advance to block 13 and run only on_initialize, which populates
		// PendingAutoRenewals as Transactions(2) expires. We deliberately do NOT call
		// apply_block_inherents, simulating an inherent that was lost or never built.
		init_block(13);
		assert_eq!(
			PendingAutoRenewals::get().len(),
			1,
			"on_initialize should have populated pending"
		);

		// on_finalize must panic on the PendingAutoRenewals invariant.
		<TransactionStorage as polkadot_sdk_frame::traits::Hooks<u64>>::on_finalize(13);
	});
}

/// Verify that `ProvideInherent::create_inherent` actually emits the composite inherent call
/// when `PendingAutoRenewals` is non-empty, even with no storage proof in `InherentData`.
///
/// This is the direct test for "the block author will inject the inherent that drains pending
/// renewals" — if `create_inherent` ever stops returning the call when only renewals (and no
/// proof) are pending, the chain would panic at on_finalize without any test catching it.
#[test]
fn create_inherent_emits_call_when_pending_renewals_present() {
	use polkadot_sdk_frame::{deps::sp_inherents::InherentData, runtime::prelude::ProvideInherent};

	new_test_ext().execute_with(|| {
		run_to_block(1, || None);

		// Baseline: no proof, no pending renewals → no inherent emitted.
		let empty = InherentData::new();
		assert!(
			<TransactionStorage as ProvideInherent>::create_inherent(&empty).is_none(),
			"no inherent should be emitted when neither proof nor pending renewals are present",
		);

		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data.clone()));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// The block-1 `Store` is no longer the latest reference after the
		// force-renew, so the latest-entry guard skips it at block 12. The
		// force-renewed entry ages out at block 13, populating
		// `PendingAutoRenewals` then. We need a proof provider because block 12
		// needs a proof for the (still-on-chain) block-2 Renew entry.
		let proof_provider = move || {
			let block_num = System::block_number();
			let period: u64 = RetentionPeriod::get();
			let target = block_num.saturating_sub(period);
			if target > 0 && Transactions::get(target).is_some() {
				let parent_hash = System::parent_hash();
				let txs = Transactions::get(target).unwrap();
				let data_vec: Vec<Vec<u8>> = txs.iter().map(|_| data.clone()).collect();
				build_proof(parent_hash.as_ref(), data_vec).unwrap()
			} else {
				None
			}
		};
		run_to_block(12, proof_provider);
		init_block(13);
		assert_eq!(PendingAutoRenewals::get().len(), 1);

		// `InherentData` carries no proof. The provider must still emit the composite call so
		// that the inherent-driven drain runs in this block.
		let result = <TransactionStorage as ProvideInherent>::create_inherent(&empty);
		match result {
			Some(Call::apply_block_inherents { proof: None }) => {},
			other => panic!(
				"expected Some(apply_block_inherents {{ proof: None }}) when only pending renewals \
				 are present, got {other:?}"
			),
		}
	});
}

#[test]
fn re_authorize_account_adds_to_allowance_and_keeps_expiry() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let call = Call::store { data: vec![0; 2000] };
		// Initial authorization at block 1: expires at block 1 + 10 = 11.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 2000));

		// Re-authorize at block 5 within the unexpired window: the new `bytes` add to the
		// existing cap, expiry stays at 11.
		run_to_block(5, || None);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 3000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);

		// Still valid at block 10.
		run_to_block(10, || None);
		assert_ok!(TransactionStorage::validate_signed(&who, &call));

		// Expires at block 11 (original expiry, not pushed back).
		run_to_block(11, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 0,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
		assert_noop!(TransactionStorage::validate_signed(&who, &call), InvalidTransaction::Payment);
	});
}

#[test]
fn re_authorize_account_preserves_used_bytes() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		// Initial 4000-byte cap, consume 2000.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store = Call::store { data: vec![0; 2000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store));

		// Add another 1000: cap becomes 5000, used stays at 2000.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 5000,
				transactions: 1,
				transactions_allowance: 0,
			},
		);
	});
}

#[test]
fn re_authorize_account_after_expiry_resets() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		// Initial authorization at block 1: expires at block 11. Consume some bytes.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store = Call::store { data: vec![0; 2000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store));

		// Re-authorize after expiry: replaces with a fresh entry (zero used, new expiry).
		run_to_block(20, || None);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1500));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 1500,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
	});
}

#[test]
fn authorize_preimage_does_not_push_expiry() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let data = vec![0u8; 2000];
		let hash = blake2_256(&data);
		let call = Call::store { data };
		// Initial authorization at block 1: expires at block 1 + 10 = 11.
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), hash, 2000));

		// Re-authorize at block 5 with larger max_size: expiration should stay at 11.
		// Preimage re-authorize takes max(existing, new) for `bytes_allowance`.
		run_to_block(5, || None);
		assert_ok!(TransactionStorage::authorize_preimage(RuntimeOrigin::root(), hash, 3000));
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 3000,
				transactions: 0,
				transactions_allowance: 1,
			},
		);

		// Still valid at block 10.
		run_to_block(10, || None);
		assert_ok!(TransactionStorage::validate_signed(&1, &call));

		// Expires at block 11 (original expiry), NOT 15.
		run_to_block(11, || None);
		assert_eq!(
			TransactionStorage::preimage_authorization_extent(hash),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 0,
				transactions: 0,
				transactions_allowance: 0,
			},
		);
	});
}

/// `refresh_account_authorization` only extends expiration — it does NOT reset any
/// consumed counters (`bytes`, `bytes_permanent`, `transactions`). In particular,
/// `bytes_permanent` MUST be left intact: permanent storage stays on chain across refresh
/// cycles, so its accounting cannot be erased. See the comment in `refresh_authorization`.
#[test]
fn refresh_does_not_reset_consumed_counters() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		// Authorize: all counters start at 0.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 0,
				transactions_allowance: 0,
			},
		);

		// Store: bumps `bytes` and `transactions`; `bytes_permanent` untouched.
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 0,
				bytes_allowance: 4000,
				transactions: 1,
				transactions_allowance: 0,
			},
			"store must advance `bytes` and `transactions`"
		);

		run_to_block(3, || None);

		// Renew: bumps `bytes_permanent` and `transactions`; `bytes` untouched.
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 2000,
				bytes_allowance: 4000,
				transactions: 2,
				transactions_allowance: 0,
			},
			"renew must advance `bytes_permanent` and `transactions`"
		);

		// Refresh: all consumed counters preserved; only expiration moves.
		assert_ok!(TransactionStorage::refresh_account_authorization(RuntimeOrigin::root(), who));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 2000,
				bytes_permanent: 2000,
				bytes_allowance: 4000,
				transactions: 2,
				transactions_allowance: 0,
			},
			"refresh must not reset any consumed counters"
		);
	});
}

/// `authorize_account` on an expired-but-present entry resets **all** consumed counters,
/// including `bytes_permanent`. The new window's renew quota is independent of any
/// renewed bytes still on chain from the old window; those are tracked by the chain-wide
/// `PermanentStorageUsed` counter and aged out by `on_initialize`.
#[test]
fn authorize_account_after_expiry_resets_bytes_permanent() {
	new_test_ext().execute_with(|| {
		run_to_block(5, || None);
		let who = 1;

		// Authorize and seed `bytes_permanent = 2000` directly (simulates a past renew).
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		Authorizations::mutate(AuthorizationScope::Account(who), |maybe_auth| {
			let auth = maybe_auth.as_mut().expect("authorization present");
			auth.extent.bytes_permanent = 2000;
			// Force expiry without advancing blocks.
			auth.expiration = 1;
		});

		// Re-authorize: cap is re-granted, all consumed counters reset to 0.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 1000,
				transactions: 0,
				transactions_allowance: 0,
			},
			"re-authorize after expiry resets every consumed counter",
		);
	});
}

/// `remove_expired_account_authorization` succeeds even when there is renewed data
/// outstanding from the old window: the chain-wide `PermanentStorageUsed` counter and
/// `Transactions` are the source of truth for renewed bytes; the per-account
/// `bytes_permanent` is just a per-window quota and removing the entry is safe.
#[test]
fn remove_expired_account_authorization_succeeds_with_outstanding_renewals() {
	new_test_ext().execute_with(|| {
		run_to_block(5, || None);
		let who = 1;

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		Authorizations::mutate(AuthorizationScope::Account(who), |maybe_auth| {
			let auth = maybe_auth.as_mut().expect("authorization present");
			auth.extent.bytes_permanent = 2000;
			auth.expiration = 1;
		});

		assert_ok!(TransactionStorage::remove_expired_account_authorization(
			RuntimeOrigin::none(),
			who,
		));
		assert!(!Authorizations::contains_key(AuthorizationScope::Account(who)));
	});
}

/// A successful renew bumps the chain-wide `PermanentStorageUsed` counter and is recorded
/// in `BlockTransactions` with `kind == Renew` so the obsolete-block cleanup in
/// `on_initialize` can later decrement the counter.
#[test]
fn renew_bumps_permanent_used_and_records_kind() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));

		assert_eq!(PermanentStorageUsed::get(), 0, "store must not bump permanent counter");
		// `BlockTransactions` holds the in-progress block's entries; the store entry must
		// have `kind = Store`.
		let block_txs = BlockTransactions::get();
		assert_eq!(block_txs.len(), 1);
		assert_eq!(block_txs[0].kind, TransactionKind::Store);

		run_to_block(3, || None);

		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));
		assert_ok!(Into::<RuntimeCall>::into(renew_call).dispatch(RuntimeOrigin::none()));

		assert_eq!(
			PermanentStorageUsed::get(),
			2000,
			"renew must bump the chain-wide permanent counter",
		);
		let block_txs = BlockTransactions::get();
		assert_eq!(block_txs.len(), 1);
		assert_eq!(block_txs[0].kind, TransactionKind::Renew);
	});
}

/// `renew` rejects with [`PERMANENT_ALLOWANCE_EXCEEDED`] when the per-account hard cap is
/// reached: `bytes_permanent + size > bytes_allowance`. The chain-wide counter must remain
/// untouched.
#[test]
fn renew_rejects_when_per_account_allowance_exceeded() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		// Allowance is below `size`, so renew must reject. Store still succeeds because the
		// non-renew path is the soft side — overshoot is allowed (and demoted in priority by
		// `AllowanceBasedPriority`).
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 1500));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));

		run_to_block(3, || None);

		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&who, &renew_call),
			PERMANENT_ALLOWANCE_EXCEEDED,
		);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who).bytes_permanent,
			0,
			"rejected renew must not bump bytes_permanent",
		);
		assert_eq!(PermanentStorageUsed::get(), 0, "rejected renew must not bump chain counter");
	});
}

// ---- v1 → v2 multi-block migration tests ----

/// Drive the v1→v2 stepped migration to completion against the test externalities.
fn drive_v2_to_v3_migration() {
	use crate::migrations::v3::MigrateV2ToV3;
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::SteppedMigration, weights::WeightMeter,
	};

	let mut meter = WeightMeter::new();
	let mut cursor: Option<<MigrateV2ToV3<Test> as SteppedMigration>::Cursor> = None;
	loop {
		cursor = MigrateV2ToV3::<Test>::step(cursor, &mut meter).expect("MBM step must not fail");
		if cursor.is_none() {
			break;
		}
	}
}

/// Insert a `BoundedVec<V2TransactionInfo, _>` raw blob under
/// `Transactions::hashed_key_for(block)`. `count` items are produced with synthetic field values.
fn insert_v2_format_transactions(block: u64, count: u32) {
	use crate::migrations::v3::V2TransactionInfo;
	use polkadot_sdk_frame::deps::sp_runtime::traits::{BlakeTwo256, Hash};

	let v2_txs: Vec<V2TransactionInfo> = (0..count)
		.map(|i| V2TransactionInfo {
			chunk_root: BlakeTwo256::hash(&[i as u8]),
			content_hash: BlakeTwo256::hash(&[i as u8 + 100]).into(),
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: 2000,
			block_chunks: (i + 1) * 8,
		})
		.collect();
	let bounded: BoundedVec<V2TransactionInfo, ConstU32<DEFAULT_MAX_BLOCK_TRANSACTIONS>> =
		v2_txs.try_into().expect("within bounds");
	let key = Transactions::hashed_key_for(block);
	unhashed::put_raw(&key, &bounded.encode());
}

#[test]
fn migrate_v2_to_v3_sets_sentinel_for_existing_entries() {
	use crate::migrations::v3::MigrateV2ToV3;
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::SteppedMigration, weights::WeightMeter,
	};
	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();
		insert_v2_format_transactions(1, 3);

		let mut meter = WeightMeter::new();
		let mut cursor: Option<<MigrateV2ToV3<Test> as SteppedMigration>::Cursor> = None;
		loop {
			cursor = MigrateV2ToV3::<Test>::step(cursor, &mut meter).expect("step should not fail");
			if cursor.is_none() {
				break;
			}
		}

		let txs = Transactions::get(1).expect("entry decodes as v2 after migration");
		assert_eq!(txs.len(), 3);
		for tx in txs.iter() {
			assert_eq!(tx.extrinsic_index, u32::MAX);
			assert_eq!(tx.size, 2000);
			assert_eq!(tx.hashing, HashingAlgorithm::Blake2b256);
			assert_eq!(tx.cid_codec, 0x55);
		}

		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(3));
	});
}

#[test]
fn migrate_v2_to_v3_resumes_across_steps() {
	use crate::{migrations::v3::MigrateV2ToV3, weights::WeightInfo};
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::SteppedMigration, weights::WeightMeter,
	};
	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();
		for block in 1..=20u64 {
			insert_v2_format_transactions(block, 1);
		}

		let per_entry_weight = <Test as crate::Config>::WeightInfo::migrate_v2_to_v3_step();
		let mut total_steps = 0u32;
		let mut cursor: Option<<MigrateV2ToV3<Test> as SteppedMigration>::Cursor> = None;
		loop {
			let mut meter = WeightMeter::with_limit(per_entry_weight.saturating_mul(5));
			cursor = MigrateV2ToV3::<Test>::step(cursor, &mut meter).expect("step should not fail");
			total_steps += 1;
			if cursor.is_none() {
				break;
			}
			assert!(total_steps < 100, "migration must converge");
		}
		assert!(total_steps >= 2, "expected ≥2 step calls; got {total_steps}");

		for block in 1..=20u64 {
			let txs = Transactions::get(block).expect("entry decodes as v2");
			assert_eq!(txs.len(), 1);
			assert_eq!(txs[0].extrinsic_index, u32::MAX);
		}
		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(3));
	});
}

#[test]
fn migrate_v2_to_v3_insufficient_weight_returns_err() {
	use crate::migrations::v3::MigrateV2ToV3;
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::{SteppedMigration, SteppedMigrationError},
		weights::WeightMeter,
	};
	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();
		insert_v2_format_transactions(1, 1);

		let mut meter = WeightMeter::with_limit(Weight::zero());
		let res = MigrateV2ToV3::<Test>::step(None, &mut meter);
		assert!(
			matches!(res, Err(SteppedMigrationError::InsufficientWeight { .. })),
			"expected InsufficientWeight, got {res:?}",
		);
	});
}

/// `renew` rejects with [`CHAIN_PERMANENT_CAP_REACHED`] when the chain-wide hard cap is
/// reached: `PermanentStorageUsed + size > MaxPermanentStorageSize`. Per-account state
/// must remain untouched.
#[test]
fn renew_rejects_when_chain_wide_cap_reached() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));

		// Lower the chain-wide cap below what a renewal would require.
		MaxPermanentStorageSize::set(&1000);

		run_to_block(3, || None);

		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&who, &renew_call),
			CHAIN_PERMANENT_CAP_REACHED,
		);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who).bytes_permanent,
			0,
			"rejected renew must not bump bytes_permanent",
		);
		assert_eq!(PermanentStorageUsed::get(), 0, "rejected renew must not bump chain counter");
	});
}

/// `on_initialize` decrements `PermanentStorageUsed` by exactly the sum of `Renew`-kind
/// entries when the obsolete block is removed; `Store`-kind entries do not contribute.
#[test]
fn on_initialize_decrements_permanent_used_when_block_obsoletes() {
	new_test_ext().execute_with(|| {
		// Seed `Transactions[3]` with one Store + one Renew so we can verify the kind
		// filter on cleanup. `block_chunks` is cumulative.
		let store_size: u32 = 1500;
		let renew_size: u32 = 2000;
		let store_chunks = num_chunks(store_size);
		let store_entry = TransactionInfo {
			chunk_root: Default::default(),
			content_hash: [0u8; 32],
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: store_size,
			extrinsic_index: u32::MAX,
			block_chunks: store_chunks,
			kind: TransactionKind::Store,
		};
		let renew_entry = TransactionInfo {
			chunk_root: Default::default(),
			content_hash: [0u8; 32],
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: renew_size,
			extrinsic_index: u32::MAX,
			block_chunks: store_chunks + num_chunks(renew_size),
			kind: TransactionKind::Renew,
		};
		Transactions::insert(
			3u64,
			BoundedVec::<TransactionInfo, _>::try_from(vec![store_entry, renew_entry]).unwrap(),
		);
		PermanentStorageUsed::put(2000);

		// `RetentionPeriod = 10`. At block 14, `obsolete = 14 - 11 = 3` so `Transactions[3]`
		// is removed and the renewed 2000 bytes are subtracted.
		System::set_block_number(14);
		<TransactionStorage as Hooks<u64>>::on_initialize(14);

		assert_eq!(PermanentStorageUsed::get(), 0, "renewed bytes must be decremented");
		assert!(Transactions::get(3).is_none(), "obsolete block must be removed");
	});
}

/// Renews landing in different blocks each contribute to the counter and each decrement
/// independently as their respective blocks become obsolete.
#[test]
fn renews_across_multiple_blocks_decrement_independently() {
	new_test_ext().execute_with(|| {
		let renew_entry = |size: u32| TransactionInfo {
			chunk_root: Default::default(),
			content_hash: [0u8; 32],
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size,
			extrinsic_index: u32::MAX,
			block_chunks: num_chunks(size),
			kind: TransactionKind::Renew,
		};
		// 1000 bytes renewed at block 3, 700 at block 5.
		Transactions::insert(
			3u64,
			BoundedVec::<TransactionInfo, _>::try_from(vec![renew_entry(1000)]).unwrap(),
		);
		Transactions::insert(
			5u64,
			BoundedVec::<TransactionInfo, _>::try_from(vec![renew_entry(700)]).unwrap(),
		);
		PermanentStorageUsed::put(1700);

		// Block 14: obsolete = 3 → drop 1000.
		System::set_block_number(14);
		<TransactionStorage as Hooks<u64>>::on_initialize(14);
		assert_eq!(PermanentStorageUsed::get(), 700);

		// Block 16: obsolete = 5 → drop 700.
		System::set_block_number(16);
		<TransactionStorage as Hooks<u64>>::on_initialize(16);
		assert_eq!(PermanentStorageUsed::get(), 0);
	});
}

/// End-to-end: hit the chain-wide cap with renews; advance past `RetentionPeriod`;
/// `on_initialize` decrements the counter; new renews succeed again. This is the
/// self-correcting bound on chain-wide renewed bytes.
#[test]
fn chain_wide_cap_self_corrects_after_age_out() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, u64::MAX,));
		MaxPermanentStorageSize::set(&2000);

		// Renew 2000 bytes at block 1 → counter at cap.
		let store_call = Call::store { data: vec![0u8; 2000] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		run_to_block(2, || None);
		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));
		assert_ok!(Into::<RuntimeCall>::into(renew_call).dispatch(RuntimeOrigin::none()));
		assert_eq!(PermanentStorageUsed::get(), 2000);

		// Another renew now must reject — chain cap reached.
		run_to_block(3, || None);
		let store_call_b = Call::store { data: vec![0u8; 100] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call_b));
		assert_ok!(Into::<RuntimeCall>::into(store_call_b).dispatch(RuntimeOrigin::none()));
		run_to_block(4, || None);
		let renew_call_b =
			Call::force_renew { entry: TransactionRef::Position { block: 3, index: 0 } };
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&who, &renew_call_b),
			CHAIN_PERMANENT_CAP_REACHED,
		);

		// Advance past `RetentionPeriod` (10) so the obsolete-block cleanup decrements
		// the counter. `on_finalize` requires a storage proof in any block whose
		// `target = n - 10` has `Transactions[target]` non-empty: blocks 1, 2, 3 each
		// got a transaction, so we provide proofs at blocks 11, 12, 13 respectively.
		let proof_provider = || {
			let parent_hash = System::parent_hash();
			let block_num = System::block_number();
			match block_num {
				11 | 12 => build_proof(parent_hash.as_ref(), vec![vec![0u8; 2000]]).unwrap(),
				13 => build_proof(parent_hash.as_ref(), vec![vec![0u8; 100]]).unwrap(),
				_ => None,
			}
		};
		run_to_block(13, proof_provider);
		assert_eq!(PermanentStorageUsed::get(), 0, "counter must self-correct as data ages out");

		// Renew now succeeds again. Mock `AuthorizationPeriod = 10`, so the original
		// authorization (granted at block 1) expired at block 11. Re-authorize for the
		// new window before driving another store/renew.
		run_to_block(14, proof_provider);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, u64::MAX,));
		let store_call_c = Call::store { data: vec![0u8; 500] };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call_c));
		assert_ok!(Into::<RuntimeCall>::into(store_call_c).dispatch(RuntimeOrigin::none()));
		run_to_block(15, proof_provider);
		let renew_call_c =
			Call::force_renew { entry: TransactionRef::Position { block: 14, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call_c));
		assert_ok!(Into::<RuntimeCall>::into(renew_call_c).dispatch(RuntimeOrigin::none()));
		assert_eq!(PermanentStorageUsed::get(), 500);
	});
}

/// Renew emits `PermanentStorageUsedUpdated { used }` so off-chain capacity-planning
/// dashboards can track the chain-wide counter without polling storage.
#[test]
fn renew_emits_permanent_storage_used_updated() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![42u8; 2000];

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, 4000));
		let store_call = Call::store { data };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
		assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
		run_to_block(3, || None);

		let renew_call =
			Call::force_renew { entry: TransactionRef::Position { block: 1, index: 0 } };
		assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));

		System::assert_has_event(RuntimeEvent::TransactionStorage(
			Event::PermanentStorageUsedUpdated { used: 2000 },
		));
	});
}

/// `on_initialize` cleanup emits a single `PermanentStorageUsedUpdated` event per obsolete
/// block (not per renewed entry within the block) — keeps event volume bounded.
#[test]
fn on_initialize_emits_single_used_updated_event_per_obsolete_block() {
	new_test_ext().execute_with(|| {
		let renew_entry = |size: u32, block_chunks: ChunkIndex| TransactionInfo {
			chunk_root: Default::default(),
			content_hash: [0u8; 32],
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size,
			extrinsic_index: u32::MAX,
			block_chunks,
			kind: TransactionKind::Renew,
		};
		let chunks_per = num_chunks(500);
		Transactions::insert(
			3u64,
			BoundedVec::<TransactionInfo, _>::try_from(vec![
				renew_entry(500, chunks_per),
				renew_entry(500, 2 * chunks_per),
				renew_entry(500, 3 * chunks_per),
			])
			.unwrap(),
		);
		PermanentStorageUsed::put(1500);

		System::set_block_number(14);
		System::reset_events();
		<TransactionStorage as Hooks<u64>>::on_initialize(14);

		let count = System::events()
			.iter()
			.filter(|r| {
				matches!(
					r.event,
					RuntimeEvent::TransactionStorage(Event::PermanentStorageUsedUpdated { .. })
				)
			})
			.count();
		assert_eq!(count, 1, "exactly one used-updated event per cleanup, not per entry");
	});
}

/// `PermanentStorageNearCap` fires once on the rising edge across the threshold and is
/// **not** re-emitted while still above the threshold. Decrementing back below and rising
/// again re-arms the signal.
#[test]
fn permanent_storage_near_cap_fires_on_rising_edge_only() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;

		// Cap = 1000; threshold = 1000 * 80 / 100 = 800.
		MaxPermanentStorageSize::set(&1000);

		// Generous per-account allowance so renews are only gated by the chain-wide cap.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 0, u64::MAX,));

		// Helper: store `size` bytes at the current block, advance one block, then renew it.
		// Captures the store block so the renew always points at the just-stored tx, not
		// some earlier one.
		let store_and_renew = |size: usize| {
			let store_block = System::block_number();
			let store_call = Call::store { data: vec![0u8; size] };
			assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &store_call));
			assert_ok!(Into::<RuntimeCall>::into(store_call).dispatch(RuntimeOrigin::none()));
			run_to_block(store_block + 1, || None);
			let renew_call = Call::force_renew {
				entry: TransactionRef::Position { block: store_block, index: 0 },
			};
			assert_ok!(TransactionStorage::pre_dispatch_signed(&who, &renew_call));
		};

		// Step 1: 500 bytes (PermanentStorageUsed: 0 → 500). Below threshold; no near-cap.
		store_and_renew(500);
		assert_eq!(PermanentStorageUsed::get(), 500);
		let evs = System::events();
		assert!(!evs.iter().any(|r| matches!(
			r.event,
			RuntimeEvent::TransactionStorage(Event::PermanentStorageNearCap { .. })
		)));

		// Step 2: +400 bytes (500 → 900). Crosses 800 threshold → near-cap fires.
		System::reset_events();
		store_and_renew(400);
		assert_eq!(PermanentStorageUsed::get(), 900);
		System::assert_has_event(RuntimeEvent::TransactionStorage(
			Event::PermanentStorageNearCap { used: 900, cap: 1000 },
		));

		// Quick sanity check on the threshold formula matching the constant.
		assert_eq!(PERMANENT_STORAGE_NEAR_CAP_PERCENT, 80);
	});
}

#[cfg(feature = "try-runtime")]
#[test]
fn migrate_v2_to_v3_post_upgrade_allows_pruned_entries() {
	use crate::migrations::v3::MigrateV2ToV3;
	use polkadot_sdk_frame::deps::frame_support::migrations::SteppedMigration;

	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();
		insert_v2_format_transactions(1, 1);
		insert_v2_format_transactions(2, 1);
		insert_v2_format_transactions(3, 1);

		let state = MigrateV2ToV3::<Test>::pre_upgrade().expect("pre_upgrade succeeds");

		Transactions::remove(2u64);
		drive_v2_to_v3_migration();

		MigrateV2ToV3::<Test>::post_upgrade(state).expect("pruned entries are allowed");
	});
}

#[test]
fn migrate_v2_to_v3_skips_already_v3_entries() {
	use crate::migrations::v3::MigrateV2ToV3;
	use polkadot_sdk_frame::deps::{
		frame_support::{migrations::SteppedMigration, weights::WeightMeter},
		sp_runtime::traits::{BlakeTwo256, Hash},
	};
	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();

		// Block 1: pre-migration v1 layout.
		insert_v2_format_transactions(1, 1);
		// Block 2: already-v2 layout, written by current code paths.
		let v2_tx = TransactionInfo {
			chunk_root: BlakeTwo256::hash(&[42]),
			content_hash: BlakeTwo256::hash(&[43]).into(),
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: 999,
			extrinsic_index: 7, // distinct from u32::MAX so we can detect corruption
			block_chunks: 4,
			kind: TransactionKind::Store,
		};
		let v2_bounded: BoundedVec<TransactionInfo, ConstU32<DEFAULT_MAX_BLOCK_TRANSACTIONS>> =
			vec![v2_tx.clone()].try_into().unwrap();
		Transactions::insert(2u64, v2_bounded);

		// Drive migration to completion.
		let mut meter = WeightMeter::new();
		let mut cursor: Option<<MigrateV2ToV3<Test> as SteppedMigration>::Cursor> = None;
		loop {
			cursor = MigrateV2ToV3::<Test>::step(cursor, &mut meter).expect("step should not fail");
			if cursor.is_none() {
				break;
			}
		}

		// Block 1: migrated v1 → v2 with sentinel.
		let txs1 = Transactions::get(1).expect("decodes as v2");
		assert_eq!(txs1[0].extrinsic_index, u32::MAX);

		// Block 2: untouched — original `extrinsic_index = 7` preserved.
		let txs2 = Transactions::get(2).expect("decodes as v2");
		assert_eq!(txs2[0].extrinsic_index, 7);
		assert_eq!(txs2[0].size, 999);

		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(3));
	});
}

/// `post_upgrade` must accept pre-existing v4 entries (e.g. prepaid `paid=true`) that `step`
/// leaves untouched. Regression for the summit migration check.
#[cfg(feature = "try-runtime")]
#[test]
fn migrate_v3_to_v4_post_upgrade_allows_preexisting_v4_paid_entries() {
	use crate::migrations::v4::{MigrateV3ToV4, V3AutoRenewalData};
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::SteppedMigration, weights::WeightMeter,
	};

	new_test_ext().execute_with(|| {
		StorageVersion::new(3).put::<TransactionStorage>();

		// A genuine v3 entry (just `{ account }`), written raw at the AutoRenewals key.
		let v3_hash: super::ContentHash = [1u8; 32];
		unhashed::put(&AutoRenewals::hashed_key_for(v3_hash), &V3AutoRenewalData { account: 1u64 });

		// A pre-existing v4 entry with `paid=true` — the case that broke on summit.
		let v4_hash: super::ContentHash = [2u8; 32];
		AutoRenewals::insert(
			v4_hash,
			super::RenewalData { account: 2u64, recurring: true, paid: true },
		);

		let state = MigrateV3ToV4::<Test>::pre_upgrade().expect("pre_upgrade succeeds");

		let mut meter = WeightMeter::new();
		let mut cursor: Option<<MigrateV3ToV4<Test> as SteppedMigration>::Cursor> = None;
		loop {
			cursor = MigrateV3ToV4::<Test>::step(cursor, &mut meter).expect("step should not fail");
			if cursor.is_none() {
				break;
			}
		}

		MigrateV3ToV4::<Test>::post_upgrade(state).expect("pre-existing v4 entries are allowed");

		let migrated = AutoRenewals::get(v3_hash).expect("v3 entry migrated");
		assert!(migrated.recurring && !migrated.paid);
		let preexisting = AutoRenewals::get(v4_hash).expect("v4 entry preserved");
		assert!(preexisting.paid, "pre-existing v4 paid=true entry must be left untouched");

		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(4));
	});
}

/// Re-running `step` against state already at/beyond v4 must not downgrade the storage version.
#[test]
fn migrate_v3_to_v4_does_not_downgrade_storage_version() {
	use crate::migrations::v4::MigrateV3ToV4;
	use polkadot_sdk_frame::deps::frame_support::{
		migrations::SteppedMigration, weights::WeightMeter,
	};

	new_test_ext().execute_with(|| {
		// Chain is already at v5 (e.g. summit), with a v4-format entry present.
		StorageVersion::new(5).put::<TransactionStorage>();
		AutoRenewals::insert(
			[2u8; 32] as super::ContentHash,
			super::RenewalData { account: 2u64, recurring: true, paid: true },
		);

		let mut meter = WeightMeter::new();
		let mut cursor: Option<<MigrateV3ToV4<Test> as SteppedMigration>::Cursor> = None;
		loop {
			cursor = MigrateV3ToV4::<Test>::step(cursor, &mut meter).expect("step should not fail");
			if cursor.is_none() {
				break;
			}
		}

		assert_eq!(
			TransactionStorage::on_chain_storage_version(),
			StorageVersion::new(5),
			"migration must not downgrade the storage version",
		);
	});
}

/// Stale `Transactions[block]` leftovers (block < current - RetentionPeriod) — e.g. from
/// a chain whose `RetentionPeriod` was previously longer — must be pruned by the v2→v3
/// migration rather than carried forward, otherwise `try_state` rejects them.
#[test]
fn migrate_v2_to_v3_prunes_stale_entries() {
	new_test_ext().execute_with(|| {
		StorageVersion::new(2).put::<TransactionStorage>();
		// Default `RetentionPeriod` in mock is 10. Run to block 50 so blocks 1..=39 are
		// "stale" (block < 50 - 10 = 40) and blocks 40..=50 are still in retention.
		System::set_block_number(50);

		insert_v2_format_transactions(1, 1); // stale
		insert_v2_format_transactions(20, 1); // stale
		insert_v2_format_transactions(40, 1); // in retention
		insert_v2_format_transactions(45, 1); // in retention

		drive_v2_to_v3_migration();

		assert!(Transactions::get(1).is_none(), "stale entry must be pruned");
		assert!(Transactions::get(20).is_none(), "stale entry must be pruned");
		assert!(Transactions::get(40).is_some(), "in-retention entry must be migrated");
		assert!(Transactions::get(45).is_some(), "in-retention entry must be migrated");
		assert_eq!(Transactions::get(40).unwrap()[0].extrinsic_index, u32::MAX);

		assert_eq!(TransactionStorage::on_chain_storage_version(), StorageVersion::new(3));

		// `do_try_state` must accept the post-migration state (no stale entries left).
		assert_ok!(TransactionStorage::do_try_state(System::block_number()));
	});
}

#[test]
fn transactions_at_decodes_v2_entry_with_sentinel() {
	new_test_ext().execute_with(|| {
		insert_v2_format_transactions(5, 2);

		// Direct `Transactions::get` cannot decode v2-shape bytes as the live (v3) layout.
		assert!(Transactions::get(5).is_none());

		let txs = TransactionStorage::transactions_at(5)
			.expect("v2 entries decode through transactions_at");
		assert_eq!(txs.len(), 2);
		for tx in txs.iter() {
			assert_eq!(tx.extrinsic_index, u32::MAX);
			assert_eq!(tx.size, 2000);
		}

		// The on-chain storage MUST be untouched: read-only API path does not write.
		assert!(Transactions::get(5).is_none());
	});
}

#[test]
fn transactions_at_handles_mixed_v2_and_v3_entries() {
	use polkadot_sdk_frame::deps::sp_runtime::traits::{BlakeTwo256, Hash};
	new_test_ext().execute_with(|| {
		// Block 1: pre-migration v2-shape (no `extrinsic_index`).
		insert_v2_format_transactions(1, 2);
		assert!(Transactions::get(1).is_none(), "v2 bytes do not decode as v3");

		// Block 2: live v3-shape entry — written by current code paths.
		let v3_tx = TransactionInfo {
			chunk_root: BlakeTwo256::hash(&[42]),
			content_hash: BlakeTwo256::hash(&[43]).into(),
			hashing: HashingAlgorithm::Blake2b256,
			cid_codec: 0x55,
			size: 999,
			extrinsic_index: 7,
			block_chunks: 4,
			kind: TransactionKind::Store,
		};
		let v3_bounded: BoundedVec<TransactionInfo, ConstU32<DEFAULT_MAX_BLOCK_TRANSACTIONS>> =
			vec![v3_tx.clone()].try_into().unwrap();
		Transactions::insert(2u64, v3_bounded);

		// Empty: a block with no entry returns None.
		assert!(TransactionStorage::transactions_at(99).is_none());

		// Slow path: v2 entry promoted to v3 with sentinel.
		let txs1 = TransactionStorage::transactions_at(1).expect("v2 entry decodes");
		assert_eq!(txs1.len(), 2);
		for tx in txs1.iter() {
			assert_eq!(tx.extrinsic_index, u32::MAX);
			assert_eq!(tx.size, 2000);
		}

		// Fast path: v3 entry returned verbatim, real `extrinsic_index` preserved.
		let txs2 = TransactionStorage::transactions_at(2).expect("v3 entry decodes");
		assert_eq!(txs2.len(), 1);
		assert_eq!(txs2[0].extrinsic_index, 7);
		assert_eq!(txs2[0].size, 999);

		// Read-only contract: storage shapes are unchanged after the read.
		assert!(Transactions::get(1).is_none(), "v2 entry must remain v2-shape on disk");
		assert_eq!(
			Transactions::get(2)
				.expect("v3 entry still decodes")
				.into_iter()
				.next()
				.unwrap(),
			v3_tx,
			"v3 entry must be byte-identical pre/post read",
		);
	});
}

// ---- Authorizer budget tests ----

#[test]
fn remove_exhausted_authorizer_removes_zero_budget_entries() {
	// Any of: both zero, transactions zero, bytes zero — all qualify as "exhausted".
	for (tx, bytes) in [(0, 0), (0, 1000), (100, 0)] {
		new_test_ext().execute_with(|| {
			run_to_block(1, || None);
			let who = 42u64;
			AllowedAuthorizers::<Test>::insert(who, test_budget(tx, bytes));

			let call = Call::remove_exhausted_authorizer { who };
			assert_ok!(TransactionStorage::pre_dispatch(&call));
			assert_ok!(
				TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), who,)
			);
			assert!(!AllowedAuthorizers::<Test>::contains_key(who));
			System::assert_has_event(RuntimeEvent::TransactionStorage(
				Event::ExhaustedAuthorizerRemoved { who },
			));
		});
	}
}

#[test]
fn remove_exhausted_authorizer_rejects_when_not_removable() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);

		// Missing entry → AuthorizerNotFound (mempool + dispatch agree).
		let call = Call::remove_exhausted_authorizer { who: 99u64 };
		assert_noop!(TransactionStorage::pre_dispatch(&call), AUTHORIZER_NOT_FOUND);
		assert_noop!(
			TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), 99u64),
			Error::AuthorizerNotFound,
		);

		// Present with non-zero budget + no expiry → AuthorizerBudgetNotExhausted.
		let who = 42u64;
		AllowedAuthorizers::<Test>::insert(who, test_budget(10, 1000));
		let call = Call::remove_exhausted_authorizer { who };
		assert_noop!(TransactionStorage::pre_dispatch(&call), AUTHORIZATION_NOT_EXHAUSTED);
		assert_noop!(
			TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), who),
			Error::AuthorizerBudgetNotExhausted,
		);
		assert!(AllowedAuthorizers::<Test>::contains_key(who));
	});
}

#[test]
fn add_authorizer_valid_until() {
	new_test_ext().execute_with(|| {
		run_to_block(5, || None);
		// `valid_until` is absolute and must be strictly in the future.
		let ok = AuthorizerBudget { valid_until: Some(25), ..test_budget(100, 1024) };
		assert_ok!(TransactionStorage::add_authorizer(RuntimeOrigin::root(), 42u64, ok));
		assert_eq!(AllowedAuthorizers::<Test>::get(42u64).unwrap().valid_until, Some(25));

		// Reject `== now` (expired immediately) and `< now` (already past).
		for t in [5, 0] {
			let bad = AuthorizerBudget { valid_until: Some(t), ..test_budget(100, 1024) };
			assert_noop!(
				TransactionStorage::add_authorizer(RuntimeOrigin::root(), 99u64, bad),
				Error::InvalidValidUntil,
			);
		}
		assert!(!AllowedAuthorizers::<Test>::contains_key(99u64));
	});
}

#[test]
fn expired_authorizer_cannot_authorize() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let authorizer = 10u64;
		// `valid_until = 6` → authorizes through block 5, expires at block 6.
		let budget = AuthorizerBudget { valid_until: Some(6), ..test_budget(100, 10_000) };
		assert_ok!(TransactionStorage::add_authorizer(RuntimeOrigin::root(), authorizer, budget,));
		let call = Call::authorize_account { who: 1, transactions: 1, bytes: 1000 };

		// Still valid at block 5.
		run_to_block(5, || None);
		assert_ok!(TransactionStorage::pre_dispatch_signed(&authorizer, &call));

		// Expired at block 6 (now >= valid_until).
		run_to_block(6, || None);
		assert_noop!(
			TransactionStorage::pre_dispatch_signed(&authorizer, &call),
			InvalidTransaction::BadSigner,
		);
	});
}

#[test]
fn remove_exhausted_authorizer_works_for_expired() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 42u64;
		// Has budget but expires at block 6.
		let budget = AuthorizerBudget { valid_until: Some(6), ..test_budget(100, 1000) };
		assert_ok!(TransactionStorage::add_authorizer(RuntimeOrigin::root(), who, budget));

		// Before expiry: removal rejected (budget present, not expired).
		assert_noop!(
			TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), who),
			Error::AuthorizerBudgetNotExhausted,
		);

		// After expiry: removal succeeds.
		run_to_block(6, || None);
		assert_ok!(TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), who));
		assert!(!AllowedAuthorizers::<Test>::contains_key(who));
		System::assert_has_event(RuntimeEvent::TransactionStorage(
			Event::ExhaustedAuthorizerRemoved { who },
		));
	});
}

#[test]
fn authorizer_budget_decrements_on_authorize() {
	// `authorize_account` consumes `(transactions, bytes)`; `authorize_preimage` is
	// equivalent to consuming `(1, max_size)`. Budget consumption happens inside
	// the dispatch body, so we dispatch via `Signed(authorizer)` directly.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let authorizer = 10u64;
		AllowedAuthorizers::<Test>::insert(authorizer, test_budget(5, 10_000));

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::signed(authorizer),
			1,
			2,
			4000,
		));
		assert_eq!(AllowedAuthorizers::<Test>::get(authorizer).unwrap(), test_budget(3, 6000));

		assert_ok!(TransactionStorage::authorize_preimage(
			RuntimeOrigin::signed(authorizer),
			[0u8; 32],
			3000,
		));
		assert_eq!(AllowedAuthorizers::<Test>::get(authorizer).unwrap(), test_budget(2, 3000));
	});
}

#[test]
fn authorizer_budget_insufficient_rejects_without_writing() {
	// Both axes (transactions, bytes) gate independently; on rejection the budget
	// must be unchanged (`try_mutate` rolls back when the closure returns Err).
	let scenarios = [
		// (initial_budget, transactions, bytes)
		(test_budget(1, 10_000), 5, 1000),
		(test_budget(100, 500), 1, 1000),
	];
	for (initial, transactions, bytes) in scenarios {
		new_test_ext().execute_with(|| {
			run_to_block(1, || None);
			let authorizer = 10u64;
			AllowedAuthorizers::<Test>::insert(authorizer, initial.clone());
			assert_noop!(
				TransactionStorage::authorize_account(
					RuntimeOrigin::signed(authorizer),
					1,
					transactions,
					bytes,
				),
				Error::InsufficientAuthorizerBudget,
			);
			assert_eq!(AllowedAuthorizers::<Test>::get(authorizer).unwrap(), initial);
		});
	}
}

#[test]
fn root_bypasses_authorizer_budget() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		// Root can authorize without being in AllowedAuthorizers
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), 1, 10, 10_000));
		assert_eq!(
			TransactionStorage::account_authorization_extent(1),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 10_000,
				transactions: 0,
				transactions_allowance: 10,
			},
		);
	});
}

#[test]
fn valid_until_clamps_authorization_expiry() {
	// Mock's `AuthorizationPeriod = 10`. An authorizer with `valid_until = 6`
	// (issued at block 1) should produce an authorization that expires at block
	// 6, not at block 11 — a grant cannot outlive its grantor.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let authorizer = 10u64;
		AllowedAuthorizers::<Test>::insert(
			authorizer,
			AuthorizerBudget { valid_until: Some(6), ..test_budget(100, 100_000) },
		);

		let who = 1u64;
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::signed(authorizer),
			who,
			1,
			1000,
		));

		// At block 5, authorization still valid.
		run_to_block(5, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 1000,
				transactions: 0,
				transactions_allowance: 1,
			},
		);
		// At block 6 (= valid_until), authorization expired — extent reads as zeros.
		run_to_block(6, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent::default(),
		);
	});
}

#[test]
fn valid_until_clamps_refresh_authorization_expiry() {
	// Refresh must respect `valid_until` the same way `authorize` does: a refresh
	// issued by an authorizer with `valid_until = 6` cannot extend the grant past
	// block 6 even if `now + AuthorizationPeriod` would land later.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let authorizer = 10u64;
		AllowedAuthorizers::<Test>::insert(
			authorizer,
			AuthorizerBudget { valid_until: Some(6), ..test_budget(100, 100_000) },
		);
		let who = 1u64;
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::signed(authorizer),
			who,
			1,
			1000,
		));

		// At block 5: refresh would naively set expiration = 5 + 10 = 15, but
		// must be clamped to authorizer's valid_until = 6.
		run_to_block(5, || None);
		assert_ok!(TransactionStorage::refresh_account_authorization(
			RuntimeOrigin::signed(authorizer),
			who,
		));
		// At block 6 the refreshed authorization is already expired.
		run_to_block(6, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent::default(),
		);
	});
}

#[test]
fn valid_until_beyond_default_period_does_not_clamp() {
	// `valid_until` past `now + AuthorizationPeriod` has no effect — the grant
	// gets the full default window.
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let authorizer = 11u64;
		// AuthorizationPeriod = 10, so default expiry from block 1 would be 11.
		// `valid_until = 100` is past that — no clamping.
		AllowedAuthorizers::<Test>::insert(
			authorizer,
			AuthorizerBudget { valid_until: Some(100), ..test_budget(100, 100_000) },
		);

		let who = 2u64;
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::signed(authorizer),
			who,
			1,
			1000,
		));

		// At block 10, still valid (default window covers 1..11).
		run_to_block(10, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent {
				bytes: 0,
				bytes_permanent: 0,
				bytes_allowance: 1000,
				transactions: 0,
				transactions_allowance: 1,
			},
		);
		// At block 11 (= default expiry), expired.
		run_to_block(11, || None);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who),
			AuthorizationExtent::default(),
		);
	});
}

#[test]
fn add_remove_authorizer_manages_system_providers() {
	// add → inc, re-add → no double-bump, remove → dec, re-remove → no underflow,
	// `remove_exhausted_authorizer` also dec's.
	new_test_ext().execute_with(|| {
		let who = 77u64;
		let providers_of = |a| frame_system::Account::<Test>::get(a).providers;
		assert_eq!(providers_of(who), 0);

		assert_ok!(TransactionStorage::add_authorizer(
			RuntimeOrigin::root(),
			who,
			test_budget(100, 1024),
		));
		assert_eq!(providers_of(who), 1);

		// Re-add must not double-bump.
		assert_ok!(TransactionStorage::add_authorizer(
			RuntimeOrigin::root(),
			who,
			test_budget(200, 2048),
		));
		assert_eq!(providers_of(who), 1);

		assert_ok!(TransactionStorage::remove_authorizer(RuntimeOrigin::root(), who));
		assert_eq!(providers_of(who), 0);

		// Re-remove must not underflow.
		assert_ok!(TransactionStorage::remove_authorizer(RuntimeOrigin::root(), who));
		assert_eq!(providers_of(who), 0);

		// `remove_exhausted_authorizer` path also dec's.
		AllowedAuthorizers::<Test>::insert(who, test_budget(0, 0));
		frame_system::Pallet::<Test>::inc_providers(&who);
		assert_ok!(TransactionStorage::remove_exhausted_authorizer(RuntimeOrigin::none(), who));
		assert_eq!(providers_of(who), 0);
	});
}

#[test]
fn feeless_if_reflects_authorizer_budget_feeless_flag() {
	// `true` only for `Signed(_)` origins whose `AllowedAuthorizers` entry has
	// `feeless = true`. Root / None / unregistered → not feeless via this flag.
	new_test_ext().execute_with(|| {
		let feeless = 7u64;
		let charged = 8u64;
		let unregistered = 9u64;
		AllowedAuthorizers::<Test>::insert(
			feeless,
			AuthorizerBudget { feeless: true, ..test_budget(10, 1000) },
		);
		AllowedAuthorizers::<Test>::insert(charged, test_budget(10, 1000));

		assert!(TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(feeless)));
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(charged)));
		// Not in the allow-list → not feeless.
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(unregistered)));
		// Root / None → not feeless (the dispatch is feeless by other means, not
		// via this flag).
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::root()));
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::none()));
	});
}

#[test]
fn feeless_if_ignored_when_authorizer_budget_inactive() {
	// An inactive budget (exhausted on either axis, or past `valid_until`)
	// disables the `feeless` flag — so the dispatcher pays for the call instead
	// of spamming free dispatches that would fail downstream.
	new_test_ext().execute_with(|| {
		let who = 21u64;
		let insert = |b| AllowedAuthorizers::<Test>::insert(who, b);

		// Exhausted on both axes.
		insert(AuthorizerBudget { feeless: true, ..test_budget(0, 0) });
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));

		// Exhausted on bytes axis only.
		insert(AuthorizerBudget { feeless: true, ..test_budget(0, 100) });
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));

		// Exhausted on transactions axis only.
		insert(AuthorizerBudget { feeless: true, ..test_budget(5, 0) });
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));

		// Sanity: budget with room on both axes is still feeless.
		insert(AuthorizerBudget { feeless: true, ..test_budget(5, 100) });
		assert!(TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));

		// `quota = None` (unlimited) is never exhausted.
		insert(AuthorizerBudget { quota: None, feeless: true, ..test_budget(0, 0) });
		assert!(TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));

		// Expired authorizer is inactive even with unlimited quota.
		// (`EnsureAllowedAuthorizers::try_origin` already rejects expired
		// authorizers, so this also exercises that rejection.)
		System::set_block_number(50);
		insert(AuthorizerBudget { quota: None, valid_until: Some(10), feeless: true });
		assert!(!TransactionStorage::is_feeless_authorizer(&RuntimeOrigin::signed(who)));
	});
}

#[test]
fn store_records_extrinsic_index_in_transaction_info() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), vec![7u8; 500]));
		run_to_block(2, || None);

		let txs = TransactionStorage::transactions_at(1).expect("block 1 has transactions");
		assert_eq!(txs.len(), 1);
		// The store call ran at extrinsic_index 0 in block 1 (it's the only call).
		assert_eq!(txs[0].extrinsic_index, 0);
		assert_eq!(txs[0].size, 500);
	});
}

/// Test to make sure we can actually access everything we need for build the
/// output times for the runtime API.
#[test]
fn transaction_info_projects_into_upstream_runtime_api_type() {
	use bulletin_transaction_storage_primitives::cids::HashingAlgorithm as PalletHashingAlgorithm;
	use codec::{Decode, Encode};
	use polkadot_sdk_frame::deps::sp_runtime::traits::{BlakeTwo256, Hash};

	type ContentHash = [u8; 32];
	type CidCodec = u64;
	const RAW_CID_CODEC: CidCodec = 0x55;

	#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo)]
	enum HashingAlgorithm {
		Blake2b256,
		Sha2_256,
		Keccak256,
	}

	#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo)]
	struct IndexedTransactionInfo {
		pub content_hash: ContentHash,
		pub size: u32,
		pub hashing: HashingAlgorithm,
		pub cid_codec: CidCodec,
		pub extrinsic_index: u32,
	}

	let tx = TransactionInfo {
		chunk_root: BlakeTwo256::hash(&[1]),
		content_hash: BlakeTwo256::hash(&[2]).into(),
		hashing: PalletHashingAlgorithm::Blake2b256,
		cid_codec: RAW_CID_CODEC,
		size: 500,
		extrinsic_index: 7,
		block_chunks: 4,
		kind: TransactionKind::Store,
	};

	let projected = IndexedTransactionInfo {
		content_hash: tx.content_hash,
		size: tx.size,
		hashing: match tx.hashing {
			PalletHashingAlgorithm::Blake2b256 => HashingAlgorithm::Blake2b256,
			PalletHashingAlgorithm::Sha2_256 => HashingAlgorithm::Sha2_256,
			PalletHashingAlgorithm::Keccak256 => HashingAlgorithm::Keccak256,
			_ => panic!("unknown bulletin HashingAlgorithm variant"),
		},
		cid_codec: tx.cid_codec,
		extrinsic_index: tx.extrinsic_index,
	};

	assert_eq!(projected.content_hash, tx.content_hash);
	assert_eq!(projected.size, 500);
	assert_eq!(projected.hashing, HashingAlgorithm::Blake2b256);
	assert_eq!(projected.cid_codec, RAW_CID_CODEC);
	assert_eq!(projected.extrinsic_index, 7);
}

// ============================================================================
// Auto-renew coverage gap-closing tests
// ============================================================================

/// `enable_auto_renew` rejects a second call for the same content hash, even from the
/// same account, with `AutoRenewalAlreadyEnabled`.
#[test]
fn enable_auto_renew_rejects_already_enabled() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		// Second call rejected at the extension (pool-level).
		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(crate::AUTO_RENEWAL_ALREADY_ENABLED.into()),
		);
	});
}

/// Expired authorization rejects through `check_authorization` with
/// `InvalidTransaction::Payment` — same path one-shot `renew` and `force_renew`
/// take when `expired()` is `now >= expiration`.
#[test]
fn enable_auto_renew_rejects_expired_authorization() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		// AuthorizationPeriod = 10; the auth granted at block 1 expires at block 11.
		init_block(11);

		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(InvalidTransaction::Payment.into()),
		);
	});
}

/// Insufficient byte capacity rejects at the extension with
/// `PERMANENT_ALLOWANCE_EXCEEDED` when `bytes_permanent + size > bytes_allowance`.
#[test]
fn enable_auto_renew_rejects_insufficient_capacity() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 1000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		let call = Call::enable_auto_renew { content_hash };
		assert_eq!(
			TransactionStorage::validate_signed(&who, &call).map(|_| ()),
			Err(PERMANENT_ALLOWANCE_EXCEEDED.into()),
		);
	});
}

/// `disable_auto_renew` dispatched in the same block as the renewal does NOT prevent the
/// renewal. `on_initialize` captures the entry into `PendingAutoRenewals` as a snapshot;
/// `do_process_auto_renewals` then iterates that vec without re-reading
/// `AutoRenewals[hash]`. So disabling between `on_initialize` and `apply_block_inherents`
/// in the renewal block is a no-op for the in-flight cycle — the caller still sees one
/// final `DataAutoRenewed` event. Subsequent cycles are correctly suppressed because
/// `AutoRenewals[hash]` is gone for the next on_initialize sweep.
///
/// This is only reachable once the registration has cleared its prepaid window
/// (`disable_auto_renew` rejects the owner while `paid: true`), so we exercise it
/// on cycle 2.
#[test]
fn disable_auto_renew_in_renewal_block_does_not_prevent_renewal() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Cycle 1: fire to consume the prepayment so the owner can `disable` later.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Cycle 2 block: on_initialize captures the block-12 renewal into pending.
		init_block(23);
		assert_eq!(PendingAutoRenewals::get().len(), 1);

		// Refresh authorization for cycle 2 (block-12 grant expired at block 22).
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));

		// Disable in the normal section — after on_initialize, before the inherent.
		assert_ok!(disable_auto_renew_via_extension(who, content_hash));
		assert!(AutoRenewals::get(content_hash).is_none(), "disable cleared the registration");

		// The mandatory inherent still iterates the captured pending vec and renews.
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));
		// AutoRenewals stays gone after the block — no further cycles.
		assert!(AutoRenewals::get(content_hash).is_none());
	});
}

/// Root `disable_auto_renew` executed in the same block as a prepaid cycle (between
/// `on_initialize` and the mandatory inherent) must not be silently undone by the
/// cycle's `paid: true → false` flip. The flip is implemented as a `mutate`, so a
/// Root disable that already removed the entry leaves nothing for the cycle to
/// flip — and no further cycles fire.
#[test]
fn root_disable_in_prepaid_renewal_block_is_not_undone_by_cycle() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		assert!(AutoRenewals::get(content_hash).unwrap().paid, "registration starts prepaid");

		// Cycle 1 block: on_initialize captures the prepaid entry into pending.
		init_block(12);
		assert_eq!(PendingAutoRenewals::get().len(), 1);

		// Root disables in the normal section — bypasses both the owner check and the
		// prepaid-window check.
		assert_ok!(TransactionStorage::disable_auto_renew(RuntimeOrigin::root(), content_hash));
		assert!(AutoRenewals::get(content_hash).is_none(), "Root disable cleared the entry");

		// Mandatory inherent: the captured pending renewal still fires (the prepayment
		// honestly delivers one cycle), but the post-cycle flip must not reinsert.
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));
		assert!(
			AutoRenewals::get(content_hash).is_none(),
			"Root's disable must survive the cycle — no silent re-arming via the paid-flip path",
		);
	});
}

/// `do_process_auto_renewals` emits `AutoRenewalFailed` when `check_authorization`
/// returns `CHAIN_PERMANENT_CAP_REACHED` — i.e. the chain-wide `PermanentStorageUsed`
/// counter would exceed `MaxPermanentStorageSize` — even if the per-account budget is
/// fine. The chain-wide gate only applies on cycles that actually charge, so this
/// scenario is reachable on cycle 2 (the first cycle is prepaid at registration).
#[test]
fn auto_renewal_fails_on_chain_wide_permanent_cap() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Per-account budget comfortably large — chain-wide cap is the only gate.
		// Pick a cap that fits the registration's prepayment but not a second cycle.
		MaxPermanentStorageSize::set(&3000);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			10,
			1_000_000,
		));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));
		assert_eq!(PermanentStorageUsed::get(), 2000);

		// Cycle 1 (prepaid) fires free at block 12 — chain-wide counter is not bumped.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			10,
			1_000_000,
		));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Cycle 2 at block 23 would charge another `size` chain-wide; overshoot the cap.
		init_block(23);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			who,
			10,
			1_000_000,
		));
		PermanentStorageUsed::put(2000);

		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash,
			account: who,
		}));
		assert!(AutoRenewals::get(content_hash).is_none());
		// Per-account counters untouched — failure happened before consume.
		assert_eq!(TransactionStorage::account_authorization_extent(who).bytes_permanent, 0,);
	});
}

/// `RetentionPeriod` change mid-cycle shifts the `obsolete = n - RP - 1` window. Raising
/// RP after auto-renew is enabled defers the renewal block — the OLD renewal point
/// becomes a normal block (no pending), and the renewal fires later at the NEW window
/// boundary.
#[test]
fn auto_renew_obeys_updated_retention_period() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Extend RetentionPeriod from 10 → 20.
		RetentionPeriod::put(20u64);

		// Block 12 was the OLD renewal boundary. With RP=20, `obsolete = 12 - 21 = 0`
		// (saturating). Transactions[1] must NOT be pulled.
		init_block(12);
		assert!(
			PendingAutoRenewals::get().is_empty(),
			"RP change should push the obsolete boundary out",
		);
		assert!(Transactions::get(1).is_some(), "Transactions[1] still present at block 12");

		// Block 22 is the NEW renewal boundary (`obsolete = 22 - 21 = 1`).
		init_block(22);
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 100_000));
		assert_eq!(PendingAutoRenewals::get().len(), 1);
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: who,
		}));
	});
}

/// `enable_auto_renew` is signed by the *registrant*, who may not be the storer.
/// The prepayment at registration (and every subsequent cycle's charge) consume
/// the registrant's authorization, not the storer's. Verifies: Bob registers
/// auto-renew on data Alice stored, and Bob's `bytes_permanent` is the one that
/// moves — both at registration time and on cycle 2.
#[test]
fn auto_renew_consumes_registrant_authorization_not_storer() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let alice = 1;
		let bob = 2;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), bob, 10, 100_000,));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);

		// Bob — not Alice — enables auto-renew. The prepayment lands on Bob.
		assert_ok!(enable_auto_renew_via_extension(bob, content_hash));
		assert_eq!(AutoRenewals::get(content_hash).unwrap().account, bob);
		assert_eq!(TransactionStorage::account_authorization_extent(alice).bytes_permanent, 0);
		assert_eq!(TransactionStorage::account_authorization_extent(bob).bytes_permanent, 2000);

		// Cycle 1 (prepaid) fires free at block 12 — re-authorize both so the
		// chain state is clean, but the cycle won't charge anyone.
		init_block(12);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), bob, 10, 100_000,));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Cycle 2 at block 23 charges per-cycle: again the registrant (Bob) pays.
		init_block(23);
		assert_ok!(TransactionStorage::authorize_account(
			RuntimeOrigin::root(),
			alice,
			10,
			100_000,
		));
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), bob, 10, 100_000,));
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));

		assert_eq!(TransactionStorage::account_authorization_extent(alice).bytes_permanent, 0);
		assert_eq!(TransactionStorage::account_authorization_extent(bob).bytes_permanent, 2000);
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::DataAutoRenewed {
			index: 0,
			content_hash,
			account: bob,
		}));
	});
}

/// `refresh_account_authorization` only extends `expiration`. If `bytes_permanent` is
/// already at or near the per-account cap, refreshing does NOT reset counters, so the
/// next auto-renew cycle still fails on the per-account axis (not on expiration).
///
/// To exercise this, we need to drive `bytes_permanent` close to the cap before
/// the cycle under test. `enable_auto_renew` pre-pays one cycle's worth at
/// registration — that registration charge already moves `bytes_permanent` to
/// `size`. The first cycle fires free (prepaid). The second cycle then has to
/// charge against an authorization whose `bytes_permanent` was preserved by
/// `refresh`, not reset.
#[test]
fn refresh_authorization_does_not_reset_counters_for_auto_renew() {
	new_test_ext().execute_with(|| {
		run_to_block(1, || None);
		let who = 1;
		let data = vec![0u8; 2000];
		let content_hash = blake2_256(&data);

		// Tight cap: one renewal fits, the second exceeds it.
		assert_ok!(TransactionStorage::authorize_account(RuntimeOrigin::root(), who, 10, 3000));
		assert_ok!(TransactionStorage::store(RuntimeOrigin::none(), data));
		run_to_block(2, || None);
		assert_ok!(enable_auto_renew_via_extension(who, content_hash));

		// Registration prepaid the first cycle: `bytes_permanent → 2000`.
		let after_register = TransactionStorage::account_authorization_extent(who);
		assert_eq!(after_register.bytes_permanent, 2000);

		// Refresh at block 11 (the original auth's expiration boundary), extending
		// expiration to 21 so block 12's free cycle and the subsequent paid cycle
		// at block 23 both see an unexpired auth — but counters must NOT reset.
		init_block(11);
		assert_ok!(TransactionStorage::refresh_account_authorization(RuntimeOrigin::root(), who,));
		let after_refresh = TransactionStorage::account_authorization_extent(who);
		assert_eq!(after_refresh, after_register, "refresh must not touch the extent");

		// Cycle 1 (prepaid) fires free at block 12; carry the renew into
		// `Transactions[12]` so it can age out for cycle 2.
		init_block(12);
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		assert!(!AutoRenewals::get(content_hash).unwrap().paid);
		assert_eq!(
			TransactionStorage::account_authorization_extent(who).bytes_permanent,
			2000,
			"prepaid cycle does not charge again",
		);
		let block_txs = BlockTransactions::take();
		if !block_txs.is_empty() {
			Transactions::insert(12u64, &block_txs);
		}

		// Refresh again at block 21 to keep the auth alive past block 23.
		init_block(21);
		assert_ok!(TransactionStorage::refresh_account_authorization(RuntimeOrigin::root(), who,));
		assert_eq!(
			TransactionStorage::account_authorization_extent(who).bytes_permanent,
			2000,
			"second refresh must also leave the extent untouched",
		);

		// Cycle 2 at block 23 must charge another 2000 against a per-account cap
		// of 3000 already at 2000 → AutoRenewalFailed on the per-account axis (not
		// on expiration — refresh kept the auth alive).
		init_block(23);
		assert_ok!(TransactionStorage::apply_block_inherents(RuntimeOrigin::none(), None));
		System::assert_has_event(RuntimeEvent::TransactionStorage(Event::AutoRenewalFailed {
			content_hash,
			account: who,
		}));
		assert!(AutoRenewals::get(content_hash).is_none());
	});
}
