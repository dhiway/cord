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

//! PGAS (People Gas) fee payment adapters.
//!
//! PGAS is meant to be burned when used for fee payment rather than going to the staking pot.
//! This module provides:
//!
//! - [`BurnPgasHandleCredit`]: A `HandleCredit` adapter that burns PGAS credits and delegates to an
//!   inner handler for other assets.
//!
//! - [`PgasOnChargeAssetTransaction`]: An `OnChargeAssetTransaction` wrapper for
//!   `pallet_asset_conversion_tx_payment` that intercepts PGAS fee payments before they reach the
//!   swap logic.

use core::marker::PhantomData;

/// A [`HandleCredit`](pallet_asset_tx_payment::HandleCredit) adapter that burns PGAS credits
/// (by dropping them) and delegates to an inner handler for all other assets.
///
/// When the incoming credit's asset matches `PgasId::get()`, the credit is simply dropped,
/// which decreases the asset's total issuance (i.e., burns the PGAS). For any other asset,
/// the credit is forwarded to `Inner::handle_credit`.
pub struct BurnPgasHandleCredit<PgasId, Inner>(PhantomData<(PgasId, Inner)>);

impl<AccountId, B, PgasId, Inner> pallet_asset_tx_payment::HandleCredit<AccountId, B>
	for BurnPgasHandleCredit<PgasId, Inner>
where
	B: frame_support::traits::fungibles::Balanced<AccountId>,
	PgasId: sp_runtime::traits::Get<B::AssetId>,
	Inner: pallet_asset_tx_payment::HandleCredit<AccountId, B>,
{
	fn handle_credit(credit: frame_support::traits::fungibles::Credit<AccountId, B>) {
		if credit.asset() == PgasId::get() {
			// Drop the credit, which burns it.
			drop(credit);
		} else {
			Inner::handle_credit(credit);
		}
	}
}

mod conversion {
	use super::*;
	use frame_support::{
		traits::{
			fungibles,
			tokens::{
				ConversionToAssetBalance, Fortitude::Polite, Precision::Exact,
				Preservation::Expendable, WithdrawConsequence,
			},
		},
		unsigned::TransactionValidityError,
	};
	use pallet_transaction_payment::OnChargeTransaction;
	use sp_runtime::{
		traits::{DispatchInfoOf, Get, One, PostDispatchInfoOf, Zero},
		transaction_validity::InvalidTransaction,
	};

	/// Convert a native fee amount to a PGAS balance, ensuring that non-zero fees always
	/// result in at least a 1-unit charge, otherwise we may be open to spam with some unfortunate
	/// swap rate. Technically it should never happen with a 1:1 peg but this is up to the
	/// configured `ConversionToAssetBalance` and better be safe.
	fn native_to_pgas_fee<NativeBalance, AssetId, C>(
		fee: NativeBalance,
		asset_id: AssetId,
	) -> Result<NativeBalance, TransactionValidityError>
	where
		NativeBalance: Zero + One + PartialEq + Copy,
		AssetId: Clone,
		C: ConversionToAssetBalance<NativeBalance, AssetId, NativeBalance>,
	{
		let pgas_fee =
			C::to_asset_balance(fee, asset_id).map_err(|_| InvalidTransaction::Payment)?;
		if !fee.is_zero() && pgas_fee.is_zero() {
			Ok(NativeBalance::one())
		} else {
			Ok(pgas_fee)
		}
	}

	/// The native balance type, derived the same way as in `pallet_asset_conversion_tx_payment`.
	type NativeBalanceOf<T> =
		<<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<
			T,
		>>::Balance;

	/// Liquidity info wrapper that distinguishes between PGAS payments and other asset payments.
	pub enum PgasLiquidityInfo<PgasInfo, InnerInfo> {
		/// The fee was paid in PGAS.
		Pgas(PgasInfo),
		/// The fee was paid in another asset, delegated to the inner handler.
		Other(InnerInfo),
	}

	/// An [`OnChargeAssetTransaction`](pallet_asset_conversion_tx_payment::OnChargeAssetTransaction)
	/// wrapper that intercepts PGAS fee payments before they reach the swap logic. For all other
	/// assets, delegates to `Inner`.
	///
	/// When the fee asset is PGAS:
	/// - `withdraw_fee`: Converts the native fee to PGAS amount, withdraws PGAS from the user.
	/// - `correct_and_deposit_fee`: Splits the credit into (fee, refund), refunds overpayment, and
	///   drops the fee credit (burning it).
	pub struct PgasOnChargeAssetTransaction<PgasId, F, C, Inner>(
		PhantomData<(PgasId, F, C, Inner)>,
	);

	impl<T, PgasId, F, C, Inner> pallet_asset_conversion_tx_payment::OnChargeAssetTransaction<T>
		for PgasOnChargeAssetTransaction<PgasId, F, C, Inner>
	where
		T: pallet_asset_conversion_tx_payment::Config,
		PgasId: Get<T::AssetId>,
		F: fungibles::Balanced<T::AccountId, Balance = NativeBalanceOf<T>, AssetId = T::AssetId>,
		C: ConversionToAssetBalance<NativeBalanceOf<T>, T::AssetId, NativeBalanceOf<T>>,
		Inner: pallet_asset_conversion_tx_payment::OnChargeAssetTransaction<
			T,
			Balance = NativeBalanceOf<T>,
			AssetId = T::AssetId,
		>,
	{
		type Balance = NativeBalanceOf<T>;
		type AssetId = T::AssetId;
		type LiquidityInfo =
			PgasLiquidityInfo<fungibles::Credit<T::AccountId, F>, Inner::LiquidityInfo>;

		fn withdraw_fee(
			who: &T::AccountId,
			call: &T::RuntimeCall,
			dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
			asset_id: Self::AssetId,
			fee: Self::Balance,
			tip: Self::Balance,
		) -> Result<Self::LiquidityInfo, TransactionValidityError> {
			if asset_id == PgasId::get() {
				let pgas_fee = native_to_pgas_fee::<_, _, C>(fee, asset_id.clone())?;
				// `Expendable` lets a fee payment kill the payer's asset account (balance drops to
				// zero and the sufficient-asset account is reaped).
				let credit = F::withdraw(asset_id, who, pgas_fee, Exact, Expendable, Polite)
					.map_err(|_| InvalidTransaction::Payment)?;
				Ok(PgasLiquidityInfo::Pgas(credit))
			} else {
				Inner::withdraw_fee(who, call, dispatch_info, asset_id, fee, tip)
					.map(PgasLiquidityInfo::Other)
			}
		}

		fn can_withdraw_fee(
			who: &T::AccountId,
			asset_id: Self::AssetId,
			fee: Self::Balance,
		) -> Result<(), TransactionValidityError> {
			if asset_id == PgasId::get() {
				let pgas_fee = native_to_pgas_fee::<_, _, C>(fee, asset_id.clone())?;
				// Accept `ReducedToZero` too: under `Expendable` withdraw, a fee that exactly
				// drains the account is allowed and reaps the sufficient-asset account.
				match F::can_withdraw(asset_id, who, pgas_fee) {
					WithdrawConsequence::Success | WithdrawConsequence::ReducedToZero(_) => Ok(()),
					_ => Err(InvalidTransaction::Payment.into()),
				}
			} else {
				Inner::can_withdraw_fee(who, asset_id, fee)
			}
		}

		fn correct_and_deposit_fee(
			who: &T::AccountId,
			dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
			post_info: &PostDispatchInfoOf<T::RuntimeCall>,
			corrected_fee: Self::Balance,
			tip: Self::Balance,
			asset_id: Self::AssetId,
			already_withdraw: Self::LiquidityInfo,
		) -> Result<NativeBalanceOf<T>, TransactionValidityError> {
			match already_withdraw {
				PgasLiquidityInfo::Pgas(credit) => {
					let pgas_corrected = native_to_pgas_fee::<_, _, C>(corrected_fee, asset_id)?;
					let (fee_credit, refund_credit) = credit.split(pgas_corrected);
					// Refund excess.
					let _ = F::resolve(who, refund_credit);
					// Burn the fee.
					drop(fee_credit);
					Ok(corrected_fee)
				},
				PgasLiquidityInfo::Other(inner_info) => Inner::correct_and_deposit_fee(
					who,
					dispatch_info,
					post_info,
					corrected_fee,
					tip,
					asset_id,
					inner_info,
				),
			}
		}
	}
}

