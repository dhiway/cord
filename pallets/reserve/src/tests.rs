// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// A module in charge of accounting reserves

 #![cfg(test)]

 use super::*;

use frame_support::{
     assert_noop, assert_ok, impl_outer_dispatch, impl_outer_origin, ord_parameter_types,
     parameter_types, traits::Currency, weights::Weight,
 };
 use frame_system::{EnsureSignedBy, RawOrigin};
 use sp_core::H256;
 use sp_runtime::{
     testing::Header,
     traits::{BlakeTwo256, IdentityLookup},
     DispatchError::BadOrigin,
     Perbill,
 };
 use sp_std::prelude::Box;
 
 impl_outer_origin! {
     pub enum Origin for Test {}
 }
 impl_outer_dispatch! {
     pub enum Call for Test where origin: Origin {
         frame_system::System,
     }
 }
 
 // For testing the module, we construct most of a mock runtime. This means
 // first constructing a configuration type (`Test`) which `impl`s each of the
 // configuration traits of modules we want to use.
 #[derive(Clone, Eq, PartialEq)]
 pub struct Test;
 parameter_types! {
     pub const BlockHashCount: u64 = 250;
     pub const MaximumBlockWeight: Weight = 1024;
     pub const MaximumBlockLength: u32 = 2 * 1024;
     pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
     pub const SS58Prefix: u8 = 29;
    }
 
impl frame_system::Config for Test {
    type BaseCallFilter = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<AccountId>;
	type Header = Header;
	type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
 }
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
 
 ord_parameter_types! {
     pub const Admin: u64 = 1;
 }
 parameter_types! {
     pub const ReserveModuleId: ModuleId = ModuleId(*b"py/resrv");
 }
 impl Trait for Test {
     type Event = ();
     type Currency = pallet_balances::Module<Self>;
     type ExternalOrigin = EnsureSignedBy<Admin, u64>;
     type Call = Call;
     type ModuleId = ReserveModuleId;
 }
 type TestModule = Module<Test>;
 type Balances = pallet_balances::Module<Test>;
 type System = frame_system::Module<Test>;
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
         assert_noop!(TestModule::transfer(Origin::signed(0), 1, 1), BadOrigin);
     })
 }
 
 #[test]
 fn spend_funds_to_target() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&TestModule::account_id(), 100);
 
         assert_eq!(Balances::free_balance(TestModule::account_id()), 100);
         assert_eq!(Balances::free_balance(3), 0);
         assert_ok!(TestModule::transfer(Origin::signed(Admin::get()), 3, 100));
         assert_eq!(Balances::free_balance(3), 100);
         assert_eq!(Balances::free_balance(TestModule::account_id()), 0);
     })
 }
 
 #[test]
 fn receive() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&999, 100);
 
         assert_ok!(TestModule::receive(Origin::signed(999), 50));
         assert_eq!(Balances::free_balance(999), 50);
         assert_eq!(Balances::free_balance(TestModule::account_id()), 50);
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
             TestModule::apply_as(Origin::signed(0), make_call(1)),
             BadOrigin
         );
     })
 }
 
 #[test]
 fn apply_as_works() {
     new_test_ext().execute_with(|| {
         assert_ok!(TestModule::apply_as(
             Origin::signed(Admin::get()),
             make_call(1)
         ));
     })
 }
 
 #[test]
 fn try_root_if_not_admin() {
     new_test_ext().execute_with(|| {
         TestCurrency::make_free_balance_be(&TestModule::account_id(), 100);
 
         assert_ok!(TestModule::transfer(RawOrigin::Root.into(), 3, 100));
         assert_ok!(TestModule::apply_as(RawOrigin::Root.into(), make_call(1)));
     })
 }