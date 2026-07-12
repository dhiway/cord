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

//! Mock runtime for Members Notifier pallet tests

extern crate alloc;

use crate::{self as pallet_members_notifier, pallet::Subscribers, Event};
use alloc::{sync::Arc, vec::Vec};
use codec::{Decode, Encode};
use core::cell::RefCell;
use cumulus_primitives_core::ParaId;
use frame_support::{assert_ok, derive_impl, parameter_types, traits::EnsureOrigin, BoundedVec};
use frame_system::{
	offchain::{CreateAuthorizedTransaction, CreateTransaction, CreateTransactionBase},
	AuthorizeCall,
};
use indiv_support::traits::{Identifier, RingExponent};
use sp_runtime::{
	offchain::{
		testing::{PoolState, TestOffchainExt, TestTransactionPoolExt},
		OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
	},
	testing::UintAuthorityId,
	BuildStorage,
};
use verifiable::mock::Mock;
use xcm::latest::SendXcm;

// ============================================================================
// Runtime setup
// ============================================================================

pub type Header = sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>;
pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;
pub type TxExtension = (AuthorizeCall<Test>,);
pub type Extrinsic =
	sp_runtime::generic::UncheckedExtrinsic<u64, RuntimeCall, UintAuthorityId, TxExtension>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		MembersNotifier: pallet_members_notifier,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = ();
}

impl<C> CreateTransactionBase<C> for Test
where
	RuntimeCall: From<C>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl<C> CreateTransaction<C> for Test
where
	RuntimeCall: From<C>,
{
	type Extension = TxExtension;
	fn create_transaction(
		call: <Self as CreateTransactionBase<C>>::RuntimeCall,
		extension: Self::Extension,
	) -> Self::Extrinsic {
		Extrinsic::new_transaction(call, extension)
	}
}

impl<C> CreateAuthorizedTransaction<C> for Test
where
	RuntimeCall: From<C>,
{
	fn create_extension() -> Self::Extension {
		(AuthorizeCall::new(),)
	}
}

// ============================================================================
// Mock implementations
// ============================================================================

pub struct MockXcmRouter;
impl SendXcm for MockXcmRouter {
	type Ticket = (xcm::latest::Location, Vec<u8>);

	fn validate(
		destination: &mut Option<xcm::latest::Location>,
		message: &mut Option<xcm::latest::Xcm<()>>,
	) -> xcm::latest::SendResult<Self::Ticket> {
		let dest = destination.take().unwrap_or(xcm::latest::Location::here());
		let msg = message.take().map(|m| m.encode()).unwrap_or_default();
		Ok(((dest, msg), xcm::latest::Assets::new()))
	}

	fn deliver(ticket: Self::Ticket) -> Result<xcm::latest::XcmHash, xcm::latest::SendError> {
		if XCM_SEND_SHOULD_FAIL.with(|f| f.get()) {
			return Err(xcm::latest::SendError::Transport("mock failure"));
		}
		XCM_SEND_COUNT.with(|c| c.set(c.get() + 1));
		SENT_XCMS.with(|x| x.borrow_mut().push(ticket));
		Ok([0u8; 32])
	}
}

/// Pull the encoded call bytes out of a captured XCM message (the `Transact` instruction).
pub fn extract_transact_call(encoded_xcm: &[u8]) -> Option<Vec<u8>> {
	use codec::Decode;
	use xcm::latest::{Instruction, Xcm};
	let xcm: Xcm<()> = Xcm::decode(&mut &encoded_xcm[..]).ok()?;
	for inst in xcm.0.into_iter() {
		if let Instruction::Transact { call, .. } = inst {
			return Some(call.into_encoded());
		}
	}
	None
}

