// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use super::{Event as CollectiveEvent, *};
use crate as pallet_collective;
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::{
	assert_noop, assert_ok, construct_runtime, derive_impl, parameter_types,
	traits::{ConstU32, ConstU64, OnFinalize, OnInitialize},
	Hashable,
};
use frame_system::{pallet_prelude::BlockNumberFor, EnsureRoot, EventRecord, Phase};
use rand::{seq::SliceRandom, thread_rng};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

pub(crate) type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test
	{
		System: frame_system::{Pallet, Call, Event<T>},
		Collective: pallet_collective::<Instance1>::{Pallet, Call, Event<T>, Origin<T>, Config<T>},
		CollectiveMajority: pallet_collective::<Instance2>::{Pallet, Call, Event<T>, Origin<T>, Config<T>},
		DefaultCollective: pallet_collective::{Pallet, Call, Event<T>, Origin<T>, Config<T>},
		NetworkMembership: pallet_network_membership::{Pallet, Call, Storage, Event<T>, Config<T>},
		Democracy: mock_democracy::{Pallet, Call, Event<T>},
	}
);

mod mock_democracy {
	pub use pallet::*;
	#[frame_support::pallet(dev_mode)]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::pallet_prelude::*;

		#[pallet::pallet]
		pub struct Pallet<T>(_);

		#[pallet::config]
		pub trait Config: frame_system::Config + Sized {
			type RuntimeEvent: From<Event<Self>>
				+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
			type ExternalMajorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		}

		#[pallet::call]
		impl<T: Config> Pallet<T> {
			pub fn external_propose_majority(origin: OriginFor<T>) -> DispatchResult {
				T::ExternalMajorityOrigin::ensure_origin(origin)?;
				Self::deposit_event(Event::<T>::ExternalProposed);
				Ok(())
			}
		}

		#[pallet::event]
		#[pallet::generate_deposit(pub(super) fn deposit_event)]
		pub enum Event<T: Config> {
			ExternalProposed,
		}
	}
}

pub type MaxMembers = ConstU32<100>;
type AccountId = u64;

parameter_types! {
	pub const MotionDuration: u64 = 3;
	pub const MaxProposals: u32 = 257;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::MAX);
	pub static MaxProposalWeight: Weight = default_max_proposal_weight();
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type RuntimeCall = RuntimeCall;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}
impl Config<Instance1> for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = ConstU64<3>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = PrimeDefaultVote;
	type WeightInfo = ();
	type NetworkMembershipOrigin = EnsureRoot<Self::AccountId>;
	type MaxProposalWeight = MaxProposalWeight;
}
impl Config<Instance2> for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = ConstU64<3>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityThenPrimeDefaultVote;
	type WeightInfo = ();
	type NetworkMembershipOrigin = EnsureRoot<Self::AccountId>;
	type MaxProposalWeight = MaxProposalWeight;
}
impl mock_democracy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ExternalMajorityOrigin = EnsureProportionAtLeast<u64, Instance1, 3, 4>;
}
impl Config for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = ConstU64<3>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = PrimeDefaultVote;
	type WeightInfo = ();
	type NetworkMembershipOrigin = EnsureRoot<Self::AccountId>;
	type MaxProposalWeight = MaxProposalWeight;
}

parameter_types! {
	pub const MembershipPeriod: BlockNumberFor<Test> = 5;
	pub const MaxMembersPerBlock: u32 = 5;
}

impl pallet_network_membership::Config for Test {
	type NetworkMembershipOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type MembershipPeriod = MembershipPeriod;
	type MaxMembersPerBlock = MaxMembersPerBlock;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	collective_members: Vec<AccountId>,
	network_members: Vec<AccountId>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { collective_members: (1..=100).collect(), network_members: (1..=120).collect() }
	}
}

