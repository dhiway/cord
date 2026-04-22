// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use crate::{attribute::AttributeValueView, element::ElementView};
use alloc::vec::Vec;
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::Debug;

/// Minimal block reference for view responses.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct EventBlockView {
	pub height: u32,
	pub index: u32,
}

/// History entry for a single attribute key/version.
/// Returned by `Entity::overview`, `Entity::attribute_history`,
/// `Entity::attribute_history_for_key`, and `Entity::attribute_history_entry`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct AttributeHistoryEntryView {
	pub key: Vec<u8>,
	pub version: u64,
	pub old_value: ElementView,
	pub block: EventBlockView,
}

/// Flattened entity info using `ElementView` and `AttributeValueView`.
/// Returned by the `Entity::details` view.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct EntityInfoView {
	pub display: ElementView,
	pub web: ElementView,
	pub email: ElementView,
	pub attributes: Option<Vec<AttributeValueView>>,
}

/// Composite overview of an entity.
/// Returned by the `Entity::overview` view.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct EntityStateView<AccountId> {
	pub info: EntityInfoView,
	pub nym: Option<Vec<u8>>,
	pub linked_accounts: Vec<AccountId>,
	pub history: Vec<AttributeHistoryEntryView>,
}

/// Slimmed down overview (info + nym).
/// Returned by SDK helpers derived from `EntityStateView`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct EntityOverview {
	pub info: EntityInfoView,
	pub nym: Option<Vec<u8>>,
}

impl<AccountId> From<EntityStateView<AccountId>> for EntityOverview {
	fn from(state: EntityStateView<AccountId>) -> Self {
		Self { info: state.info, nym: state.nym }
	}
}

/// Unbind entry for the `Entity::account_history` view.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, Debug)]
pub struct AccountUnbindEntryView<AccountId> {
	pub account: AccountId,
	pub block: EventBlockView,
}
