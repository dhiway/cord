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

use core::marker::PhantomData;
use frame_support::traits::fungibles;

/// A type to define a combined fungibles implementation from an asset and a holder.
///
/// The asset and the holder must refer to the same assets and balances and be compatible.
/// This type can be used the holder doesn't implement Mutate or similar traits.
pub struct CombineAssetsWithHolder<A, H>(PhantomData<(A, H)>);

impl<AccountId, A, H> fungibles::Inspect<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Inspect<AccountId>,
{
	type AssetId = A::AssetId;
	type Balance = A::Balance;
	fn balance(asset: Self::AssetId, who: &AccountId) -> Self::Balance {
		A::balance(asset, who)
	}
	fn can_deposit(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
		provenance: frame_support::traits::tokens::Provenance,
	) -> frame_support::traits::tokens::DepositConsequence {
		A::can_deposit(asset, who, amount, provenance)
	}
	fn can_withdraw(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> frame_support::traits::tokens::WithdrawConsequence<Self::Balance> {
		A::can_withdraw(asset, who, amount)
	}
	fn asset_exists(asset: Self::AssetId) -> bool {
		A::asset_exists(asset)
	}
	fn total_balance(asset: Self::AssetId, who: &AccountId) -> Self::Balance {
		A::total_balance(asset, who)
	}
	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		A::total_issuance(asset)
	}
	fn active_issuance(asset: Self::AssetId) -> Self::Balance {
		A::active_issuance(asset)
	}
	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		A::minimum_balance(asset)
	}
	fn reducible_balance(
		asset: Self::AssetId,
		who: &AccountId,
		preservation: frame_support::traits::tokens::Preservation,
		force: frame_support::traits::tokens::Fortitude,
	) -> Self::Balance {
		A::reducible_balance(asset, who, preservation, force)
	}
}

impl<AccountId, A, H> fungibles::Mutate<AccountId> for CombineAssetsWithHolder<A, H>
where
	AccountId: Eq,
	A: fungibles::Mutate<AccountId>,
{
	fn shelve(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::shelve(asset, who, amount)
	}
	fn restore(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::restore(asset, who, amount)
	}
	fn transfer(
		asset: Self::AssetId,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Balance,
		preservation: frame_support::traits::tokens::Preservation,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::transfer(asset, source, dest, amount, preservation)
	}
	fn mint_into(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::mint_into(asset, who, amount)
	}
	fn burn_from(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
		preservation: frame_support::traits::tokens::Preservation,
		precision: frame_support::traits::tokens::Precision,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::burn_from(asset, who, amount, preservation, precision, force)
	}
	fn set_balance(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) -> Self::Balance {
		A::set_balance(asset, who, amount)
	}
	fn done_shelve(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_shelve(asset, who, amount)
	}
	fn done_restore(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_restore(asset, who, amount)
	}
	fn done_transfer(
		asset: Self::AssetId,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Balance,
	) {
		A::done_transfer(asset, source, dest, amount)
	}
	fn done_mint_into(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_mint_into(asset, who, amount)
	}
	fn done_burn_from(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_burn_from(asset, who, amount)
	}
}

impl<AccountId, A, H> fungibles::Unbalanced<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Unbalanced<AccountId>,
{
	fn deactivate(asset: Self::AssetId, b: Self::Balance) {
		A::deactivate(asset, b)
	}
	fn reactivate(asset: Self::AssetId, b: Self::Balance) {
		A::reactivate(asset, b)
	}
	fn handle_dust(dust: fungibles::Dust<AccountId, Self>) {
		A::handle_dust(fungibles::Dust(dust.0, dust.1))
	}
	fn write_balance(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> Result<Option<Self::Balance>, sp_runtime::DispatchError> {
		A::write_balance(asset, who, amount)
	}
	fn handle_raw_dust(asset: Self::AssetId, amount: Self::Balance) {
		A::handle_raw_dust(asset, amount)
	}
	fn decrease_balance(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
		preservation: frame_support::traits::tokens::Preservation,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::decrease_balance(asset, who, amount, precision, preservation, force)
	}
	fn increase_balance(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		A::increase_balance(asset, who, amount, precision)
	}
	fn set_total_issuance(asset: Self::AssetId, amount: Self::Balance) {
		A::set_total_issuance(asset, amount)
	}
}

impl<AccountId, A, H> fungibles::InspectHold<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Inspect<AccountId>,
	H: fungibles::InspectHold<AccountId, AssetId = A::AssetId, Balance = A::Balance>,
{
	type Reason = H::Reason;
	fn can_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) -> bool {
		H::can_hold(asset, reason, who, amount)
	}
	fn hold_available(asset: Self::AssetId, reason: &Self::Reason, who: &AccountId) -> bool {
		H::hold_available(asset, reason, who)
	}
	fn balance_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
	) -> Self::Balance {
		H::balance_on_hold(asset, reason, who)
	}
	fn ensure_can_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		H::ensure_can_hold(asset, reason, who, amount)
	}
	fn total_balance_on_hold(asset: Self::AssetId, who: &AccountId) -> Self::Balance {
		H::total_balance_on_hold(asset, who)
	}
	fn reducible_total_balance_on_hold(
		asset: Self::AssetId,
		who: &AccountId,
		force: frame_support::traits::tokens::Fortitude,
	) -> Self::Balance {
		H::reducible_total_balance_on_hold(asset, who, force)
	}
}