impl ExtBuilder {
	fn set_collective_members(mut self, collective_members: Vec<AccountId>) -> Self {
		self.collective_members = collective_members;
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut old_members = Vec::new();
		const SEED: u32 = 0;
		let caller: AccountId = whitelisted_caller();
		for i in 0..100 {
			let old_member = account::<AccountId>("member", i, SEED);
			old_members.push(old_member);
		}
		old_members.push(caller);

		let mut network_membership = self.network_members.clone();
		network_membership.extend(old_members);

		let mut ext: sp_io::TestExternalities = RuntimeGenesisConfig {
			collective: pallet_collective::GenesisConfig {
				members: self.collective_members,
				phantom: Default::default(),
			},
			collective_majority: pallet_collective::GenesisConfig {
				members: (1..100).collect(),
				phantom: Default::default(),
			},
			network_membership: pallet_network_membership::GenesisConfig {
				members: network_membership.into_iter().map(|member| (member, false)).collect(),
			},
			default_collective: Default::default(),
		}
		.build_storage()
		.unwrap()
		.into();

		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	pub fn build_and_execute(self, test: impl FnOnce()) {
		self.build().execute_with(|| {
			test();
			Collective::do_try_state().unwrap();
		})
	}
}

fn make_proposal(value: u64) -> RuntimeCall {
	RuntimeCall::System(frame_system::Call::remark_with_event {
		remark: value.to_be_bytes().to_vec(),
	})
}

fn record(event: RuntimeEvent) -> EventRecord<RuntimeEvent, H256> {
	EventRecord { phase: Phase::Initialization, event, topics: vec![] }
}

fn default_max_proposal_weight() -> Weight {
	sp_runtime::Perbill::from_percent(80) * BlockWeights::get().max_block
}
pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		Collective::on_finalize(System::block_number());
		System::on_finalize(System::block_number());
		System::reset_events();
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		Collective::on_initialize(System::block_number());
	}
}

#[test]
fn motions_basic_environment_works() {
	ExtBuilder::default().build_and_execute(|| {
		assert_eq!(Collective::members(), (1..=100).collect::<Vec<_>>());
		assert_eq!(*Collective::proposals(), Vec::<H256>::new());
	});
}

#[test]
fn initialize_members_sorts_members() {
	let mut rng = thread_rng();
	let mut unsorted_members: Vec<u64> = (1..=100).map(|x| x as u64).collect();
	unsorted_members.shuffle(&mut rng); // Shuffle the vector

	let mut expected_members: Vec<u64> = unsorted_members.clone();
	expected_members.sort(); // Sort the vector

	ExtBuilder::default()
		.set_collective_members(unsorted_members)
		.build_and_execute(|| {
			assert_eq!(Collective::members(), expected_members);
		});
}

#[test]
fn close_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));

		System::set_block_number(3);
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), hash, 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::TooEarly
		);

		assert_noop!(
			Collective::close(RuntimeOrigin::signed(101), hash, 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::NotMember
		);

		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(1),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 3
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 2,
					no: 98
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					proposal_hash: hash
				}))
			]
		);
	});
}

#[test]
fn proposal_weight_limit_ignored_on_disapprove() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = RuntimeCall::Collective(crate::Call::set_members {
			new_members: vec![1, 2, 3],
			prime: None,
			old_count: MaxMembers::get(),
		});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		// No votes, this proposal wont pass
		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(3),
			hash,
			0,
			proposal_weight - Weight::from_parts(100, 0),
			proposal_len
		));
	})
}

#[test]
fn close_with_prime_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			vec![1, 2, 3],
			Some(3),
			MaxMembers::get()
		));

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));

		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(3),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 3
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 2,
					no: 1
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					proposal_hash: hash
				}))
			]
		);
	});
}

#[test]
fn close_with_voting_prime_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			vec![1, 2, 3],
			Some(1),
			MaxMembers::get()
		));

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));

		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(3),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 3
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved { proposal_hash: hash })),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				}))
			]
		);
	});
}