pub use conversion::{PgasLiquidityInfo, PgasOnChargeAssetTransaction};

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{
		derive_impl, parameter_types,
		traits::{
			fungibles::{Balanced, Credit, Inspect},
			tokens::{Fortitude::Polite, Precision::Exact, Preservation::Preserve},
		},
	};
	use pallet_asset_tx_payment::HandleCredit;
	use sp_runtime::BuildStorage;

	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test {
			System: frame_system,
			Balances: pallet_balances,
			Assets: pallet_assets,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Test {
		type Block = Block;
		type AccountData = pallet_balances::AccountData<u64>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
	impl pallet_balances::Config for Test {
		type AccountStore = System;
	}

	#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
	impl pallet_assets::Config for Test {
		type Currency = Balances;
		type CreateOrigin =
			frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
		type ForceOrigin = frame_system::EnsureRoot<u64>;
		type Holder = ();
	}

	parameter_types! {
		pub const PgasAssetId: u32 = 42;
	}

	use core::cell::RefCell;

	thread_local! {
		static INNER_HANDLER_CALLED: RefCell<bool> = const { RefCell::new(false) };
		static INNER_HANDLER_AMOUNT: RefCell<u64> = const { RefCell::new(0) };
	}

	pub struct MockInnerHandler;
	impl HandleCredit<u64, pallet_assets::Pallet<Test>> for MockInnerHandler {
		fn handle_credit(credit: Credit<u64, pallet_assets::Pallet<Test>>) {
			INNER_HANDLER_CALLED.with(|c| *c.borrow_mut() = true);
			INNER_HANDLER_AMOUNT.with(|a| *a.borrow_mut() = credit.peek());
			// Drop credit (burns it).
			drop(credit);
		}
	}

	fn reset_inner_handler_state() {
		INNER_HANDLER_CALLED.with(|c| *c.borrow_mut() = false);
		INNER_HANDLER_AMOUNT.with(|a| *a.borrow_mut() = 0);
	}

	fn inner_handler_was_called() -> bool {
		INNER_HANDLER_CALLED.with(|c| *c.borrow())
	}

	fn inner_handler_amount() -> u64 {
		INNER_HANDLER_AMOUNT.with(|a| *a.borrow())
	}

	type BurnPgas = BurnPgasHandleCredit<PgasAssetId, MockInnerHandler>;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(1, 10_000), (2, 10_000)],
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}

	// Helper function to create an asset and mint to a beneficiary.
	fn setup_asset(asset_id: u32, owner: u64, beneficiary: u64, amount: u64) {
		assert!(Assets::force_create(RuntimeOrigin::root(), asset_id, owner, true, 1).is_ok());
		assert!(Assets::mint(RuntimeOrigin::signed(owner), asset_id, beneficiary, amount).is_ok());
	}

	#[test]
	fn pgas_credit_is_burned_and_total_issuance_decreases() {
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 1_000);

			let initial_issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(initial_issuance, 1_000);

			// Withdraw PGAS from account 2.
			let credit = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				100,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();

			assert_eq!(credit.peek(), 100);
			assert_eq!(credit.asset(), PgasAssetId::get());

			reset_inner_handler_state();

			// Handle credit via BurnPgas. It should be burned, not delegated to inner.
			BurnPgas::handle_credit(credit);

			assert!(!inner_handler_was_called());

			// Total issuance decreased by the burned amount.
			let final_issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(final_issuance, 900);
		});
	}

	#[test]
	fn non_pgas_credit_delegates_to_inner_handler() {
		new_test_ext().execute_with(|| {
			let non_pgas_id = 99u32;
			setup_asset(non_pgas_id, 1, 2, 1_000);

			// Withdraw non-PGAS from account 2.
			let credit = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				non_pgas_id,
				&2,
				200,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();

			reset_inner_handler_state();

			// Handle credit via BurnPgas. It should be delegated to inner.
			BurnPgas::handle_credit(credit);

			assert!(inner_handler_was_called());
			assert_eq!(inner_handler_amount(), 200);
		});
	}

	#[test]
	fn pgas_fee_correction_refund_scenario() {
		// Simulates the fee correction flow: user pays estimated fee, then gets refund.
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 1_000);

			// Step 1: Withdraw estimated fee (e.g. 100 PGAS).
			let credit = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				100,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();

			// Step 2: Actual fee is only 60 PGAS. Split credit.
			let (fee_credit, refund_credit) = credit.split(60);
			assert_eq!(fee_credit.peek(), 60);
			assert_eq!(refund_credit.peek(), 40);

			// Step 3: Refund the overpayment to the user.
			let _ = <pallet_assets::Pallet<Test> as Balanced<u64>>::resolve(&2, refund_credit);

			// Step 4: Burn the fee via BurnPgas.
			reset_inner_handler_state();
			BurnPgas::handle_credit(fee_credit);

			assert!(!inner_handler_was_called());

			// User balance: started with 1000, paid 100, got 40 back = 940.
			let user_balance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::balance(PgasAssetId::get(), &2);
			assert_eq!(user_balance, 940);

			// Total issuance: started with 1000, burned 60 = 940.
			let issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(issuance, 940);
		});
	}

	#[test]
	fn zero_pgas_credit_is_handled_without_side_effects() {
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 1_000);

			let initial_issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());

			// Create a zero-value credit.
			let credit = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				0,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();

			reset_inner_handler_state();
			BurnPgas::handle_credit(credit);

			assert!(!inner_handler_was_called());

			let final_issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(final_issuance, initial_issuance);
		});
	}

	#[test]
	fn user_balance_decreases_after_pgas_burn() {
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 1_000);

			let initial_balance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::balance(PgasAssetId::get(), &2);
			assert_eq!(initial_balance, 1_000);

			let credit = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				300,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();

			BurnPgas::handle_credit(credit);

			let final_balance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::balance(PgasAssetId::get(), &2);
			assert_eq!(final_balance, 700);
		});
	}

	#[test]
	fn insufficient_balance_prevents_withdrawal() {
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 50);

			// Try to withdraw more than balance - should fail at the withdraw level.
			let result = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				100,
				Exact,
				Preserve,
				Polite,
			);

			assert!(result.is_err());

			// Balance and issuance unchanged.
			let balance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::balance(PgasAssetId::get(), &2);
			assert_eq!(balance, 50);
			let issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(issuance, 50);
		});
	}

	#[test]
	fn multiple_sequential_pgas_burns_accumulate() {
		new_test_ext().execute_with(|| {
			setup_asset(PgasAssetId::get(), 1, 2, 1_000);

			let initial_issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(initial_issuance, 1_000);

			// Burn 100 PGAS.
			let credit1 = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				100,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();
			BurnPgas::handle_credit(credit1);

			// Burn another 200 PGAS.
			let credit2 = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				200,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();
			BurnPgas::handle_credit(credit2);

			// Burn 50 more.
			let credit3 = <pallet_assets::Pallet<Test> as Balanced<u64>>::withdraw(
				PgasAssetId::get(),
				&2,
				50,
				Exact,
				Preserve,
				Polite,
			)
			.unwrap();
			BurnPgas::handle_credit(credit3);

			// User balance: 1000 - 100 - 200 - 50 = 650.
			let balance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::balance(PgasAssetId::get(), &2);
			assert_eq!(balance, 650);

			// Total issuance: 1000 - 350 = 650.
			let issuance =
				<pallet_assets::Pallet<Test> as Inspect<u64>>::total_issuance(PgasAssetId::get());
			assert_eq!(issuance, 650);
		});
	}
}