impl<AccountId, A, H> fungibles::MutateHold<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Inspect<AccountId> + fungibles::Unbalanced<AccountId>,
	H: fungibles::MutateHold<AccountId, AssetId = A::AssetId, Balance = A::Balance>,
{
	fn hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		H::hold(asset, reason, who, amount)
	}
	fn release(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::release(asset, reason, who, amount, precision)
	}
	fn burn_held(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::burn_held(asset, reason, who, amount, precision, force)
	}
	fn done_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) {
		H::done_hold(asset, reason, who, amount)
	}
	fn done_release(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) {
		H::done_release(asset, reason, who, amount)
	}
	fn burn_all_held(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		precision: frame_support::traits::tokens::Precision,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::burn_all_held(asset, reason, who, precision, force)
	}
	fn done_burn_held(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) {
		H::done_burn_held(asset, reason, who, amount)
	}
	fn transfer_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
		mode: frame_support::traits::tokens::Restriction,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::transfer_on_hold(asset, reason, source, dest, amount, precision, mode, force)
	}
	fn transfer_and_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
		expendability: frame_support::traits::tokens::Preservation,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::transfer_and_hold(asset, reason, source, dest, amount, precision, expendability, force)
	}
	fn done_transfer_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Balance,
	) {
		H::done_transfer_on_hold(asset, reason, source, dest, amount)
	}
	fn done_transfer_and_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		source: &AccountId,
		dest: &AccountId,
		transferred: Self::Balance,
	) {
		H::done_transfer_and_hold(asset, reason, source, dest, transferred)
	}
}

impl<AccountId, A, H> fungibles::UnbalancedHold<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Inspect<AccountId>,
	H: fungibles::UnbalancedHold<AccountId, AssetId = A::AssetId, Balance = A::Balance>,
{
	fn set_balance_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
	) -> sp_runtime::DispatchResult {
		H::set_balance_on_hold(asset, reason, who, amount)
	}
	fn decrease_balance_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::decrease_balance_on_hold(asset, reason, who, amount, precision)
	}
	fn increase_balance_on_hold(
		asset: Self::AssetId,
		reason: &Self::Reason,
		who: &AccountId,
		amount: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
	) -> Result<Self::Balance, sp_runtime::DispatchError> {
		H::increase_balance_on_hold(asset, reason, who, amount, precision)
	}
}

impl<AccountId, A, H> fungibles::Create<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Create<AccountId>,
{
	fn create(
		id: Self::AssetId,
		admin: AccountId,
		is_sufficient: bool,
		min_balance: Self::Balance,
	) -> sp_runtime::DispatchResult {
		A::create(id, admin, is_sufficient, min_balance)
	}
}

impl<AccountId, A, H> fungibles::Balanced<AccountId> for CombineAssetsWithHolder<A, H>
where
	A: fungibles::Balanced<AccountId>,
{
	type OnDropDebt = A::OnDropDebt;
	type OnDropCredit = A::OnDropCredit;
	fn pair(
		asset: Self::AssetId,
		amount: Self::Balance,
	) -> Result<
		(fungibles::Debt<AccountId, Self>, fungibles::Credit<AccountId, Self>),
		sp_runtime::DispatchError,
	> {
		A::pair(asset, amount)
	}
	fn issue(asset: Self::AssetId, amount: Self::Balance) -> fungibles::Credit<AccountId, Self> {
		A::issue(asset, amount)
	}
	fn settle(
		who: &AccountId,
		debt: fungibles::Debt<AccountId, Self>,
		preservation: frame_support::traits::tokens::Preservation,
	) -> Result<fungibles::Credit<AccountId, Self>, fungibles::Debt<AccountId, Self>> {
		A::settle(who, debt, preservation)
	}
	fn rescind(asset: Self::AssetId, amount: Self::Balance) -> fungibles::Debt<AccountId, Self> {
		A::rescind(asset, amount)
	}
	fn deposit(
		asset: Self::AssetId,
		who: &AccountId,
		value: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
	) -> Result<fungibles::Debt<AccountId, Self>, sp_runtime::DispatchError> {
		A::deposit(asset, who, value, precision)
	}
	fn resolve(
		who: &AccountId,
		credit: fungibles::Credit<AccountId, Self>,
	) -> Result<(), fungibles::Credit<AccountId, Self>> {
		A::resolve(who, credit)
	}
	fn withdraw(
		asset: Self::AssetId,
		who: &AccountId,
		value: Self::Balance,
		precision: frame_support::traits::tokens::Precision,
		preservation: frame_support::traits::tokens::Preservation,
		force: frame_support::traits::tokens::Fortitude,
	) -> Result<fungibles::Credit<AccountId, Self>, sp_runtime::DispatchError> {
		A::withdraw(asset, who, value, precision, preservation, force)
	}
	fn done_issue(asset: Self::AssetId, amount: Self::Balance) {
		A::done_issue(asset, amount)
	}
	fn done_rescind(asset: Self::AssetId, amount: Self::Balance) {
		A::done_rescind(asset, amount)
	}
	fn done_deposit(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_deposit(asset, who, amount)
	}
	fn done_withdraw(asset: Self::AssetId, who: &AccountId, amount: Self::Balance) {
		A::done_withdraw(asset, who, amount)
	}
}