#[test]
fn close_with_no_prime_but_majority_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(CollectiveMajority::set_members(
			RuntimeOrigin::root(),
			vec![1, 2, 3, 4, 5],
			Some(5),
			MaxMembers::get()
		));

		assert_ok!(CollectiveMajority::propose(
			RuntimeOrigin::signed(1),
			5,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(2), hash, 0, true));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(3), hash, 0, true));

		System::set_block_number(4);
		assert_ok!(CollectiveMajority::close(
			RuntimeOrigin::signed(4),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 5
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					account: 3,
					proposal_hash: hash,
					voted: true,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 5,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Approved {
					proposal_hash: hash
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Executed {
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				}))
			]
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![1, 2], nays: vec![], end })
		);
		Collective::change_members_sorted(&[4], &[1], &[2, 3, 4]);
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(2),
			2,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), hash, 1, false));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![3], end })
		);
		Collective::change_members_sorted(&[], &[3], &[2, 4]);
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works_with_set_members() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![1, 2], nays: vec![], end })
		);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			vec![2, 3, 4],
			None,
			MaxMembers::get()
		));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(2),
			2,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), hash, 1, false));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![3], end })
		);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			vec![2, 4],
			None,
			MaxMembers::get()
		));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 1, threshold: 2, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn propose_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(*Collective::proposals(), vec![hash]);
		assert_eq!(Collective::proposal_of(hash), Some(proposal));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 3, ayes: vec![], nays: vec![], end })
		);

		assert_eq!(
			System::events(),
			vec![record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				account: 1,
				proposal_index: 0,
				proposal_hash: hash,
				threshold: 3
			}))]
		);
	});
}

#[test]
fn limit_active_proposals() {
	ExtBuilder::default().build_and_execute(|| {
		for i in 0..MaxProposals::get() {
			let proposal = make_proposal(i as u64);
			let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
			assert_ok!(Collective::propose(
				RuntimeOrigin::signed(1),
				3,
				Box::new(proposal.clone()),
				proposal_len
			));
		}
		let proposal = make_proposal(MaxProposals::get() as u64 + 1);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(RuntimeOrigin::signed(1), 3, Box::new(proposal), proposal_len),
			Error::<Test, Instance1>::TooManyProposals
		);
	})
}

#[test]
fn correct_validate_and_get_proposal() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = RuntimeCall::Collective(crate::Call::set_members {
			new_members: vec![1, 2, 3],
			prime: None,
			old_count: MaxMembers::get(),
		});
		let length = proposal.encode().len() as u32;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal.clone()),
			length
		));

		let hash = BlakeTwo256::hash_of(&proposal);
		let weight = proposal.get_dispatch_info().weight;
		assert_noop!(
			Collective::validate_and_get_proposal(
				&BlakeTwo256::hash_of(&vec![3; 4]),
				length,
				weight
			),
			Error::<Test, Instance1>::ProposalMissing
		);
		assert_noop!(
			Collective::validate_and_get_proposal(&hash, length - 2, weight),
			Error::<Test, Instance1>::WrongProposalLength
		);
		let res = Collective::validate_and_get_proposal(&hash, length, weight);
		assert_ok!(res.clone());
		let (retrieved_proposal, len) = res.unwrap();
		assert_eq!(length as usize, len);
		assert_eq!(proposal, retrieved_proposal);
	})
}

#[test]
fn motions_ignoring_non_collective_proposals_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(RuntimeOrigin::signed(150), 3, Box::new(proposal), proposal_len),
			Error::<Test, Instance1>::NotMember
		);
	});
}

#[test]
fn motions_ignoring_non_collective_votes_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(142), hash, 0, true),
			Error::<Test, Instance1>::NotMember,
		);
	});
}

#[test]
fn motions_ignoring_bad_index_collective_vote_works() {
	ExtBuilder::default().build_and_execute(|| {
		System::set_block_number(3);
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(2), hash, 1, true),
			Error::<Test, Instance1>::WrongIndex,
		);
	});
}

