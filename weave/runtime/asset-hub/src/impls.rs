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

use crate::*;

// TODO: move implementations to the polkadot-sdk.
pub mod tx_payment {
	use super::*;
	use core::marker::PhantomData;
	use frame_support::{
		ensure,
		pallet_prelude::{InvalidTransaction, TransactionValidityError},
		traits::{
			fungibles::{Balanced as FungiblesBalanced, Inspect as FungiblesInspect},
			tokens::{Fortitude, Precision, Preservation},
			Defensive, OnUnbalanced, SameOrOther,
		},
	};
	use pallet_asset_conversion::{Pallet as AssetConversion, SwapCredit};
	use pallet_asset_conversion_tx_payment::OnChargeAssetTransaction;
	use pallet_transaction_payment::OnChargeTransaction;
	use sp_core::Get;
	use sp_runtime::{
		traits::{DispatchInfoOf, PostDispatchInfoOf, Zero},
		Saturating,
	};

	/// Implements [`OnChargeTransaction`] for [`pallet_transaction_payment`], where the asset class
	/// used to pay the fee is defined with the `A` type parameter (eg. KSM location) and accessed
	/// via the type implementing the [`frame_support::traits::fungibles`] trait.
	///
	/// This implementation with the `fungibles` trait is necessary to set up
	/// [`pallet_asset_conversion_tx_payment`] with the [`SwapCreditAdapter`] type. For both types,
	/// the credit types they handle must be the same, therefore they must be credits of
	/// `fungibles`.
	pub struct FungiblesAdapter<F, A, OU>(PhantomData<(F, A, OU)>);

	impl<T, F, A, OU> OnChargeTransaction<T> for FungiblesAdapter<F, A, OU>
	where
		T: pallet_transaction_payment::Config,
		F: fungibles::Balanced<T::AccountId>,
		A: Get<F::AssetId>,
		OU: OnUnbalanced<fungibles::Credit<T::AccountId, F>>,
	{
		type LiquidityInfo = Option<fungibles::Credit<T::AccountId, F>>;
		type Balance = F::Balance;

		fn withdraw_fee(
			who: &<T>::AccountId,
			_call: &<T>::RuntimeCall,
			_dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
			fee: Self::Balance,
			_tip: Self::Balance,
		) -> Result<Self::LiquidityInfo, TransactionValidityError> {
			if fee.is_zero() {
				return Ok(None);
			}

			match F::withdraw(
				A::get(),
				who,
				fee,
				Precision::Exact,
				Preservation::Preserve,
				Fortitude::Polite,
			) {
				Ok(imbalance) => Ok(Some(imbalance)),
				Err(_) => Err(InvalidTransaction::Payment.into()),
			}
		}

		fn correct_and_deposit_fee(
			who: &<T>::AccountId,
			_dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
			_post_info: &PostDispatchInfoOf<<T>::RuntimeCall>,
			corrected_fee: Self::Balance,
			_tip: Self::Balance,
			already_withdrawn: Self::LiquidityInfo,
		) -> Result<(), TransactionValidityError> {
			let Some(paid) = already_withdrawn else {
				return Ok(());
			};
			// Make sure the credit is in desired asset id.
			ensure!(paid.asset() == A::get(), InvalidTransaction::Payment);
			// Calculate how much refund we should return.
			let refund_amount = paid.peek().saturating_sub(corrected_fee);
			// Refund to the the account that paid the fees if it was not removed by the
			// dispatched function. If fails for any reason (eg. ED requirement is not met) no
			// refund given.
			let refund_debt =
				if F::total_balance(A::get(), who).is_zero() || refund_amount.is_zero() {
					fungibles::Debt::<T::AccountId, F>::zero(A::get())
				} else {
					F::deposit(A::get(), who, refund_amount, Precision::BestEffort)
						.unwrap_or_else(|_| fungibles::Debt::<T::AccountId, F>::zero(A::get()))
				};
			// Merge the imbalance caused by paying the fees and refunding parts of it again.
			let adjusted_paid: fungibles::Credit<T::AccountId, F> =
				match paid.offset(refund_debt).defensive_proof("credits should be identical") {
					Ok(SameOrOther::Same(credit)) => credit,
					// Paid amount is fully refunded.
					Ok(SameOrOther::None) => fungibles::Credit::<T::AccountId, F>::zero(A::get()),
					// Should never fail as at this point the asset id is always valid and the
					// refund amount is not greater than paid amount.
					_ => return Err(InvalidTransaction::Payment.into()),
				};
			// No separation for simplicity.
			// In our case the fees and the tips are deposited to the same pot.
			// We cannot call [`OnUnbalanced::on_unbalanceds`] since fungibles credit does not
			// implement `Imbalanced` trait.
			OU::on_unbalanced(adjusted_paid);
			Ok(())
		}
	}

	type LiquidityInfoOf<T> =
		<<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<
			T,
		>>::LiquidityInfo;

	/// Implements [`OnChargeAssetTransaction`] for [`pallet_asset_conversion_tx_payment`], where
	/// the asset class used to pay the fee is defined with the `A` type parameter (eg. DOT
	/// location) and accessed via the type implementing the [`frame_support::traits::fungibles`]
	/// trait.
	pub struct SwapCreditAdapter<A, S>(PhantomData<(A, S)>);
	impl<A, S, T> OnChargeAssetTransaction<T> for SwapCreditAdapter<A, S>
	where
		A: Get<S::AssetKind>,
		S: SwapCredit<
			T::AccountId,
			Balance = T::Balance,
			AssetKind = T::AssetKind,
			Credit = fungibles::Credit<T::AccountId, T::Assets>,
		>,

