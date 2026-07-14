// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate alloc;

use crate::*;
use alloc::{
	collections::{BTreeMap, BTreeSet},
	vec::Vec,
};
use codec::Encode;
use core::cell::RefCell;
use frame_support::{
	derive_impl,
	dispatch::{DispatchErrorWithPostInfo, GetDispatchInfo},
	parameter_types,
	storage::with_transaction,
};
use frame_system::EnsureRoot;
use indiv_support::traits::{
	AppendOnlyMembers, BatchProofItem, Context, ContextualAlias, Identifier, MembershipProver,
	RevisedContextualAlias, RevisionIndex, RingExponent, RingIndex, RingMode, RingPosition,
	RingStatus,
};
use sp_runtime::{
	testing::UintAuthorityId,
	traits::{Applyable, Checkable},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
	BuildStorage, DispatchError, TransactionOutcome,
};
use verifiable::mock::Mock;

pub type Header = sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>;
pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;
pub type Extrinsic = sp_runtime::generic::UncheckedExtrinsic<
	u64,
	RuntimeCall,
	UintAuthorityId,
	PeopleLiteAuth<Test>,
>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		PeopleLite: crate,
		AliasTarget: alias_target,
		Balances: pallet_balances,
	}
);

#[frame_support::pallet(dev_mode)]
pub mod alias_target {
	use alloc::vec::Vec;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use indiv_support::traits::ContextualAlias;
	use sp_runtime::DispatchError;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + crate::Config {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Recorded { alias: ContextualAlias, payload: Vec<u8> },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::default())]
		pub fn record(origin: OriginFor<T>, payload: Vec<u8>) -> DispatchResult {
			let alias = match origin.into_caller().try_into() {
				Ok(crate::Origin::LiteAlias(rev_ca)) => rev_ca.ca,
				_ => return Err(DispatchError::BadOrigin.into()),
			};

			Self::deposit_event(Event::Recorded { alias, payload });
			Ok(())
		}
	}
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
}

impl alias_target::Config for Test {}

#[cfg(feature = "runtime-benchmarks")]
pub struct Helper;
#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<u64, UintAuthorityId> for Helper {
	fn sign_message(_message: &[u8]) -> (u64, UintAuthorityId) {
		(0, UintAuthorityId(0))
	}
}

impl crate::Config for Test {
	type WeightInfo = ();
	type AttestationAllowanceManager = EnsureRoot<Self::AccountId>;
	type MemberService = MockMemberService;
	type CollectionOwner = LiteCollectionOwnerConst;
	type LiteRingExponent = LiteRingExponentConst;
	type LiteOnboardingSize = LiteOnboardingSizeConst;
	type AttestationSignature = UintAuthorityId;
	type LiteConsumerRegistrar = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = Helper;
}

parameter_types! {
	pub const LiteCollectionOwnerConst: u32 = 42;
	pub const LiteRingExponentConst: RingExponent = RingExponent::R2e9;
	pub const LiteOnboardingSizeConst: u32 = 7;
}

thread_local! {
	static MOCK_COLLECTIONS: RefCell<BTreeSet<Identifier>> = const { RefCell::new(BTreeSet::new()) };
	static MOCK_COLLECTION_MEMBERS: RefCell<BTreeMap<Identifier, Vec<<Mock as verifiable::GenerateVerifiable>::Member>>> = const { RefCell::new(BTreeMap::new()) };
	static MOCK_COLLECTION_REVISIONS: RefCell<BTreeMap<Identifier, RevisionIndex>> = const { RefCell::new(BTreeMap::new()) };
	static MOCK_FAIL_NEXT_ADD_MEMBERS: RefCell<bool> = const { RefCell::new(false) };
}

pub fn mock_member_service_members(
	identifier: &Identifier,
) -> Vec<<Mock as verifiable::GenerateVerifiable>::Member> {
	MOCK_COLLECTION_MEMBERS.with(|members_by_collection| {
		members_by_collection.borrow().get(identifier).cloned().unwrap_or_default()
	})
}

pub fn mock_member_service_revision(identifier: &Identifier) -> RevisionIndex {
	MOCK_COLLECTION_REVISIONS
		.with(|revisions| revisions.borrow().get(identifier).copied().unwrap_or(0))
}