#[cfg(test)]
mod conversion_tests {
	use frame_support::{
		derive_impl, parameter_types,
		traits::{
			fungibles::{Balanced, Inspect},
			tokens::{
				ConversionToAssetBalance, Fortitude::Polite, Precision::Exact,
				Preservation::Preserve, WithdrawConsequence,
			},
		},
	};
	use pallet_asset_conversion_tx_payment::OnChargeAssetTransaction;
	use sp_runtime::BuildStorage;

	use super::conversion::*;

	type Block = frame_system::mocking::MockBlock<ConvTest>;

	frame_support::construct_runtime!(
		pub enum ConvTest {
			System: frame_system,
			Balances: pallet_balances,
			Assets: pallet_assets,
			TransactionPayment: pallet_transaction_payment,
			AssetConversionTxPayment: pallet_asset_conversion_tx_payment,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for ConvTest {
		type Block = Block;
		type AccountData = pallet_balances::AccountData<u64>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
	impl pallet_balances::Config for ConvTest {
		type AccountStore = System;
	}

	#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
	impl pallet_assets::Config for ConvTest {
		type Currency = Balances;
		type CreateOrigin =
			frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
		type ForceOrigin = frame_system::EnsureRoot<u64>;
		type Holder = ();
	}

	impl pallet_transaction_payment::Config for ConvTest {
		type RuntimeEvent = RuntimeEvent;
		type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
		type OperationalFeeMultiplier = sp_runtime::traits::ConstU8<5>;
		type WeightToFee = frame_support::weights::IdentityFee<u64>;
		type LengthToFee = frame_support::weights::IdentityFee<u64>;
		type FeeMultiplierUpdate = ();
		type WeightInfo = ();
	}

	parameter_types! {
		pub const PgasId: u32 = 42;
		pub const OtherAssetId: u32 = 99;
	}

	/// 1:1 conversion for testing.
	pub struct IdentityConversion;
	impl ConversionToAssetBalance<u64, u32, u64> for IdentityConversion {
		type Error = ();
		fn to_asset_balance(balance: u64, _asset_id: u32) -> Result<u64, Self::Error> {
			Ok(balance)
		}
	}

	/// Mock inner handler that just does a simple withdraw + hold for non-PGAS assets.
	pub struct MockInnerOnCharge;
	impl OnChargeAssetTransaction<ConvTest> for MockInnerOnCharge {
		type Balance = u64;
		type AssetId = u32;
		type LiquidityInfo =
			frame_support::traits::fungibles::Credit<u64, pallet_assets::Pallet<ConvTest>>;

		fn withdraw_fee(
			who: &u64,
			_call: &RuntimeCall,
			_dispatch_info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
			asset_id: Self::AssetId,
			fee: Self::Balance,
			_tip: Self::Balance,
		) -> Result<Self::LiquidityInfo, frame_support::unsigned::TransactionValidityError> {
			<pallet_assets::Pallet<ConvTest> as Balanced<u64>>::withdraw(
				asset_id, who, fee, Exact, Preserve, Polite,
			)
			.map_err(|_| sp_runtime::transaction_validity::InvalidTransaction::Payment.into())
		}

		fn can_withdraw_fee(
			who: &u64,
			asset_id: Self::AssetId,
			fee: Self::Balance,
		) -> Result<(), frame_support::unsigned::TransactionValidityError> {
			match <pallet_assets::Pallet<ConvTest> as Inspect<u64>>::can_withdraw(
				asset_id, who, fee,
			) {
				WithdrawConsequence::Success => Ok(()),
				_ => Err(sp_runtime::transaction_validity::InvalidTransaction::Payment.into()),
			}
		}

		fn correct_and_deposit_fee(
			who: &u64,
			_dispatch_info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
			_post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
			corrected_fee: Self::Balance,
			_tip: Self::Balance,
			_asset_id: Self::AssetId,
			already_withdraw: Self::LiquidityInfo,
		) -> Result<u64, frame_support::unsigned::TransactionValidityError> {
			let (fee_credit, refund) = already_withdraw.split(corrected_fee);
			let _ = <pallet_assets::Pallet<ConvTest> as Balanced<u64>>::resolve(who, refund);
			drop(fee_credit);
			Ok(corrected_fee)
		}
	}

	type PgasAdapter = PgasOnChargeAssetTransaction<
		PgasId,
		pallet_assets::Pallet<ConvTest>,
		IdentityConversion,
		MockInnerOnCharge,
	>;

	impl pallet_asset_conversion_tx_payment::Config for ConvTest {
		type RuntimeEvent = RuntimeEvent;
		type AssetId = u32;
		type OnChargeAssetTransaction = PgasAdapter;
		type WeightInfo = ();
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper = super::StubBenchmarkHelper;
	}

	fn new_conv_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<ConvTest>::default().build_storage().unwrap();
		pallet_balances::GenesisConfig::<ConvTest> {
			balances: vec![(1, 10_000), (2, 10_000)],
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
			// Create PGAS asset.
			pallet_assets::Pallet::<ConvTest>::force_create(
				RuntimeOrigin::root(),
				PgasId::get(),
				1,
				true,
				1,
			)
			.unwrap();
			pallet_assets::Pallet::<ConvTest>::mint(
				RuntimeOrigin::signed(1),
				PgasId::get(),
				2,
				5_000,
			)
			.unwrap();
			// Create another asset.
			pallet_assets::Pallet::<ConvTest>::force_create(
				RuntimeOrigin::root(),
				OtherAssetId::get(),
				1,
				true,
				1,
			)
			.unwrap();
			pallet_assets::Pallet::<ConvTest>::mint(
				RuntimeOrigin::signed(1),
				OtherAssetId::get(),
				2,
				5_000,
			)
			.unwrap();
		});
		ext
	}

	fn dummy_dispatch_info() -> sp_runtime::traits::DispatchInfoOf<RuntimeCall> {
		frame_support::dispatch::DispatchInfo {
			call_weight: frame_support::weights::Weight::zero(),
			extension_weight: frame_support::weights::Weight::zero(),
			class: frame_support::dispatch::DispatchClass::Normal,
			pays_fee: frame_support::dispatch::Pays::Yes,
		}
	}

	fn dummy_post_info() -> sp_runtime::traits::PostDispatchInfoOf<RuntimeCall> {
		sp_runtime::DispatchErrorWithPostInfo {
			post_info: frame_support::dispatch::PostDispatchInfo {
				actual_weight: None,
				pays_fee: frame_support::dispatch::Pays::Yes,
			},
			error: sp_runtime::DispatchError::Other("dummy"),
		}
		.post_info
	}

	fn dummy_call() -> RuntimeCall {
		RuntimeCall::System(frame_system::Call::remark { remark: vec![] })
	}

	#[test]
	fn pgas_withdraw_fee_burns_on_correction() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			let initial_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());
			let initial_balance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(PgasId::get(), &2);