/// Decode the `SubscriberCall` from a captured XCM payload (stripping the pallet index byte).
pub fn decode_subscriber_call(encoded_xcm: &[u8]) -> Option<crate::pallet::SubscriberCall<Test>> {
	use codec::Decode;
	let call = extract_transact_call(encoded_xcm)?;
	// First byte is the pallet index.
	let mut body = &call[1..];
	crate::pallet::SubscriberCall::<Test>::decode(&mut body).ok()
}

pub fn clear_sent_xcms() {
	SENT_XCMS.with(|x| x.borrow_mut().clear());
}

pub fn get_sent_xcms() -> Vec<(xcm::latest::Location, Vec<u8>)> {
	SENT_XCMS.with(|x| x.borrow().clone())
}

pub struct MockClock;
impl frame_support::traits::UnixTime for MockClock {
	fn now() -> core::time::Duration {
		core::time::Duration::from_secs(MOCK_CLOCK_TIME.get())
	}
}

pub const GOVERNANCE_ACCOUNT: u64 = 99;

pub struct MockEnsureSubscriberOrigin;
impl EnsureOrigin<RuntimeOrigin> for MockEnsureSubscriberOrigin {
	type Success = ParaId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		match o.clone().into() {
			Ok(frame_system::RawOrigin::Root) => Ok(ParaId::from(1000u32)),
			_ => Err(o),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(RuntimeOrigin::root())
	}
}

pub struct MockManageOrigin;
impl EnsureOrigin<RuntimeOrigin> for MockManageOrigin {
	type Success = ();

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		match o.clone().into() {
			Ok(frame_system::RawOrigin::Root) => Ok(()),
			Ok(frame_system::RawOrigin::Signed(who)) if who == GOVERNANCE_ACCOUNT => Ok(()),
			_ => Err(o),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(RuntimeOrigin::signed(GOVERNANCE_ACCOUNT))
	}
}