pub fn mock_member_service_delete_collection(identifier: &Identifier) {
	MOCK_COLLECTIONS.with(|collections| {
		collections.borrow_mut().remove(identifier);
	});
	MOCK_COLLECTION_REVISIONS.with(|revisions| {
		revisions.borrow_mut().remove(identifier);
	});
	MOCK_COLLECTION_MEMBERS.with(|members_by_collection| {
		members_by_collection.borrow_mut().remove(identifier);
	});
}

pub fn mock_member_service_fail_next_add_members() {
	MOCK_FAIL_NEXT_ADD_MEMBERS.with(|flag| {
		*flag.borrow_mut() = true;
	});
}

fn reset_mock_member_service_state() {
	MOCK_COLLECTIONS.with(|collections| collections.borrow_mut().clear());
	MOCK_COLLECTION_MEMBERS
		.with(|members_by_collection| members_by_collection.borrow_mut().clear());
	MOCK_COLLECTION_REVISIONS.with(|revisions| revisions.borrow_mut().clear());
	MOCK_FAIL_NEXT_ADD_MEMBERS.with(|flag| *flag.borrow_mut() = false);
}

pub struct MockMemberService;

impl MembershipProver for MockMemberService {
	type Crypto = Mock;

	fn verify_membership(
		identifier: &Identifier,
		proof: &<Self::Crypto as verifiable::GenerateVerifiable>::Proof,
		_ring_index: RingIndex,
		context: Context,
		msg: &[u8],
	) -> Result<RevisedContextualAlias, DispatchError> {
		let members = mock_member_service_members(identifier);
		if proof == &verifiable::mock::MockProof::default() && !members.is_empty() {
			return Ok(RevisedContextualAlias {
				revision: mock_member_service_revision(identifier),
				ring: 0,
				ca: ContextualAlias { alias: [0u8; 32], context },
			});
		}
		let members =
			members.try_into().map_err(|_| DispatchError::Other("mock members overflow"))?;
		let alias = Mock::validate((), proof, &members, &context[..], msg)
			.map_err(|_| DispatchError::Other("mock invalid proof"))?;
		Ok(RevisedContextualAlias {
			revision: mock_member_service_revision(identifier),
			ring: 0,
			ca: ContextualAlias { alias, context },
		})
	}

	fn verify_membership_at_rev(
		identifier: &Identifier,
		proof: &<Self::Crypto as verifiable::GenerateVerifiable>::Proof,
		ring_index: RingIndex,
		_revision: RevisionIndex,
		context: Context,
		msg: &[u8],
	) -> Result<ContextualAlias, DispatchError> {
		Self::verify_membership(identifier, proof, ring_index, context, msg).map(|alias| alias.ca)
	}

	fn verify_memberships_in_ring(
		identifier: &Identifier,
		_ring_index: RingIndex,
		items: &[BatchProofItem<<Self::Crypto as verifiable::GenerateVerifiable>::Proof>],
	) -> Result<Vec<RevisedContextualAlias>, DispatchError> {
		let members = mock_member_service_members(identifier);
		let members =
			members.try_into().map_err(|_| DispatchError::Other("mock members overflow"))?;
		let revision = mock_member_service_revision(identifier);
		items
			.iter()
			.map(|item| {
				let context: Context = item
					.context
					.as_slice()
					.try_into()
					.map_err(|_| DispatchError::Other("mock invalid context"))?;
				let alias = Mock::validate((), &item.proof, &members, &item.context, &item.message)
					.map_err(|_| DispatchError::Other("mock invalid proof"))?;
				Ok(RevisedContextualAlias {
					revision,
					ring: 0,
					ca: ContextualAlias { alias, context },
				})
			})
			.collect()
	}

	fn verify_memberships_in_ring_at_rev(
		identifier: &Identifier,
		ring_index: RingIndex,
		_revision: RevisionIndex,
		items: &[BatchProofItem<<Self::Crypto as verifiable::GenerateVerifiable>::Proof>],
	) -> Result<Vec<ContextualAlias>, DispatchError> {
		Self::verify_memberships_in_ring(identifier, ring_index, items)
			.map(|v| v.into_iter().map(|rca| rca.ca).collect())
	}

