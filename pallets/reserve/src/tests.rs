// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// A module in charge of accounting reserves

 #![cfg(test)]

 use crate as pallet_reserve;

 use frame_support::{
	assert_noop, assert_ok,
	dispatch::Weight,
	parameter_types,
    traits::Currency,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		DispatchClass,
	},
	StorageMap,
};

// use frame_support::{
//      assert_noop, assert_ok, ord_parameter_types,
//      parameter_types, traits::Currency,
//  };
 use frame_system::{EnsureSignedBy, RawOrigin};
 use frame_system::limits::{BlockLength, BlockWeights};
 use cord_primitives::{AccountId, Signature};
 use sp_core::H256;
 use sp_runtime::{
     testing::Header,
     traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
     MultiSignature, MultiSigner, Perbill, DispatchError::BadOrigin,
 };
 use sp_std::prelude::Box;
 
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
 
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        CordReserve: pallet_reserve::<Instance1>::{Module, Call, Storage, Config, Event<T>},
	}
);

const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

 parameter_types! {
    pub const BlockHashCount: u64 = 250;
	// pub BlockWeights: frame_system::limits::BlockWeights =
	// 	frame_system::limits::BlockWeights::simple_max(1024);
    pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u8 = 29;
	// pub const BlockHashCount: u64 = 250;   
    // pub const SS58Prefix: u8 = 29;
}

impl frame_system::Config for Test {
	type Origin = Origin;
	type Call = Call;
	type Index = u32;
	type BlockNumber = u32;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type SS58Prefix = SS58Prefix;
}


// impl frame_system::Config for Test {
//     type BaseCallFilter = ();
// 	type BlockWeights = ();
// 	type BlockLength = ();
// 	type DbWeight = ();
// 	type Origin = Origin;
// 	type Index = u64;
// 	type BlockNumber = u64;
// 	type Call = Call;
// 	type Hash = H256;
// 	type Hashing = BlakeTwo256;
// 	type AccountId = u64; 
// 	type Lookup = IdentityLookup<Self::AccountId>;
// 	type Header = Header;
// 	type Event = ();
// 	type BlockHashCount = BlockHashCount;
// 	type Version = ();
// 	type PalletInfo = PalletInfo;
// 	type AccountData = pallet_balances::AccountData<u64>;
// 	type OnNewAccount = ();
// 	type OnKilledAccount = ();
// 	type SystemWeightInfo = ();
// 	type SS58Prefix = ();
//  }

 parameter_types! {
     pub const MaxLocks: u32 = 50;
 }
 impl pallet_balances::Config for Test {
     type Balance = u64;
     type Event = ();
     type DustRemoval = ();
     type MaxLocks = MaxLocks;
     type ExistentialDeposit = ();
     type AccountStore = frame_system::Module<Test>;
     type WeightInfo = ();
 }
 
//  ord_parameter_types! {
     
//  }
 parameter_types! {
     pub const ReserveModuleId: ModuleId = ModuleId(*b"py/resrv");
     pub const Admin: u64 = 1;
 }
 impl Trait for Test {
     type Event = ();
     type Currency = pallet_balances::Module<Self>;
     type ExternalOrigin = EnsureSignedBy<Admin, u64>;
     type Call = Call;
     type ModuleId = ReserveModuleId;
 }

//  type CordReserve = Module<Test>;
 type TestCurrency = <Test as Trait>::Currency;
 
 // This function basically just builds a genesis storage key/value store according to
 // our desired mockup.
 pub fn new_test_ext() -> sp_io::TestExternalities {
     frame_system::GenesisConfig::default()
         .build_storage::<Test>()
         .unwrap()
         .into()
 }
 
 #[test]
 fn spend_error_if_bad_origin() {
     new_test_ext().execute_with(|| {
         assert_noop!(CordReserve::transfer(Origin::signed(0), 1, 1), BadOrigin);
     })
 }
 
 #[test]
 fn spend_funds_to_target() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&CordReserve::account_id(), 100);
 
         assert_eq!(Balances::free_balance(CordReserve::account_id()), 100);
         assert_eq!(Balances::free_balance(3), 0);
         assert_ok!(CordReserve::transfer(Origin::signed(Admin::get()), 3, 100));
         assert_eq!(Balances::free_balance(3), 100);
         assert_eq!(Balances::free_balance(CordReserve::account_id()), 0);
     })
 }
 
 #[test]
 fn receive() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&999, 100);
 
         assert_ok!(CordReserve::receive(Origin::signed(999), 50));
         assert_eq!(Balances::free_balance(999), 50);
         assert_eq!(Balances::free_balance(CordReserve::account_id()), 50);
     })
 }
 
 fn make_call(value: u8) -> Box<Call> {
     Box::new(Call::System(frame_system::Call::<Test>::remark(vec![
         value,
     ])))
 }
 
 #[test]
 fn apply_as_error_if_bad_origin() {
     new_test_ext().execute_with(|| {
         assert_noop!(
             CordReserve::apply_as(Origin::signed(0), make_call(1)),
             BadOrigin
         );
     })
 }
 
 #[test]
 fn apply_as_works() {
     new_test_ext().execute_with(|| {
         assert_ok!(CordReserve::apply_as(
             Origin::signed(Admin::get()),
             make_call(1)
         ));
     })
 }
 
 #[test]
 fn try_root_if_not_admin() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&CordReserve::account_id(), 100);
 
         assert_ok!(CordReserve::transfer(RawOrigin::Root.into(), 3, 100));
         assert_ok!(CordReserve::apply_as(RawOrigin::Root.into(), make_call(1)));
     })
 }