std::thread_local! {
	pub static MOCK_RING_ROOTS_COUNT: RefCell<u32> = const { RefCell::new(0) };
	pub static MOCK_NEXT_RING_INDEX: RefCell<u32> = const { RefCell::new(0) };
	pub static MOCK_UPDATE_TRIGGER_BLOCKS: RefCell<u64> = const { RefCell::new(0) };
	pub static MOCK_UPDATE_TRIGGER_THRESHOLD: RefCell<u32> = const { RefCell::new(1) };
	pub static XCM_SEND_SHOULD_FAIL: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
	pub static XCM_SEND_COUNT: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
	pub static SENT_XCMS: RefCell<Vec<(xcm::latest::Location, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
	pub static MOCK_MAX_MESSAGE_SIZE: RefCell<u32> = const { RefCell::new(100_000) };
	pub static MOCK_CHANNELLESS_PARAS: RefCell<alloc::collections::BTreeSet<u32>> =
		const { RefCell::new(alloc::collections::BTreeSet::new()) };
	pub static MOCK_CLOCK_TIME: core::cell::Cell<u64> = const { core::cell::Cell::new(1000) };
	static TRANSACTION_POOL: RefCell<Arc<parking_lot::RwLock<PoolState>>> =
		RefCell::new(Arc::new(parking_lot::RwLock::new(PoolState {
			transactions: Vec::new(),
		})));
}

pub fn set_mock_ring_roots_count(count: u32) {
	MOCK_RING_ROOTS_COUNT.with(|c| *c.borrow_mut() = count);
	MOCK_NEXT_RING_INDEX.with(|c| *c.borrow_mut() = count);
}

pub fn set_throttle_config(blocks: u64, threshold: u32) {
	MOCK_UPDATE_TRIGGER_BLOCKS.with(|b| *b.borrow_mut() = blocks);
	MOCK_UPDATE_TRIGGER_THRESHOLD.with(|t| *t.borrow_mut() = threshold);
}

pub fn set_mock_max_message_size(size: u32) {
	MOCK_MAX_MESSAGE_SIZE.with(|s| *s.borrow_mut() = size);
}

pub fn set_mock_clock_time(secs: u64) {
	MOCK_CLOCK_TIME.with(|t| t.set(secs));
}

pub struct DynamicUpdateTriggerBlocks;
impl frame_support::traits::Get<u64> for DynamicUpdateTriggerBlocks {
	fn get() -> u64 {
		MOCK_UPDATE_TRIGGER_BLOCKS.with(|b| *b.borrow())
	}
}

pub struct DynamicUpdateTriggerThreshold;
impl frame_support::traits::Get<u32> for DynamicUpdateTriggerThreshold {
	fn get() -> u32 {
		MOCK_UPDATE_TRIGGER_THRESHOLD.with(|t| *t.borrow())
	}
}

pub struct MockRingRootsProvider;
impl indiv_support::traits::RingRootsProvider<<Mock as verifiable::GenerateVerifiable>::Members>
	for MockRingRootsProvider
{
	fn get_ring_roots(
		_identifier: Identifier,
		indices: &[u32],
	) -> Vec<(u32, <Mock as verifiable::GenerateVerifiable>::Members, u32)> {
		indices.iter().map(|&idx| (idx, Default::default(), 1)).collect()
	}

	fn next_ring_index(_identifier: Identifier) -> u32 {
		MOCK_NEXT_RING_INDEX.with(|c| *c.borrow())
	}

	fn get_ring_roots_paginated(
		_identifier: Identifier,
		after_key: Option<u32>,
		limit: u32,
	) -> Vec<(u32, <Mock as verifiable::GenerateVerifiable>::Members, u32)> {
		let total = MOCK_RING_ROOTS_COUNT.with(|c| *c.borrow());
		let start = match after_key {
			Some(k) => k.saturating_add(1),
			None => 0,
		};
		if start >= total {
			return Vec::new();
		}
		let end = (start + limit).min(total);
		(start..end).map(|idx| (idx, Default::default(), 1)).collect()
	}
}

pub struct MockChannelInfo;
impl cumulus_primitives_core::GetChannelInfo for MockChannelInfo {
	fn get_channel_status(_id: ParaId) -> cumulus_primitives_core::ChannelStatus {
		cumulus_primitives_core::ChannelStatus::Ready(0, 0)
	}

	fn get_channel_info(id: ParaId) -> Option<cumulus_primitives_core::ChannelInfo> {
		let is_channelless = MOCK_CHANNELLESS_PARAS.with(|s| s.borrow().contains(&u32::from(id)));
		if is_channelless {
			return None;
		}
		let max_message_size = MOCK_MAX_MESSAGE_SIZE.with(|s| *s.borrow());
		Some(cumulus_primitives_core::ChannelInfo {
			max_capacity: 1000,
			max_total_size: 102_400 * 1000,
			max_message_size,
			msg_count: 0,
			total_size: 0,
		})
	}
}

// ============================================================================
// Configuration
// ============================================================================

parameter_types! {
	pub const MaxSubscribers: u32 = 10;
	pub const MaxUpdatesPerBatch: u32 = 10;
	pub const MaxCollectionsPerSubscriber: u32 = 5;
	pub const MaxCollections: u32 = 10;
	pub const RequestReplayRemoteWeight: frame_support::weights::Weight = frame_support::weights::Weight::zero();
	pub const OffchainWorkerInterval: u64 = 1;
	pub const StuckBatchTimeout: u64 = 100;
	pub const ReplayCooldownSeconds: u64 = 60;
}

impl pallet_members_notifier::Config for Test {
	type WeightInfo = ();
	type XcmRouter = MockXcmRouter;
	type ManageOrigin = MockManageOrigin;
	type Crypto = Mock;
	type Clock = MockClock;
	type MaxSubscribers = MaxSubscribers;
	type MaxUpdatesPerBatch = MaxUpdatesPerBatch;
	type MaxCollectionsPerSubscriber = MaxCollectionsPerSubscriber;
	type MaxCollections = MaxCollections;
	type RingRootsProvider = MockRingRootsProvider;
	type EnsureSubscriberOrigin = MockEnsureSubscriberOrigin;
	type ChannelInfo = MockChannelInfo;
	type UpdateTriggerBlocks = DynamicUpdateTriggerBlocks;
	type UpdateTriggerThreshold = DynamicUpdateTriggerThreshold;
	type RequestReplayRemoteWeight = RequestReplayRemoteWeight;
	type OffchainWorkerInterval = OffchainWorkerInterval;
	type StuckBatchTimeout = StuckBatchTimeout;
	type ReplayCooldownSeconds = ReplayCooldownSeconds;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for () {
	fn setup_ring_roots(count: u32) {
		set_mock_ring_roots_count(count);
		// Large enough channel for any number of updates in benchmarks.
		set_mock_max_message_size(u32::MAX);
	}

	fn set_max_message_size(size: u32) {
		set_mock_max_message_size(size);
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	MOCK_MAX_MESSAGE_SIZE.with(|s| *s.borrow_mut() = 100_000);
	MOCK_CHANNELLESS_PARAS.with(|s| s.borrow_mut().clear());
	MOCK_CLOCK_TIME.with(|t| t.set(1000));
	let storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext: sp_io::TestExternalities = storage.into();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, state) = TestTransactionPoolExt::new();
	TRANSACTION_POOL.set(state);
	ext.register_extension(OffchainDbExt::new(offchain.clone()));
	ext.register_extension(OffchainWorkerExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.execute_with(|| System::set_block_number(1));
	ext
}

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Test,
	Block,
	frame_system::ChainContext<Test>,
	Test,
	AllPalletsWithSystem,
	(),
>;

/// Advances the chain to `target_block`, running offchain worker and draining the tx pool each
/// block.
pub fn advance_to_block(target_block: u64) {
	use frame_support::traits::OffchainWorker;

	loop {
		let current = frame_system::Pallet::<Test>::block_number();
		if current >= target_block {
			break;
		}

		// Running offchain worker for the current block.
		AllPalletsWithSystem::offchain_worker(current);

		// Advancing the block.
		let next = current.saturating_add(1u64);
		frame_system::Pallet::<Test>::initialize(&next, &Default::default(), &Default::default());

		// Draining and applying transactions from the pool.
		let transactions = {
			TRANSACTION_POOL.with_borrow_mut(|pool| std::mem::take(&mut pool.write().transactions))
		};
		for tx in transactions {
			let tx = Extrinsic::decode(&mut &tx[..]).unwrap();
			Executive::apply_extrinsic(tx).expect("tx valid").expect("tx succeeds");
		}
	}
}

// ============================================================================
// Test helpers
// ============================================================================

/// Returns the raw transactions currently sitting in the test transaction pool,
/// without draining them.
pub fn pool_transactions() -> Vec<Vec<u8>> {
	TRANSACTION_POOL.with_borrow(|pool| pool.read().transactions.clone())
}

pub fn authorized_origin() -> RuntimeOrigin {
	RuntimeOrigin::from(frame_system::RawOrigin::<u64>::Authorized)
}

pub fn subscriber_exists(para_id: u32) -> bool {
	Subscribers::<Test>::contains_key(ParaId::from(para_id))
}

fn updates_sent_events() -> Vec<(ParaId, u32)> {
	System::events()
		.into_iter()
		.filter_map(|record| match record.event {
			RuntimeEvent::MembersNotifier(Event::UpdatesSent { para_id, update_count }) =>
				Some((para_id, update_count)),
			_ => None,
		})
		.collect()
}

// ============================================================================
// Story-like test types
// ============================================================================

/// Default pallet index used in tests.
pub const TEST_PALLET_INDEX: u8 = 50;

pub struct TestSubscriber {
	pub para_id: ParaId,
	pub pallet_index: u8,
}

impl TestSubscriber {
	pub fn new(para_id: u32) -> Self {
		Self { para_id: ParaId::from(para_id), pallet_index: TEST_PALLET_INDEX }
	}

	#[allow(dead_code)]
	pub fn with_pallet_index(mut self, pallet_index: u8) -> Self {
		self.pallet_index = pallet_index;
		self
	}

	pub fn subscribe_to(self, collections: &[Identifier]) -> Self {
		let paired: Vec<(Identifier, RingExponent)> =
			collections.iter().map(|id| (*id, RingExponent::R2e9)).collect();
		self.subscribe_to_with_exponents(&paired)
	}

	pub fn subscribe_to_with_exponents(self, collections: &[(Identifier, RingExponent)]) -> Self {
		let bounded: BoundedVec<(Identifier, RingExponent), MaxCollectionsPerSubscriber> =
			collections.to_vec().try_into().expect("test collections within bounds");
		assert_ok!(MembersNotifier::subscribe(
			RuntimeOrigin::signed(GOVERNANCE_ACCOUNT),
			self.para_id,
			bounded,
			self.pallet_index,
		));
		self
	}
}

/// Well-known test identifiers.
pub const PEOPLE_IDENTIFIER: Identifier = [0u8; 32];
pub const PEOPLE_LITE_IDENTIFIER: Identifier = [1u8; 32];

pub struct TestCollection(pub Identifier);

impl TestCollection {
	pub fn people() -> Self {
		Self(PEOPLE_IDENTIFIER)
	}

	pub fn people_lite() -> Self {
		Self(PEOPLE_LITE_IDENTIFIER)
	}

	pub fn add_pending_update(&self, ring_index: u32, _revision: u32) -> &Self {
		let page = crate::pallet::PageState::<Test>::get().write_page;
		let key = (page, self.0, ring_index);
		let is_new = !crate::pallet::PendingUpdates::<Test>::contains_key(key);
		crate::pallet::PendingUpdates::<Test>::insert(key, ());
		if is_new {
			crate::pallet::PageUpdatesCount::<Test>::mutate(page, |c| *c = c.saturating_add(1));
		}
		self
	}
}

// ============================================================================
// Event query builders
// ============================================================================

pub struct Events;

impl Events {
	pub fn updates_sent() -> UpdatesSentEvents {
		UpdatesSentEvents::new()
	}

	pub fn update_send_failed() -> UpdateSendFailedEvents {
		UpdateSendFailedEvents::new()
	}

	pub fn subscribed() -> SubscribedEvents {
		SubscribedEvents::new()
	}

	pub fn unsubscribed() -> UnsubscribedEvents {
		UnsubscribedEvents::new()
	}

	pub fn replay_requested() -> ReplayRequestedEvents {
		ReplayRequestedEvents::new()
	}

	pub fn batch_abandoned() -> BatchAbandonedEvents {
		BatchAbandonedEvents::new()
	}
}

pub struct UpdatesSentEvents(Vec<(ParaId, u32)>);

impl UpdatesSentEvents {
	pub fn new() -> Self {
		Self(updates_sent_events())
	}

	pub fn count(&self) -> usize {
		self.0.len()
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(self.0.len(), expected, "expected {} events, got {:?}", expected, self.0);
		self
	}

	pub fn assert_received(self, para_id: impl Into<ParaId>, expected_count: u32) -> Self {
		let para = para_id.into();
		let total: u32 = self.0.iter().filter(|(p, _)| *p == para).map(|(_, c)| c).sum();
		assert_eq!(
			total, expected_count,
			"expected {:?} to receive {} updates total, got {} from {:?}",
			para, expected_count, total, self.0
		);
		self
	}

	pub fn assert_any_received(self, para_id: u32) -> Self {
		let para = ParaId::from(para_id);
		assert!(
			self.0.iter().any(|(p, _)| *p == para),
			"expected para_id {} to receive updates, got {:?}",
			para_id,
			self.0
		);
		self
	}

	pub fn total_for(&self, para_id: u32) -> u32 {
		let para = ParaId::from(para_id);
		self.0.iter().filter(|(p, _)| *p == para).map(|(_, c)| c).sum()
	}
}

pub struct UpdateSendFailedEvents(Vec<ParaId>);

#[allow(dead_code)]
impl UpdateSendFailedEvents {
	pub fn new() -> Self {
		Self(
			System::events()
				.into_iter()
				.filter_map(|record| match record.event {
					RuntimeEvent::MembersNotifier(Event::UpdateSendFailed { para_id }) =>
						Some(para_id),
					_ => None,
				})
				.collect(),
		)
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(
			self.0.len(),
			expected,
			"expected {} UpdateSendFailed events, got {:?}",
			expected,
			self.0
		);
		self
	}

	pub fn assert_emitted_for(self, para_id: impl Into<ParaId>) -> Self {
		let para = para_id.into();
		assert!(
			self.0.contains(&para),
			"expected UpdateSendFailed for {:?}, got {:?}",
			para,
			self.0
		);
		self
	}
}

pub struct SubscribedEvents(Vec<ParaId>);

impl SubscribedEvents {
	pub fn new() -> Self {
		Self(
			System::events()
				.into_iter()
				.filter_map(|record| match record.event {
					RuntimeEvent::MembersNotifier(Event::Subscribed { para_id }) => Some(para_id),
					_ => None,
				})
				.collect(),
		)
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(
			self.0.len(),
			expected,
			"expected {} Subscribed events, got {:?}",
			expected,
			self.0
		);
		self
	}

	pub fn assert_emitted_for(self, para_id: impl Into<ParaId>) -> Self {
		let para = para_id.into();
		assert!(self.0.contains(&para), "expected Subscribed for {:?}, got {:?}", para, self.0);
		self
	}
}

pub struct UnsubscribedEvents(Vec<ParaId>);

impl UnsubscribedEvents {
	pub fn new() -> Self {
		Self(
			System::events()
				.into_iter()
				.filter_map(|record| match record.event {
					RuntimeEvent::MembersNotifier(Event::Unsubscribed { para_id }) => Some(para_id),
					_ => None,
				})
				.collect(),
		)
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(
			self.0.len(),
			expected,
			"expected {} Unsubscribed events, got {:?}",
			expected,
			self.0
		);
		self
	}

	pub fn assert_emitted_for(self, para_id: impl Into<ParaId>) -> Self {
		let para = para_id.into();
		assert!(self.0.contains(&para), "expected Unsubscribed for {:?}, got {:?}", para, self.0);
		self
	}
}

pub struct ReplayRequestedEvents(Vec<(ParaId, Identifier, u32)>);

impl ReplayRequestedEvents {
	pub fn new() -> Self {
		Self(
			System::events()
				.into_iter()
				.filter_map(|record| match record.event {
					RuntimeEvent::MembersNotifier(Event::ReplayRequested {
						para_id,
						identifier,
						indices_count,
					}) => Some((para_id, identifier, indices_count)),
					_ => None,
				})
				.collect(),
		)
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(
			self.0.len(),
			expected,
			"expected {} ReplayRequested events, got {:?}",
			expected,
			self.0
		);
		self
	}
}

pub struct BatchAbandonedEvents(Vec<u64>);

impl BatchAbandonedEvents {
	pub fn new() -> Self {
		Self(
			System::events()
				.into_iter()
				.filter_map(|record| match record.event {
					RuntimeEvent::MembersNotifier(Event::BatchAbandoned { sequence }) =>
						Some(sequence),
					_ => None,
				})
				.collect(),
		)
	}

	pub fn assert_count(self, expected: usize) -> Self {
		assert_eq!(
			self.0.len(),
			expected,
			"expected {} BatchAbandoned events, got {:?}",
			expected,
			self.0
		);
		self
	}
}