	fn ring_revision(identifier: &Identifier, _ring_index: RingIndex) -> Option<RevisionIndex> {
		MOCK_COLLECTION_REVISIONS.with(|revisions| revisions.borrow().get(identifier).copied())
	}

	fn is_revision_valid(
		identifier: &Identifier,
		_ring_index: RingIndex,
		revision: RevisionIndex,
	) -> bool {
		mock_member_service_revision(identifier) == revision
	}

	fn revision_source_time(
		_identifier: &Identifier,
		_ring_index: RingIndex,
		_revision: RevisionIndex,
	) -> Option<u64> {
		None
	}
}

impl AppendOnlyMembers for MockMemberService {
	type Location = u32;

	fn create_collection(
		_owner: Self::Location,
		identifier: &Identifier,
		_onboarding_size: u32,
		_mode: RingMode,
		_ring_size: RingExponent,
		_self_inclusion_delay: Option<u64>,
	) -> frame_support::dispatch::DispatchResult {
		let inserted =
			MOCK_COLLECTIONS.with(|collections| collections.borrow_mut().insert(*identifier));
		if !inserted {
			return Err(DispatchError::Other("mock collection already exists"));
		}

		MOCK_COLLECTION_REVISIONS.with(|revisions| {
			revisions.borrow_mut().insert(*identifier, 0);
		});
		Ok(())
	}

	fn delete_collection(
		_owner: Self::Location,
		identifier: &Identifier,
	) -> frame_support::dispatch::DispatchResult {
		mock_member_service_delete_collection(identifier);
		Ok(())
	}

	fn active_count(_identifier: &Identifier) -> u32 {
		0
	}

	fn add_members(
		identifier: &Identifier,
		members: Vec<<Self::Crypto as verifiable::GenerateVerifiable>::Member>,
	) -> frame_support::dispatch::DispatchResult {
		let exists = MOCK_COLLECTIONS.with(|collections| collections.borrow().contains(identifier));
		if !exists {
			return Err(DispatchError::Other("mock collection not found"));
		}
		if MOCK_FAIL_NEXT_ADD_MEMBERS.with(|flag| flag.replace(false)) {
			return Err(DispatchError::Other("mock add_members failed"));
		}

		MOCK_COLLECTION_MEMBERS.with(|members_by_collection| {
			members_by_collection
				.borrow_mut()
				.entry(*identifier)
				.or_default()
				.extend(members);
		});
		MOCK_COLLECTION_REVISIONS.with(|revisions| {
			let next_revision =
				revisions.borrow().get(identifier).copied().unwrap_or(0).saturating_add(1);
			revisions.borrow_mut().insert(*identifier, next_revision);
		});
		Ok(())
	}

	fn remove_ring(
		_identifier: &Identifier,
		_ring_index: RingIndex,
	) -> frame_support::dispatch::DispatchResult {
		Ok(())
	}

	fn ring_status(_identifier: &Identifier, _ring_index: RingIndex) -> Option<RingStatus> {
		None
	}

	fn member_status(
		identifier: &Identifier,
		member: &<Self::Crypto as verifiable::GenerateVerifiable>::Member,
	) -> Option<RingPosition> {
		let position = MOCK_COLLECTION_MEMBERS.with(|members_by_collection| {
			members_by_collection
				.borrow()
				.get(identifier)
				.and_then(|members| members.iter().position(|existing| existing == member))
		})?;

		Some(RingPosition::Included { ring_index: 0, ring_page: 0, ring_position: position as u32 })
	}