		T: pallet_asset_conversion_tx_payment::Config,
		T::Fungibles:
			fungibles::Inspect<T::AccountId, Balance = T::Balance, AssetId = T::AssetKind>,
		T::OnChargeTransaction:
			OnChargeTransaction<T, Balance = T::Balance, LiquidityInfo = Option<S::Credit>>,
	{
		type AssetId = T::AssetKind;
		type Balance = T::Balance;
		type LiquidityInfo = T::Balance;

		fn withdraw_fee(
			who: &<T>::AccountId,
			_call: &<T>::RuntimeCall,
			_dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
			asset_id: Self::AssetId,
			fee: Self::Balance,
			_tip: Self::Balance,
		) -> Result<(LiquidityInfoOf<T>, Self::LiquidityInfo, T::Balance), TransactionValidityError>
		{
			let asset_fee = AssetConversion::<T>::quote_price_tokens_for_exact_tokens(
				asset_id.clone(),
				A::get(),
				fee,
				true,
			)
			.ok_or(InvalidTransaction::Payment)?;

			let asset_fee_credit = T::Assets::withdraw(
				asset_id.clone(),
				who,
				asset_fee,
				Precision::Exact,
				Preservation::Preserve,
				Fortitude::Polite,
			)
			.map_err(|_| TransactionValidityError::from(InvalidTransaction::Payment))?;

			let (fee_credit, change) = match S::swap_tokens_for_exact_tokens(
				vec![asset_id, A::get()],
				asset_fee_credit,
				fee,
			) {
				Ok((fee_credit, change)) => (fee_credit, change),
				Err((credit_in, _)) => {
					// The swap should not error since the price quote was successful.
					let _ = T::Assets::resolve(who, credit_in).defensive();
					return Err(InvalidTransaction::Payment.into());
				},
			};

			// Should be always zero since the exact price was quoted before.
			ensure!(change.peek().is_zero(), InvalidTransaction::Payment);

			Ok((Some(fee_credit), fee, asset_fee))
		}
		fn correct_and_deposit_fee(
			who: &<T>::AccountId,
			dispatch_info: &DispatchInfoOf<<T>::RuntimeCall>,
			post_info: &PostDispatchInfoOf<<T>::RuntimeCall>,
			corrected_fee: Self::Balance,
			tip: Self::Balance,
			fee_paid: LiquidityInfoOf<T>,
			_received_exchanged: Self::LiquidityInfo,
			asset_id: Self::AssetId,
			initial_asset_consumed: T::Balance,
		) -> Result<T::Balance, TransactionValidityError> {
			let Some(fee_paid) = fee_paid else {
				return Ok(Zero::zero());
			};
			// Try to refund if the fee paid is more than the corrected fee and the account was not
			// removed by the dispatched function.
			let (fee, fee_in_asset) = if fee_paid.peek() > corrected_fee &&
				!T::Assets::total_balance(asset_id.clone(), who).is_zero()
			{
				let refund_amount = fee_paid.peek().saturating_sub(corrected_fee);
				// Check if the refund amount can be swapped back into the asset used by `who` for
				// fee payment.
				let refund_asset_amount =
					AssetConversion::<T>::quote_price_exact_tokens_for_tokens(
						A::get(),
						asset_id.clone(),
						refund_amount,
						true,
					)
					// No refund given if it cannot be swapped back.
					.unwrap_or(Zero::zero());

				// Deposit the refund before the swap to ensure it can be processed.
				let debt = match T::Assets::deposit(
					asset_id.clone(),
					who,
					refund_asset_amount,
					Precision::BestEffort,
				) {
					Ok(debt) => debt,
					// No refund given since it cannot be deposited.
					Err(_) => fungibles::Debt::<T::AccountId, T::Assets>::zero(asset_id.clone()),
				};

				if debt.peek().is_zero() {
					// No refund given.
					(fee_paid, initial_asset_consumed)
				} else {
					let (refund, fee_paid) = fee_paid.split(refund_amount);
					match S::swap_exact_tokens_for_tokens(
						vec![A::get(), asset_id],
						refund,
						Some(refund_asset_amount),
					) {
						Ok(refund_asset) => {
							match refund_asset.offset(debt) {
								Ok(SameOrOther::None) => {},
								// This arm should never be reached, as the  amount of `debt` is
								// expected to be exactly equal to the amount of `refund_asset`
								// credit.
								_ => return Err(InvalidTransaction::Payment.into()),
							};
							(fee_paid, initial_asset_consumed.saturating_sub(refund_asset_amount))
						},
						// The error should not occur since swap was quoted before.
						Err((refund, _)) => {
							match T::Assets::settle(who, debt, Preservation::Expendable) {
								Ok(dust) => {
									ensure!(dust.peek().is_zero(), InvalidTransaction::Payment)
								},
								// The error should not occur as the `debt` was just withdrawn
								// above.
								Err(_) => return Err(InvalidTransaction::Payment.into()),
							};
							let fee_paid = fee_paid.merge(refund).map_err(|_| {
								// The error should never occur since `fee_paid` and `refund` are
								// credits of the same asset.
								TransactionValidityError::from(InvalidTransaction::Payment)
							})?;
							(fee_paid, initial_asset_consumed)
						},
					}
				}
			} else {
				(fee_paid, initial_asset_consumed)
			};

			// Refund is already processed.
			let corrected_fee = fee.peek();
			// Deposit fee.
			T::OnChargeTransaction::correct_and_deposit_fee(
				who,
				dispatch_info,
				post_info,
				corrected_fee,
				tip,
				Some(fee),
			)
			.map(|_| fee_in_asset)
		}
	}
}
