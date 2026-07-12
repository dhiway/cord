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

use super::*;
use crate::{Runtime, Token};
use frame_support::parameter_types;
use pallet_orbis_entity::entity::EntityInfo;

parameter_types! {
	pub const MaxLinkedAccounts: u32 = 32;
	pub const MaxRawDataLength: u32 = 4096;
	pub const MaxEntityNymLength: u32 = 32;
	pub const MaxAdditionalAttributes: u32 = 32;
	pub const DefaultEntityOverviewHistory: u32 = 20;
	pub const MaxEntityOverviewHistory: u32 = 50;
	pub const GeneralAdminBodyId: BodyId = BodyId::Administration;
	pub const MaxAuthorizationLen: u32 = 256;
	pub const MaxAuthorizationTTL: u32 = 30;
}

pub type IdentityAdminOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	EnsureXcm<IsVoiceOfBody<GovernanceLocation, GeneralAdminBodyId>>,
>;

impl pallet_orbis_entity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Token = Token;
	type MaxLinkedAccounts = MaxLinkedAccounts;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type EntityInfoPacket = EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>;
	type Feeless = Feeless;
	type MaxEntityNymLength = MaxEntityNymLength;
	type DefaultEntityOverviewHistory = DefaultEntityOverviewHistory;
	type MaxEntityOverviewHistory = MaxEntityOverviewHistory;
	type MaxAuthorizationLen = MaxAuthorizationLen;
	type MaxAuthorizationTTL = MaxAuthorizationTTL;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weights::pallet_orbis_entity::WeightInfo<Runtime>;
}
