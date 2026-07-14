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

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{vec, vec::Vec};
use codec::{Decode, Encode};
use polkadot_sdk_frame::{
	deps::frame_support,
	prelude::{
		fungible::{
			hold::DoneSlash, Balanced, BalancedHold, Dust, Inspect, InspectHold, Mutate,
			MutateHold, Unbalanced, UnbalancedHold,
		},
		DepositConsequence, DispatchError, DispatchResult, Fortitude, Preservation, Provenance,
		WithdrawConsequence, Zero,
	},
	token::{BalanceStatus, WithdrawReasons},
	traits::{
		tokens::imbalance::TryMerge, Currency, Imbalance, ReservableCurrency, SameOrOther,
		SignedImbalance, TryDrop,
	},
};
use scale_info::TypeInfo;

/// Resolve an RFC-78 metadata implicit through the extension's production implementation.
///
/// Keeping this tiny adapter in a no-std common crate lets runtimes and isolated no-hash tests use
/// exactly the same resolution path without introducing a dependency on the Commons runtime.
pub fn resolve_metadata_implicit<Call, Extension>(
	extension: &Extension,
) -> Result<
	Option<[u8; 32]>,
	polkadot_sdk_frame::deps::sp_runtime::transaction_validity::TransactionValidityError,
>
where
	Call: polkadot_sdk_frame::deps::sp_runtime::traits::Dispatchable,
	Extension: polkadot_sdk_frame::deps::sp_runtime::traits::TransactionExtension<
		Call,
		Implicit = Option<[u8; 32]>,
	>,
{
	polkadot_sdk_frame::deps::sp_runtime::traits::TransactionExtension::implicit(extension)
}

/// Fungible currency implementation that does not support any balance operations.
/// Works only with zero balances.
///
/// Note: This is a workaround to satisfy the `pallet-session::Config::Currency` and
/// `pallet-proxy::Config::Currency` trait requirements.
pub struct NoCurrency<AccountId, HoldReason = ()>(
	core::marker::PhantomData<(AccountId, HoldReason)>,
);

impl<AccountId, HoldReason> Balanced<AccountId> for NoCurrency<AccountId, HoldReason>
where
	HoldReason: Encode + Decode + TypeInfo + 'static,
{
	type OnDropDebt = ();
	type OnDropCredit = ();
}

impl<AccountId, HoldReason> BalancedHold<AccountId> for NoCurrency<AccountId, HoldReason> where
	HoldReason: Encode + Decode + TypeInfo + 'static
{
}

impl<AccountId, HoldReason, Balance> DoneSlash<HoldReason, AccountId, Balance>
	for NoCurrency<AccountId, HoldReason>
where
	Balance: Sized,
	HoldReason: Encode + Decode + TypeInfo + 'static,
{
}

impl<AccountId, HoldReason: Encode + Decode + TypeInfo + 'static> Inspect<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
	type Balance = u128;

	fn total_issuance() -> Self::Balance {
		Zero::zero()
	}

	fn minimum_balance() -> Self::Balance {
		Zero::zero()
	}

	fn total_balance(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}

	fn balance(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}

	fn reducible_balance(
		_who: &AccountId,
		_preservation: Preservation,
		_force: Fortitude,
	) -> Self::Balance {
		Zero::zero()
	}

	fn can_deposit(
		_who: &AccountId,
		_amount: Self::Balance,
		_provenance: Provenance,
	) -> DepositConsequence {
		DepositConsequence::Success
	}

	fn can_withdraw(
		_who: &AccountId,
		_amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		WithdrawConsequence::Success
	}
}

impl<AccountId, HoldReason: Encode + Decode + TypeInfo + 'static> Unbalanced<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
	fn handle_dust(_dust: Dust<AccountId, Self>) {}

	fn write_balance(
		_who: &AccountId,
		_amount: Self::Balance,
	) -> Result<Option<Self::Balance>, DispatchError> {
		Ok(None)
	}

	fn set_total_issuance(_amount: Self::Balance) {}
}

impl<AccountId, HoldReason: Encode + Decode + TypeInfo + 'static> InspectHold<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
	type Reason = HoldReason;

	fn total_balance_on_hold(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}

	fn balance_on_hold(_reason: &Self::Reason, _who: &AccountId) -> Self::Balance {
		Zero::zero()
	}
}

impl<AccountId, HoldReason: Encode + Decode + TypeInfo + 'static> UnbalancedHold<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
	fn set_balance_on_hold(
		_reason: &Self::Reason,
		_who: &AccountId,
		_amount: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId, HoldReason: Encode + Decode + TypeInfo + 'static> MutateHold<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
}