			// Withdraw 100 PGAS as fee.
			let liquidity =
				PgasAdapter::withdraw_fee(&2, &call, &info, PgasId::get(), 100, 0).unwrap();
			assert!(matches!(liquidity, PgasLiquidityInfo::Pgas(_)));

			// Correct fee down to 60.
			let post = dummy_post_info();
			let result = PgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				60,
				0,
				PgasId::get(),
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 60);

			// User paid 60 (100 withdrawn - 40 refunded).
			let final_balance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(PgasId::get(), &2);
			assert_eq!(final_balance, initial_balance - 60);

			// Total issuance decreased by 60 (burned).
			let final_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());
			assert_eq!(final_issuance, initial_issuance - 60);
		});
	}

	#[test]
	fn non_pgas_delegates_to_inner() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			// Withdraw a non-PGAS asset.
			let liquidity =
				PgasAdapter::withdraw_fee(&2, &call, &info, OtherAssetId::get(), 50, 0).unwrap();
			assert!(matches!(liquidity, PgasLiquidityInfo::Other(_)));

			// Correct fee.
			let post = dummy_post_info();
			let result = PgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				30,
				0,
				OtherAssetId::get(),
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 30);

			// User paid 30 of the non-PGAS asset.
			let balance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(OtherAssetId::get(), &2);
			assert_eq!(balance, 5_000 - 30);
		});
	}

	#[test]
	fn pgas_can_withdraw_fee_checks_balance() {
		new_conv_test_ext().execute_with(|| {
			// Should succeed with enough balance.
			assert!(PgasAdapter::can_withdraw_fee(&2, PgasId::get(), 100).is_ok());

			// Should fail with insufficient balance.
			assert!(PgasAdapter::can_withdraw_fee(&2, PgasId::get(), 10_000).is_err());
		});
	}

	#[test]
	fn pgas_insufficient_balance_rejected() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			// Try to pay more PGAS than available.
			let result = PgasAdapter::withdraw_fee(&2, &call, &info, PgasId::get(), 10_000, 0);
			assert!(result.is_err());
		});
	}

	#[test]
	fn pgas_zero_fee_works() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			let initial_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());

			// Zero fee should succeed without side effects.
			let liquidity =
				PgasAdapter::withdraw_fee(&2, &call, &info, PgasId::get(), 0, 0).unwrap();

			let post = dummy_post_info();
			let result = PgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				0,
				0,
				PgasId::get(),
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 0);

			let final_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());
			assert_eq!(final_issuance, initial_issuance);
		});
	}

	#[test]
	fn pgas_no_refund_when_corrected_fee_equals_withdrawn() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			let initial_balance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(PgasId::get(), &2);
			let initial_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());

			// Withdraw exactly 100 PGAS.
			let liquidity =
				PgasAdapter::withdraw_fee(&2, &call, &info, PgasId::get(), 100, 0).unwrap();

			// Corrected fee is the same as withdrawn — no refund.
			let post = dummy_post_info();
			let result = PgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				100,
				0,
				PgasId::get(),
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 100);

			// User paid exactly 100.
			let final_balance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(PgasId::get(), &2);
			assert_eq!(final_balance, initial_balance - 100);

			// Total issuance decreased by 100.
			let final_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());
			assert_eq!(final_issuance, initial_issuance - 100);
		});
	}

	#[test]
	fn pgas_nonzero_tip_does_not_affect_burn() {
		new_conv_test_ext().execute_with(|| {
			let info = dummy_dispatch_info();
			let call = dummy_call();

			let initial_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());

			// fee=100 includes tip=30.
			let liquidity =
				PgasAdapter::withdraw_fee(&2, &call, &info, PgasId::get(), 100, 30).unwrap();
			assert!(matches!(liquidity, PgasLiquidityInfo::Pgas(_)));

			// Corrected fee down to 80 (still includes tip=30).
			let post = dummy_post_info();
			let result = PgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				80,
				30,
				PgasId::get(),
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 80);

			// 80 burned (20 refunded from original 100).
			let final_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(PgasId::get());
			assert_eq!(final_issuance, initial_issuance - 80);
		});
	}

	#[test]
	fn pgas_partial_refund_lost_when_account_was_reaped_during_withdraw() {
		// Edge case worth pinning down: with `Expendable` preservation, a payer whose
		// post-fee balance would land below ED gets reaped during `withdraw_fee`. If
		// `correct_and_deposit_fee` then needs to refund the overpayment, the refund credit
		// can't be re-deposited into the now-nonexistent account when the refund amount is
		// itself below ED — `pallet_assets::can_increase` rejects with `BelowMinimum`.
		// `F::resolve` returns the credit in `Err`, our `let _ = F::resolve(...)` discards
		// it, and the credit drops — silently burning the refund.
		//
		// Substrate's standard fee paths (`pallet-transaction-payment::FungibleAdapter` with
		// `Preserve`, `pallet-asset-tx-payment::FungiblesAdapter` with `Protect`) sidestep
		// this by rejecting at `can_withdraw_fee` time when the user can't pay-and-preserve.
		// PGAS chose `Expendable` so the last-tx-drains-the-account UX works, with this
		// corner case as the cost.
		new_conv_test_ext().execute_with(|| {
			// Use a fresh asset id with a high min_balance so a small refund really is below
			// ED. (The shared PGAS asset in `new_conv_test_ext` has ED=1 and can re-create
			// accounts with arbitrarily small deposits.)
			const HIGH_ED_ASSET_ID: u32 = 7;
			const HIGH_ED: u64 = 100;
			pallet_assets::Pallet::<ConvTest>::force_create(
				RuntimeOrigin::root(),
				HIGH_ED_ASSET_ID,
				1,
				true,
				HIGH_ED,
			)
			.unwrap();
			pallet_assets::Pallet::<ConvTest>::mint(
				RuntimeOrigin::signed(1),
				HIGH_ED_ASSET_ID,
				2,
				1_000,
			)
			.unwrap();

			// Build a wrapper that mints into this high-ED asset.
			parameter_types! {
				pub const HighEdPgasId: u32 = HIGH_ED_ASSET_ID;
			}
			type HighEdPgasAdapter = super::conversion::PgasOnChargeAssetTransaction<
				HighEdPgasId,
				pallet_assets::Pallet<ConvTest>,
				IdentityConversion,
				MockInnerOnCharge,
			>;

			let info = dummy_dispatch_info();
			let call = dummy_call();
			let initial_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(HIGH_ED_ASSET_ID);

			// Withdraw the full 1_000 — exactly all of account 2's balance. Under `Expendable`
			// on a sufficient asset this reaps the account.
			let liquidity =
				HighEdPgasAdapter::withdraw_fee(&2, &call, &info, HIGH_ED_ASSET_ID, 1_000, 0)
					.unwrap();
			assert!(matches!(liquidity, PgasLiquidityInfo::Pgas(_)));
			assert_eq!(
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(HIGH_ED_ASSET_ID, &2),
				0,
			);
			assert!(!pallet_assets::Account::<ConvTest>::contains_key(HIGH_ED_ASSET_ID, 2));

			// Correct down to 950 → refund=50, which is < HIGH_ED (100). Resolving that
			// credit back to the reaped account fails (`BelowMinimum`); our wrapper drops the
			// returned credit, burning the refund.
			let post = dummy_post_info();
			let result = HighEdPgasAdapter::correct_and_deposit_fee(
				&2,
				&info,
				&post,
				950,
				0,
				HIGH_ED_ASSET_ID,
				liquidity,
			)
			.unwrap();
			assert_eq!(result, 950);

			// Account stays reaped — the 50 refund didn't recreate it.
			assert_eq!(
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::balance(HIGH_ED_ASSET_ID, &2),
				0,
			);
			assert!(!pallet_assets::Account::<ConvTest>::contains_key(HIGH_ED_ASSET_ID, 2));

			// The over-charge: total issuance dropped by the full pre-dispatch amount (1_000),
			// not just by the corrected fee (950). The 50 unit refund was burned.
			let final_issuance =
				<pallet_assets::Pallet<ConvTest> as Inspect<u64>>::total_issuance(HIGH_ED_ASSET_ID);
			assert_eq!(final_issuance, initial_issuance - 1_000);
		});
	}
}