	fn ring_members(
		identifier: &Identifier,
		_ring_index: RingIndex,
	) -> Vec<<Self::Crypto as verifiable::GenerateVerifiable>::Member> {
		mock_member_service_members(identifier)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn set_active_count(_identifier: &Identifier, _count: u32) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn initialize_chunks(_ring_size: RingExponent) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn onboard_all_and_build_ring(
		_identifier: &Identifier,
		_ring_index: RingIndex,
	) -> frame_support::dispatch::DispatchResult {
		Ok(())
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	reset_mock_member_service_state();
	let c = RuntimeGenesisConfig::default().build_storage().unwrap();
	sp_io::TestExternalities::from(c)
}

/// We gather both error into a single type in order to do `assert_ok` and `assert_err` safely.
/// Otherwise, we can easily miss the inner error in a `Resut<Resut<_, _>, _>`.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TransactionExecutionError {
	Validity(TransactionValidityError),
	// This ignores the post info.
	Dispatch(DispatchErrorWithPostInfo),
}

impl TransactionExecutionError {
	#[allow(unused)]
	pub fn unwrap_dispatch(self) -> DispatchErrorWithPostInfo {
		let Self::Dispatch(error) = self else {
			panic!("validity error unwrapped as dispatch");
		};
		error
	}
}

impl From<DispatchErrorWithPostInfo> for TransactionExecutionError {
	fn from(e: DispatchErrorWithPostInfo) -> Self {
		TransactionExecutionError::Dispatch(e)
	}
}

impl From<TransactionValidityError> for TransactionExecutionError {
	fn from(e: TransactionValidityError) -> Self {
		TransactionExecutionError::Validity(e)
	}
}

impl From<DispatchError> for TransactionExecutionError {
	fn from(e: DispatchError) -> Self {
		TransactionExecutionError::Dispatch(e.into())
	}
}

impl From<InvalidTransaction> for TransactionExecutionError {
	fn from(e: InvalidTransaction) -> Self {
		TransactionExecutionError::Validity(e.into())
	}
}

/// Execute a bare extrinsic with the given call.
pub fn exec_tx(x: Extrinsic) -> Result<(), TransactionExecutionError> {
	let info = x.get_dispatch_info();
	let len = x.encoded_size();

	let checked = Checkable::check(x, &frame_system::ChainContext::<Test>::default())?;

	// validation is always rollbacked in production.
	with_transaction(|| {
		let valid = checked.validate::<Test>(TransactionSource::External, &info, len);

		TransactionOutcome::Rollback(Result::<_, DispatchError>::Ok(valid))
	})
	.unwrap()?;

	checked.apply::<Test>(&info, len)??;

	Ok(())
}

/// Execute a signed extrinsic with the given call.
#[allow(unused)]
pub fn exec_signed_tx(
	account: u64,
	call: impl Into<RuntimeCall>,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call.into(),
		account,
		UintAuthorityId(account),
		PeopleLiteAuth::<Test>::new(None),
	);

	exec_tx(x)
}

#[allow(unused)]
pub fn exec_as_lite_person_tx(
	signer: u64,
	call: RuntimeCall,
	nonce: u32,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call,
		signer,
		UintAuthorityId(signer),
		PeopleLiteAuth::<Test>::new(Some(crate::PeopleLiteAuthData::AsLitePerson(nonce))),
	);
	exec_tx(x)
}

pub fn exec_as_lite_alias_with_account_tx(
	signer: u64,
	call: RuntimeCall,
	nonce: u32,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call,
		signer,
		UintAuthorityId(signer),
		PeopleLiteAuth::<Test>::new(Some(crate::PeopleLiteAuthData::AsLiteAliasWithAccount(nonce))),
	);
	exec_tx(x)
}

pub fn exec_as_lite_alias_with_proof_tx(
	call: RuntimeCall,
	proof: crate::ProofOf<Test>,
	ring_index: RingIndex,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_transaction(
		call,
		PeopleLiteAuth::<Test>::new(Some(crate::PeopleLiteAuthData::AsLiteAliasWithProof(
			proof,
			ring_index,
			*crate::LITE_PEOPLE_AUTH_CONTEXT,
		))),
	);
	exec_tx(x)
}

pub fn exec_as_lite_alias_with_account_revised_tx(
	signer: u64,
	call: RuntimeCall,
	nonce: u32,
	proof: crate::ProofOf<Test>,
	ring_index: RingIndex,
) -> Result<(), TransactionExecutionError> {
	let x = Extrinsic::new_signed(
		call,
		signer,
		UintAuthorityId(signer),
		PeopleLiteAuth::<Test>::new(Some(
			crate::PeopleLiteAuthData::AsLiteAliasWithAccountRevised(
				nonce,
				proof,
				ring_index,
				*crate::LITE_PEOPLE_AUTH_CONTEXT,
			),
		)),
	);
	exec_tx(x)
}
