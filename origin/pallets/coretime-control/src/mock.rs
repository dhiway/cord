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

use crate::{self as pallet_coretime_control, *};
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		CoretimeControl: pallet_coretime_control,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = frame_system::mocking::MockBlock<Self>;
}

thread_local! {
	pub static SENT: RefCell<Vec<(RequestId, CoreCount)>> = const { RefCell::new(Vec::new()) };
	pub static APPLIED: RefCell<Vec<CoreCount>> = const { RefCell::new(Vec::new()) };
	pub static RECEIPTS: RefCell<Vec<RequestReceipt>> = const { RefCell::new(Vec::new()) };
	pub static FAIL_SEND: RefCell<bool> = const { RefCell::new(false) };
	pub static FAIL_APPLY: RefCell<bool> = const { RefCell::new(false) };
}

pub struct RequestSender;
impl SendRequest for RequestSender {
	fn send(id: RequestId, count: CoreCount) -> frame_support::dispatch::DispatchResult {
		if FAIL_SEND.with(|fail| *fail.borrow()) {
			return Err(sp_runtime::DispatchError::Other("request send failed"));
		}
		SENT.with(|sent| sent.borrow_mut().push((id, count)));
		Ok(())
	}
}

pub struct RequestApplier;
impl ApplyRequest for RequestApplier {
	fn apply(count: CoreCount) -> frame_support::dispatch::DispatchResult {
		if FAIL_APPLY.with(|fail| *fail.borrow()) {
			return Err(sp_runtime::DispatchError::Other("request apply failed"));
		}
		APPLIED.with(|applied| applied.borrow_mut().push(count));
		Ok(())
	}
}

pub struct ReceiptSender;
impl SendReceipt for ReceiptSender {
	fn send(receipt: RequestReceipt) {
		RECEIPTS.with(|receipts| receipts.borrow_mut().push(receipt));
	}
}

parameter_types! {
	pub const MaxTrackedRequests: u32 = 2;
}

impl Config for Test {
	type RequestOrigin = frame_system::EnsureRoot<u64>;
	type BrokerOrigin = frame_system::EnsureRoot<u64>;
	type ReceiptOrigin = frame_system::EnsureRoot<u64>;
	type TransportControlOrigin = frame_system::EnsureRoot<u64>;
	type TransportControlEnabled = frame_support::traits::ConstBool<true>;
	type RequestSender = RequestSender;
	type RequestApplier = RequestApplier;
	type ReceiptSender = ReceiptSender;
	type MaxTrackedRequests = MaxTrackedRequests;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	SENT.with(|value| value.borrow_mut().clear());
	APPLIED.with(|value| value.borrow_mut().clear());
	RECEIPTS.with(|value| value.borrow_mut().clear());
	FAIL_SEND.with(|value| *value.borrow_mut() = false);
	FAIL_APPLY.with(|value| *value.borrow_mut() = false);
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