/// Shared stub for `BenchmarkHelper` associated types on the payment pallets.
#[cfg(all(test, feature = "runtime-benchmarks"))]
pub struct StubBenchmarkHelper;

#[cfg(all(test, feature = "runtime-benchmarks"))]
impl pallet_asset_tx_payment::BenchmarkHelperTrait<u64, u32, u32> for StubBenchmarkHelper {
	fn create_asset_id_parameter(_id: u32) -> (u32, u32) {
		panic!("benchmark helper should not be invoked from `cargo test`");
	}
	fn setup_balances_and_pool(_asset_id: u32, _account: u64) {
		panic!("benchmark helper should not be invoked from `cargo test`");
	}
}

#[cfg(all(test, feature = "runtime-benchmarks"))]
impl pallet_asset_conversion_tx_payment::BenchmarkHelperTrait<u64, u32, u32>
	for StubBenchmarkHelper
{
	fn create_asset_id_parameter(_id: u32) -> (u32, u32) {
		panic!("benchmark helper should not be invoked from `cargo test`");
	}
	fn setup_balances_and_pool(_asset_id: u32, _account: u64) {
		panic!("benchmark helper should not be invoked from `cargo test`");
	}
}

/// Test-only pallet shared by the apply-level tests below. Its `half_weight_refund` call
/// declares a weight up front and then consumes only half of it at dispatch time — the
/// transaction extension's `post_dispatch` observes the overpayment and refunds the
/// weight-based portion of the fee it withheld in excess. A second call, `no_refund_call`,
/// consumes the full declared weight and is used where a test needs predictable
/// "user pays exactly X" arithmetic.
#[cfg(test)]
#[frame_support::pallet]
pub mod refund_test_pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Weight the call declares up front (full fee is withdrawn against this amount).
	pub const DECLARED_WEIGHT: Weight = Weight::from_parts(1_000_000, 0);
	/// Weight the call actually consumes — half of [`DECLARED_WEIGHT`].
	pub const ACTUAL_WEIGHT: Weight = Weight::from_parts(500_000, 0);

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dispatches trivially and reports [`ACTUAL_WEIGHT`] as the actual weight consumed,
		/// leaving [`Pays::Yes`] intact so the extension refunds only half of the pre-dispatch
		/// fee.
		#[pallet::call_index(0)]
		#[pallet::weight(DECLARED_WEIGHT)]
		pub fn half_weight_refund(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Ok(Some(ACTUAL_WEIGHT).into())
		}

		/// Dispatches trivially without any weight refund; the user is charged the full
		/// declared fee. Useful for testing scenarios that hinge on no refund happening
		/// (e.g. the payer's asset account staying reaped after fee payment).
		#[pallet::call_index(1)]
		#[pallet::weight(DECLARED_WEIGHT)]
		pub fn no_refund_call(origin: OriginFor<T>) -> DispatchResult {
			ensure_signed(origin)?;
			Ok(())
		}
	}
}