impl<AccountId: Eq, HoldReason: Encode + Decode + TypeInfo + 'static> Mutate<AccountId>
	for NoCurrency<AccountId, HoldReason>
{
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZeroImbalance<Balance> {
	_phantom: core::marker::PhantomData<Balance>,
}

impl<Balance: Default + Copy> Imbalance<Balance> for ZeroImbalance<Balance> {
	type Opposite = Self;

	fn zero() -> Self {
		Self { _phantom: core::marker::PhantomData }
	}

	fn split(self, _amount: Balance) -> (Self, Self) {
		(Self::zero(), Self::zero())
	}

	fn merge(self, _other: Self) -> Self {
		Self::zero()
	}

	fn subsume(&mut self, _other: Self) {}

	fn offset(self, _other: Self::Opposite) -> SameOrOther<Self, Self::Opposite> {
		SameOrOther::None
	}

	fn peek(&self) -> Balance {
		Balance::default()
	}

	fn drop_zero(self) -> Result<(), Self> {
		Ok(())
	}

	fn extract(&mut self, _amount: Balance) -> Self {
		Self::zero()
	}
}

impl<Balance: Default + Copy> TryDrop for ZeroImbalance<Balance> {
	fn try_drop(self) -> Result<(), Self> {
		Ok(())
	}
}

impl<Balance: Default + Copy> TryMerge for ZeroImbalance<Balance> {
	fn try_merge(self, _other: Self) -> Result<Self, (Self, Self)> {
		Ok(Self::zero())
	}
}

impl<AccountId, HoldReason> Currency<AccountId> for NoCurrency<AccountId, HoldReason> {
	type Balance = u128;
	type PositiveImbalance = ZeroImbalance<u128>;
	type NegativeImbalance = ZeroImbalance<u128>;

	fn total_balance(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}

	fn can_slash(_who: &AccountId, _value: Self::Balance) -> bool {
		false
	}

	fn total_issuance() -> Self::Balance {
		Zero::zero()
	}

	fn minimum_balance() -> Self::Balance {
		Zero::zero()
	}

	fn burn(_amount: Self::Balance) -> Self::PositiveImbalance {
		ZeroImbalance::zero()
	}

	fn issue(_amount: Self::Balance) -> Self::NegativeImbalance {
		ZeroImbalance::zero()
	}

	fn free_balance(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}

	fn ensure_can_withdraw(
		_who: &AccountId,
		_amount: Self::Balance,
		_reasons: WithdrawReasons,
		_new_balance: Self::Balance,
	) -> frame_support::pallet_prelude::DispatchResult {
		Ok(())
	}

	fn transfer(
		_source: &AccountId,
		_dest: &AccountId,
		_value: Self::Balance,
		_existence_requirement: frame_support::traits::ExistenceRequirement,
	) -> frame_support::pallet_prelude::DispatchResult {
		Ok(())
	}

	fn slash(_who: &AccountId, _value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
		(ZeroImbalance::zero(), Zero::zero())
	}

	fn deposit_into_existing(
		_who: &AccountId,
		_value: Self::Balance,
	) -> Result<Self::PositiveImbalance, frame_support::pallet_prelude::DispatchError> {
		Ok(ZeroImbalance::zero())
	}

	fn deposit_creating(_who: &AccountId, _value: Self::Balance) -> Self::PositiveImbalance {
		ZeroImbalance::zero()
	}

	fn withdraw(
		_who: &AccountId,
		_value: Self::Balance,
		_reasons: WithdrawReasons,
		_liveness: frame_support::traits::ExistenceRequirement,
	) -> Result<Self::NegativeImbalance, frame_support::pallet_prelude::DispatchError> {
		Ok(ZeroImbalance::zero())
	}

	fn make_free_balance_be(
		_who: &AccountId,
		_balance: Self::Balance,
	) -> frame_support::traits::SignedImbalance<Self::Balance, Self::PositiveImbalance> {
		SignedImbalance::zero()
	}
}

impl<AccountId, HoldReason> ReservableCurrency<AccountId> for NoCurrency<AccountId, HoldReason> {
	fn can_reserve(_who: &AccountId, _value: Self::Balance) -> bool {
		true
	}

	fn reserve(_who: &AccountId, _value: Self::Balance) -> DispatchResult {
		Ok(())
	}

	fn unreserve(_who: &AccountId, _value: Self::Balance) -> Self::Balance {
		Zero::zero()
	}

	fn slash_reserved(_who: &AccountId, _value: Self::Balance) -> (ZeroImbalance<u128>, u128) {
		(ZeroImbalance::zero(), Zero::zero())
	}

	fn repatriate_reserved(
		_slashed: &AccountId,
		_beneficiary: &AccountId,
		_value: Self::Balance,
		_status: BalanceStatus,
	) -> Result<Self::Balance, DispatchError> {
		Ok(Zero::zero())
	}

	fn reserved_balance(_who: &AccountId) -> Self::Balance {
		Zero::zero()
	}
}

/// Inspect a utility call for wrapper semantics: returns the inner calls if the call
/// is a wrapper variant, `None` otherwise.
pub fn inspect_utility_wrapper<T: pallet_utility::Config>(
	call: &pallet_utility::Call<T>,
) -> Option<Vec<&<T as pallet_utility::Config>::RuntimeCall>> {
	let inner = utility_inner_calls(call);
	if inner.is_empty() {
		return None;
	}
	Some(inner)
}

/// Inspect a sudo call for wrapper semantics: returns inner calls.
pub fn inspect_sudo_wrapper<T: pallet_sudo::Config>(
	call: &pallet_sudo::Call<T>,
) -> Option<Vec<&<T as pallet_sudo::Config>::RuntimeCall>> {
	let inner = sudo_inner_calls(call);
	if inner.is_empty() {
		return None;
	}
	Some(inner)
}

/// Inspect a proxy call for wrapper semantics: returns inner calls.
pub fn inspect_proxy_wrapper<T: pallet_proxy::Config>(
	call: &pallet_proxy::Call<T>,
) -> Option<Vec<&<T as pallet_proxy::Config>::RuntimeCall>> {
	let inner = proxy_inner_calls(call);
	if inner.is_empty() {
		return None;
	}
	Some(inner)
}

/// Extract inner calls from a utility call variant.
pub fn utility_inner_calls<T: pallet_utility::Config>(
	call: &pallet_utility::Call<T>,
) -> Vec<&<T as pallet_utility::Config>::RuntimeCall> {
	match call {
		pallet_utility::Call::batch { calls }
		| pallet_utility::Call::batch_all { calls }
		| pallet_utility::Call::force_batch { calls } => calls.iter().collect(),
		pallet_utility::Call::as_derivative { call, .. }
		| pallet_utility::Call::dispatch_as { call, .. }
		| pallet_utility::Call::with_weight { call, .. }
		| pallet_utility::Call::dispatch_as_fallible { call, .. } => vec![call.as_ref()],
		// `if_else` executes only ONE branch (fallback runs only if main fails),
		// but we return both so that authorization is validated and consumed for
		// both paths during prepare. This over-charges by one branch's worth of
		// authorization, but is safe — the alternative of only validating `main`
		// would leave the `fallback` store unvalidated if it runs.
		pallet_utility::Call::if_else { main, fallback, .. } => {
			vec![main.as_ref(), fallback.as_ref()]
		},
		// __Ignore is a phantom variant generated by FRAME (contains Never so is
		// unreachable). Listing it explicitly instead of `_` ensures that new
		// upstream variants cause a compile error, forcing a conscious decision.
		pallet_utility::Call::__Ignore(..) => vec![],
	}
}

/// Extract inner calls from a proxy call variant.
pub fn proxy_inner_calls<T: pallet_proxy::Config>(
	call: &pallet_proxy::Call<T>,
) -> Vec<&<T as pallet_proxy::Config>::RuntimeCall> {
	match call {
		pallet_proxy::Call::proxy { call, .. }
		| pallet_proxy::Call::proxy_announced { call, .. } => vec![call.as_ref()],
		pallet_proxy::Call::add_proxy { .. }
		| pallet_proxy::Call::remove_proxy { .. }
		| pallet_proxy::Call::remove_proxies { .. }
		| pallet_proxy::Call::create_pure { .. }
		| pallet_proxy::Call::kill_pure { .. }
		| pallet_proxy::Call::announce { .. }
		| pallet_proxy::Call::remove_announcement { .. }
		| pallet_proxy::Call::reject_announcement { .. }
		| pallet_proxy::Call::poke_deposit { .. } => vec![],
		pallet_proxy::Call::__Ignore(..) => vec![],
	}
}

/// Extract inner calls from a sudo call variant.
pub fn sudo_inner_calls<T: pallet_sudo::Config>(
	call: &pallet_sudo::Call<T>,
) -> Vec<&<T as pallet_sudo::Config>::RuntimeCall> {
	match call {
		pallet_sudo::Call::sudo { call }
		| pallet_sudo::Call::sudo_as { call, .. }
		| pallet_sudo::Call::sudo_unchecked_weight { call, .. } => vec![call.as_ref()],
		pallet_sudo::Call::set_key { .. } | pallet_sudo::Call::remove_key {} => vec![],
		pallet_sudo::Call::__Ignore(..) => vec![],
	}
}