#[test]
fn motions_vote_after_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		// Initially there a no votes when the motion is proposed.
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![], nays: vec![], end })
		);
		// Cast first aye vote.
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![1], nays: vec![], end })
		);
		// Try to cast a duplicate aye vote.
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(1), hash, 0, true),
			Error::<Test, Instance1>::DuplicateVote,
		);
		// Cast a nay vote.
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, false));
		assert_eq!(
			Collective::voting(hash),
			Some(Votes { index: 0, threshold: 2, ayes: vec![], nays: vec![1], end })
		);
		// Try to cast a duplicate nay vote.
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(1), hash, 0, false),
			Error::<Test, Instance1>::DuplicateVote,
		);

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 1
				})),
			]
		);
	});
}

#[test]
fn motions_reproposing_disapproved_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		run_to_block(1);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, false));
		run_to_block(5);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			0,
			proposal_weight,
			proposal_len
		));
		assert_eq!(*Collective::proposals(), vec![]);
		run_to_block(7);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		assert_eq!(*Collective::proposals(), vec![hash]);
	});
}

#[test]
fn motions_approval_with_enough_votes_and_lower_voting_threshold_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = RuntimeCall::Democracy(mock_democracy::Call::external_propose_majority {});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		// The voting threshold is 2, but the required votes for
		// `ExternalMajorityOrigin` is 3. The proposal will be executed regardless of
		// the voting threshold as long as we have enough yes votes.
		//
		// Failed to execute with only 2 yes votes.
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			0,
			proposal_weight,
			proposal_len
		));
		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved { proposal_hash: hash })),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
			]
		);

		System::reset_events();

		// Executed with 3 yes votes.
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		for i in 1..=80 {
			assert_ok!(Collective::vote(RuntimeOrigin::signed(i), hash, 1, true));
		}
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			1,
			proposal_weight,
			proposal_len
		));

		let mut expected_events =
			vec![record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				account: 1,
				proposal_index: 1,
				proposal_hash: hash,
				threshold: 2,
			}))];

		for i in 1..=80 {
			expected_events.push(record(RuntimeEvent::Collective(CollectiveEvent::Voted {
				account: i,
				proposal_hash: hash,
				voted: true,
				yes: i as u32,
				no: 0,
			})));
		}

		expected_events.extend(vec![
			record(RuntimeEvent::Collective(CollectiveEvent::Closed {
				proposal_hash: hash,
				yes: 80,
				no: 0,
			})),
			record(RuntimeEvent::Collective(CollectiveEvent::Approved { proposal_hash: hash })),
			record(RuntimeEvent::Democracy(
				mock_democracy::pallet::Event::<Test>::ExternalProposed,
			)),
			record(RuntimeEvent::Collective(CollectiveEvent::Executed {
				proposal_hash: hash,
				result: Ok(()),
			})),
		]);
		assert_eq!(System::events(), expected_events);
	});
}

#[test]
fn motions_disapproval_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		run_to_block(1);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		run_to_block(4);
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, false));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: false,
					yes: 1,
					no: 1
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 1,
					no: 99
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					proposal_hash: hash
				})),
			]
		);
	});
}

#[test]
fn motions_approval_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved { proposal_hash: hash })),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
			]
		);
	});
}

#[test]
fn motion_with_no_votes_closes_with_disapproval() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			3,
			Box::new(proposal),
			proposal_len
		));
		assert_eq!(
			System::events()[0],
			record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				account: 1,
				proposal_index: 0,
				proposal_hash: hash,
				threshold: 3
			}))
		);

		// Closing the motion too early is not possible because it has neither
		// an approving or disapproving simple majority due to the lack of votes.
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), hash, 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::TooEarly
		);

		// Once the motion duration passes,
		let closing_block = System::block_number() + MotionDuration::get();
		System::set_block_number(closing_block);
		// we can successfully close the motion.
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		// Events show that the close ended in a disapproval.
		assert_eq!(
			System::events()[1],
			record(RuntimeEvent::Collective(CollectiveEvent::Closed {
				proposal_hash: hash,
				yes: 0,
				no: 100
			}))
		);
		assert_eq!(
			System::events()[2],
			record(RuntimeEvent::Collective(CollectiveEvent::Disapproved { proposal_hash: hash }))
		);
	})
}