#[cfg(test)]
mod asset_tx_payment_apply_tests {
	use super::*;
	use frame_support::{
		derive_impl,
		dispatch::GetDispatchInfo,
		parameter_types,
		traits::{
			fungibles::{Credit, Inspect, Mutate},
			tokens::ConversionToAssetBalance,
		},
		weights::IdentityFee,
	};
	use pallet_asset_tx_payment::{ChargeAssetTxPayment, FungiblesAdapter, HandleCredit};
	use sp_runtime::{traits::DispatchTransaction, BuildStorage};

	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test {
			System: frame_system,
			Balances: pallet_balances,
			Assets: pallet_assets,
			TransactionPayment: pallet_transaction_payment,
			AssetTxPayment: pallet_asset_tx_payment,
			Refund: refund_test_pallet,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for Test {
		type Block = Block;
		type AccountData = pallet_balances::AccountData<u64>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
	impl pallet_balances::Config for Test {
		type AccountStore = System;
	}

	#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
	impl pallet_assets::Config for Test {
		type Currency = Balances;
		type CreateOrigin =
			frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
		type ForceOrigin = frame_system::EnsureRoot<u64>;
		type Holder = ();
	}

	impl pallet_transaction_payment::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
		type OperationalFeeMultiplier = sp_runtime::traits::ConstU8<5>;
		type WeightToFee = IdentityFee<u64>;
		type LengthToFee = IdentityFee<u64>;
		type FeeMultiplierUpdate = ();
		type WeightInfo = ();
	}

	parameter_types! {
		pub const PgasId: u32 = 42;
	}

	/// Inner `HandleCredit` that should never be called in these tests — the whole point is that
	/// PGAS credits are intercepted by `BurnPgasHandleCredit` before reaching the inner handler.
	pub struct UnreachableInner;
	impl HandleCredit<u64, pallet_assets::Pallet<Test>> for UnreachableInner {
		fn handle_credit(_: Credit<u64, pallet_assets::Pallet<Test>>) {
			panic!("inner handler should not be reached for PGAS");
		}
	}

	/// 1:1 native↔PGAS conversion — tests run with the implicit assumption that one unit of fee
	/// costs one PGAS.
	pub struct IdentityConversion;
	impl ConversionToAssetBalance<u64, u32, u64> for IdentityConversion {
		type Error = ();
		fn to_asset_balance(balance: u64, _asset_id: u32) -> Result<u64, Self::Error> {
			Ok(balance)
		}
	}

	type PgasCredits = BurnPgasHandleCredit<PgasId, UnreachableInner>;

	impl pallet_asset_tx_payment::Config for Test {
		type RuntimeEvent = RuntimeEvent;
		type Fungibles = Assets;
		type OnChargeAssetTransaction = FungiblesAdapter<IdentityConversion, PgasCredits>;
		type WeightInfo = ();
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper = super::StubBenchmarkHelper;
	}

	impl refund_test_pallet::Config for Test {}

	fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
			// Create the PGAS asset and seed account 1 with enough PGAS to cover a worst-case
			// weight-based fee (`IdentityFee<u64>` converts the call's weight 1:1 into units).
			assert!(Assets::force_create(RuntimeOrigin::root(), PgasId::get(), 1, true, 1).is_ok());
			assert!(Assets::mint_into(PgasId::get(), &1u64, 1_000_000_000).is_ok());
		});
		ext
	}

	/// Build a signed origin + call + `ChargeAssetTxPayment` extension and run the full
	/// validate → prepare → dispatch → post_dispatch cycle via
	/// [`DispatchTransaction::dispatch_transaction`]. Returns the inner dispatch result.
	fn dispatch_refund_call_with_tip(tip: u64) -> sp_runtime::DispatchResult {
		let call = RuntimeCall::Refund(refund_test_pallet::Call::half_weight_refund {});
		let info = call.get_dispatch_info();
		let extension = ChargeAssetTxPayment::<Test>::from(tip, Some(PgasId::get()));
		extension
			.dispatch_transaction(RuntimeOrigin::signed(1), call, &info, 0, 0)
			.expect("transaction should be valid")
			.map(|_| ())
			.map_err(|e| e.error)
	}

	/// Expected fee the user pays after the half-weight refund, computed using the same helpers
	/// the extension's `post_dispatch` invokes. `IdentityFee<u64>` + a unit fee multiplier make
	/// this exact.
	fn expected_actual_fee(tip: u64) -> u64 {
		let call = RuntimeCall::Refund(refund_test_pallet::Call::half_weight_refund {});
		let info = call.get_dispatch_info();
		let post = frame_support::dispatch::PostDispatchInfo {
			actual_weight: Some(refund_test_pallet::ACTUAL_WEIGHT),
			pays_fee: frame_support::dispatch::Pays::Yes,
		};
		pallet_transaction_payment::Pallet::<Test>::compute_actual_fee(0, &info, &post, tip)
	}

	#[test]
	fn full_dispatch_burns_fee_for_actual_weight_only() {
		new_test_ext().execute_with(|| {
			let initial_balance = Assets::balance(PgasId::get(), 1u64);
			let initial_issuance = Assets::total_issuance(PgasId::get());
			let expected = expected_actual_fee(0);

			assert!(dispatch_refund_call_with_tip(0).is_ok());

			let paid = initial_balance - Assets::balance(PgasId::get(), 1u64);
			let burned = initial_issuance - Assets::total_issuance(PgasId::get());

			// The extension withdrew for the full declared weight and refunded the half that was
			// unused; the user pays exactly the actual fee the payment pallet would compute.
			assert_eq!(paid, expected);
			// Everything the user paid was burned: nothing leaked to `UnreachableInner` or any
			// other sink.
			assert_eq!(burned, expected);
		});
	}

	#[test]
	fn full_dispatch_with_tip_burns_fee_plus_tip() {
		new_test_ext().execute_with(|| {
			let initial_balance = Assets::balance(PgasId::get(), 1u64);
			let initial_issuance = Assets::total_issuance(PgasId::get());
			let tip = 123u64;
			let expected = expected_actual_fee(tip);

			assert!(dispatch_refund_call_with_tip(tip).is_ok());

			let paid = initial_balance - Assets::balance(PgasId::get(), 1u64);
			let burned = initial_issuance - Assets::total_issuance(PgasId::get());

			// Tip is never refunded so it folds into the actual fee; the weight-component refund
			// still applies to the rest.
			assert_eq!(paid, expected);
			assert_eq!(burned, expected);
			// Sanity: adding a tip of `tip` to the same call with the same actual weight should
			// simply add `tip` to the fee.
			assert_eq!(expected, expected_actual_fee(0) + tip);
		});
	}
}