#[test]
fn close_disapprove_does_not_care_about_weight_or_len() {
	// This test confirms that if you close a proposal that would be disapproved,
	// we do not care about the proposal length or proposal weight since it will
	// not be read from storage or executed.
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		run_to_block(1);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		// First we make the proposal succeed
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		// It will not close with bad weight/len information
		run_to_block(3);
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), hash, 0, Weight::zero(), 0),
			Error::<Test, Instance1>::WrongProposalLength,
		);
		// Now we make the proposal fail
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, false));
		// It can close even if the weight/len information is bad
		run_to_block(5);
		assert_ok!(Collective::close(RuntimeOrigin::signed(2), hash, 0, Weight::zero(), 0));
	})
}

#[test]
fn disapprove_proposal_works() {
	ExtBuilder::default().build_and_execute(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			2,
			Box::new(proposal),
			proposal_len
		));
		// Proposal would normally succeed
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), hash, 0, true));
		// But Root can disapprove and remove it anyway
		assert_ok!(Collective::disapprove_proposal(RuntimeOrigin::root(), hash));
		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					proposal_hash: hash
				})),
			]
		);
	})
}

#[should_panic(expected = "Members length cannot exceed MaxMembers.")]
#[test]
fn genesis_build_panics_with_too_many_members() {
	let max_members: u32 = MaxMembers::get();
	let too_many_members = (1..=max_members as u64 + 1).collect::<Vec<AccountId>>();
	pallet_collective::GenesisConfig::<Test> {
		members: too_many_members,
		phantom: Default::default(),
	}
	.build_storage()
	.unwrap();
}

#[test]
#[should_panic(expected = "Members cannot contain duplicate accounts.")]
fn genesis_build_panics_with_duplicate_members() {
	pallet_collective::GenesisConfig::<Test> {
		members: vec![1, 2, 3, 1],
		phantom: Default::default(),
	}
	.build_storage()
	.unwrap();
}


use crate::{Error, Config, Module};
use frame_support::{assert_noop, assert_ok, impl_outer_dispatch};
use frame_system::{self as system, RawOrigin};
use sp_core::H256;
use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, testing::Header};
use crate::ChangeMembers;

impl_outer_dispatch! {
    pub enum OuterCall {
        Collective(crate::Call),
        System(system::Call),
    }
}

type AccountId = u64;

// Mock runtime to test the pallet
frame_support::construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: system::{Module, Call, Config},
        Collective: collective::{Module, Call, Config},
    }
);

#[test]
fn test_prime_account_not_member() {
    // Initialize your runtime
    let mut t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
    t.execute_with(|| {
        // Add a member to the collective
        let member_id: AccountId = 1;
        CollectiveModule::set_members(RawOrigin::Root.into(), vec![member_id.clone()]).unwrap();

        // Attempt to perform an action requiring membership with a non-member account
        let non_member_id: AccountId = 2;
        assert_noop!(
            CollectiveModule::some_function(RawOrigin::Signed(non_member_id).into()),
            Error::<Runtime>::PrimeAccountNotMember
        );

        // Test other functions returning PrimeAccountNotMember error
        assert_noop!(
            CollectiveModule::another_function(RawOrigin::Signed(non_member_id).into()),
            Error::<Runtime>::PrimeAccountNotMember
        );

        assert_noop!(
            CollectiveModule::yet_another_function(RawOrigin::Signed(non_member_id).into()),
            Error::<Runtime>::PrimeAccountNotMember
        );

    });
}