#[cfg(test)]
mod asset_conversion_tx_payment_apply_tests {
	use super::*;
	use frame_support::{
		derive_impl,
		dispatch::GetDispatchInfo,
		parameter_types,
		traits::{
			fungibles::{Inspect, Mutate},
			tokens::ConversionToAssetBalance,
		},
		weights::IdentityFee,
	};
	use pallet_asset_conversion_tx_payment::{ChargeAssetTxPayment, OnChargeAssetTransaction};
	use sp_runtime::{traits::DispatchTransaction, BuildStorage};

	type Block = frame_system::mocking::MockBlock<ConvTest>;

	frame_support::construct_runtime!(
		pub enum ConvTest {
			System: frame_system,
			Balances: pallet_balances,
			Assets: pallet_assets,
			TransactionPayment: pallet_transaction_payment,
			AssetConversionTxPayment: pallet_asset_conversion_tx_payment,
			Refund: refund_test_pallet,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
	impl frame_system::Config for ConvTest {
		type Block = Block;
		type AccountData = pallet_balances::AccountData<u64>;
	}

	#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
	impl pallet_balances::Config for ConvTest {
		type AccountStore = System;
	}

	#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
	impl pallet_assets::Config for ConvTest {
		type Currency = Balances;
		type CreateOrigin =
			frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<u64>>;
		type ForceOrigin = frame_system::EnsureRoot<u64>;
		type Holder = ();
	}

	impl pallet_transaction_payment::Config for ConvTest {
		type RuntimeEvent = RuntimeEvent;
		type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
		type OperationalFeeMultiplier = sp_runtime::traits::ConstU8<5>;
		type WeightToFee = IdentityFee<u64>;
		type LengthToFee = IdentityFee<u64>;
		type FeeMultiplierUpdate = ();
		type WeightInfo = ();
	}

	parameter_types! {
		pub const PgasId: u32 = 42;
	}

	pub struct IdentityConversion;
	impl ConversionToAssetBalance<u64, u32, u64> for IdentityConversion {
		type Error = ();
		fn to_asset_balance(balance: u64, _asset_id: u32) -> Result<u64, Self::Error> {
			Ok(balance)
		}
	}

	/// Fallback `OnChargeAssetTransaction` that should never be called: these tests only exercise
	/// the PGAS path, so the `Inner` branch in `PgasOnChargeAssetTransaction` panicking would
	/// surface accidental mis-routing as test failures.
	pub struct UnreachableInner;
	impl OnChargeAssetTransaction<ConvTest> for UnreachableInner {
		type Balance = u64;
		type AssetId = u32;
		type LiquidityInfo = ();

		fn withdraw_fee(
			_: &u64,
			_: &RuntimeCall,
			_: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
			_: Self::AssetId,
			_: Self::Balance,
			_: Self::Balance,
		) -> Result<Self::LiquidityInfo, frame_support::unsigned::TransactionValidityError> {
			panic!("inner OnChargeAssetTransaction should not be reached for PGAS");
		}

		fn can_withdraw_fee(
			_: &u64,
			_: Self::AssetId,
			_: Self::Balance,
		) -> Result<(), frame_support::unsigned::TransactionValidityError> {
			panic!("inner can_withdraw_fee should not be reached for PGAS");
		}

		fn correct_and_deposit_fee(
			_: &u64,
			_: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
			_: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
			_: Self::Balance,
			_: Self::Balance,
			_: Self::AssetId,
			_: Self::LiquidityInfo,
		) -> Result<u64, frame_support::unsigned::TransactionValidityError> {
			panic!("inner correct_and_deposit_fee should not be reached for PGAS");
		}
	}

	type PgasOnCharge = super::conversion::PgasOnChargeAssetTransaction<
		PgasId,
		pallet_assets::Pallet<ConvTest>,
		IdentityConversion,
		UnreachableInner,
	>;

	impl pallet_asset_conversion_tx_payment::Config for ConvTest {
		type RuntimeEvent = RuntimeEvent;
		type AssetId = u32;
		type OnChargeAssetTransaction = PgasOnCharge;
		type WeightInfo = ();
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper = super::StubBenchmarkHelper;
	}

	impl refund_test_pallet::Config for ConvTest {}

	/// Existential deposit / min_balance for the PGAS asset in these tests. Chosen > 1 so the
	/// dust case (`0 < remainder < ED`) is expressible with integer balances.
	const PGAS_MIN_BALANCE: u64 = 100;

	fn new_conv_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<ConvTest>::default().build_storage().unwrap();
		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
			assert!(Assets::force_create(
				RuntimeOrigin::root(),
				PgasId::get(),
				1,
				true,
				PGAS_MIN_BALANCE,
			)
			.is_ok());
			assert!(Assets::mint_into(PgasId::get(), &1u64, 1_000_000_000).is_ok());
		});
		ext
	}

	fn dispatch_refund_call_with_tip(tip: u64) -> sp_runtime::DispatchResult {
		let call = RuntimeCall::Refund(refund_test_pallet::Call::half_weight_refund {});
		let info = call.get_dispatch_info();
		let extension = ChargeAssetTxPayment::<ConvTest>::from(tip, Some(PgasId::get()));
		extension
			.dispatch_transaction(RuntimeOrigin::signed(1), call, &info, 0, 0)
			.expect("transaction should be valid")
			.map(|_| ())
			.map_err(|e| e.error)
	}

	/// Expected fee the user pays after the half-weight refund, computed using the same helpers
	/// the extension's `post_dispatch` invokes.
	fn expected_actual_fee(tip: u64) -> u64 {
		let call = RuntimeCall::Refund(refund_test_pallet::Call::half_weight_refund {});
		let info = call.get_dispatch_info();
		let post = frame_support::dispatch::PostDispatchInfo {
			actual_weight: Some(refund_test_pallet::ACTUAL_WEIGHT),
			pays_fee: frame_support::dispatch::Pays::Yes,
		};
		pallet_transaction_payment::Pallet::<ConvTest>::compute_actual_fee(0, &info, &post, tip)
	}

	#[test]
	fn full_dispatch_burns_fee_for_actual_weight_only() {
		new_conv_test_ext().execute_with(|| {
			let initial_balance = Assets::balance(PgasId::get(), 1u64);
			let initial_issuance = Assets::total_issuance(PgasId::get());
			let expected = expected_actual_fee(0);

			assert!(dispatch_refund_call_with_tip(0).is_ok());

			let paid = initial_balance - Assets::balance(PgasId::get(), 1u64);
			let burned = initial_issuance - Assets::total_issuance(PgasId::get());

			// Half the declared weight was consumed, so the wrapper's `correct_and_deposit_fee`
			// splits the pre-dispatch withdrawal at the smaller actual fee, refunds the rest, and
			// drops (burns) only the actual-fee credit.
			assert_eq!(paid, expected);
			assert_eq!(burned, expected);
		});
	}

	#[test]
	fn full_dispatch_with_tip_burns_fee_plus_tip() {
		new_conv_test_ext().execute_with(|| {
			let initial_balance = Assets::balance(PgasId::get(), 1u64);
			let initial_issuance = Assets::total_issuance(PgasId::get());
			let tip = 77u64;
			let expected = expected_actual_fee(tip);

			assert!(dispatch_refund_call_with_tip(tip).is_ok());

			let paid = initial_balance - Assets::balance(PgasId::get(), 1u64);
			let burned = initial_issuance - Assets::total_issuance(PgasId::get());

			assert_eq!(paid, expected);
			assert_eq!(burned, expected);
			assert_eq!(expected, expected_actual_fee(0) + tip);
		});
	}

	/// Dispatch the no-refund call paying from `payer` with the given tip. Returns the
	/// pre-dispatch `full_fee` (what the extension withdraws up front, and with `no_refund_call`
	/// also what the user actually pays).
	fn dispatch_no_refund_as(payer: u64, tip: u64) -> u64 {
		let call = RuntimeCall::Refund(refund_test_pallet::Call::no_refund_call {});
		let info = call.get_dispatch_info();
		let full_fee = pallet_transaction_payment::Pallet::<ConvTest>::compute_fee(0, &info, tip);
		let extension = ChargeAssetTxPayment::<ConvTest>::from(tip, Some(PgasId::get()));
		extension
			.dispatch_transaction(RuntimeOrigin::signed(payer), call, &info, 0, 0)
			.expect("transaction should be valid")
			.expect("dispatch should succeed");
		full_fee
	}

	#[test]
	fn full_dispatch_allows_fee_payment_to_reap_pgas_account() {
		new_conv_test_ext().execute_with(|| {
			// Phase 1: exact-fee payer — balance drops to zero and the sufficient-asset account
			// is reaped. Phase 2 (below) covers the sub-ED dust case on a separate account.
			let payer = 2u64;
			let tip = 0u64;
			// Pre-compute the fee so we can seed the account with exactly that amount.
			let preview = {
				let call = RuntimeCall::Refund(refund_test_pallet::Call::no_refund_call {});
				let info = call.get_dispatch_info();
				pallet_transaction_payment::Pallet::<ConvTest>::compute_fee(0, &info, tip)
			};
			// The fee must be at least the ED, otherwise `mint_into` rejects the seed as
			// below-ED and the test premise doesn't hold.
			assert!(preview >= PGAS_MIN_BALANCE);

			assert!(Assets::mint_into(PgasId::get(), &payer, preview).is_ok());
			let initial_issuance = Assets::total_issuance(PgasId::get());
			assert_eq!(Assets::balance(PgasId::get(), payer), preview);

			let full_fee = dispatch_no_refund_as(payer, tip);
			assert_eq!(full_fee, preview);

			// The sufficient-asset account is reaped: balance is zero and the account record is
			// gone.
			assert_eq!(Assets::balance(PgasId::get(), payer), 0);
			assert!(!pallet_assets::Account::<ConvTest>::contains_key(PgasId::get(), payer));
			// Full fee was burned — issuance dropped by exactly the pre-dispatch fee.
			assert_eq!(Assets::total_issuance(PgasId::get()), initial_issuance - full_fee);
		});
	}

	#[test]
	fn full_dispatch_reaps_and_dusts_account_when_remainder_is_sub_ed() {
		new_conv_test_ext().execute_with(|| {
			// Seed a fresh payer with `fee + dust` where `0 < dust < ED`. Paying the fee leaves
			// the account below the existential deposit, so — under `Expendable` preservation on
			// a sufficient asset — pallet-assets reaps the account and destroys the dust.
			let payer = 3u64;
			let tip = 0u64;
			let preview = {
				let call = RuntimeCall::Refund(refund_test_pallet::Call::no_refund_call {});
				let info = call.get_dispatch_info();
				pallet_transaction_payment::Pallet::<ConvTest>::compute_fee(0, &info, tip)
			};
			let dust = PGAS_MIN_BALANCE - 1;
			let seeded = preview + dust;

			assert!(Assets::mint_into(PgasId::get(), &payer, seeded).is_ok());
			let initial_issuance = Assets::total_issuance(PgasId::get());
			assert_eq!(Assets::balance(PgasId::get(), payer), seeded);

			let full_fee = dispatch_no_refund_as(payer, tip);
			assert_eq!(full_fee, preview);

			// Account is reaped — the leftover `dust < ED` cannot keep a sufficient-asset
			// account alive.
			assert_eq!(Assets::balance(PgasId::get(), payer), 0);
			assert!(!pallet_assets::Account::<ConvTest>::contains_key(PgasId::get(), payer));
			// The fee is burned via `drop(fee_credit)` and the `dust` is destroyed by the
			// reap path — both come out of total issuance.
			assert_eq!(Assets::total_issuance(PgasId::get()), initial_issuance - full_fee - dust,);
		});
	}
}
