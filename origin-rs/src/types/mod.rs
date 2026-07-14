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

pub mod account;
pub mod auth;
pub mod core;
pub mod entity;
pub mod error;
pub mod identifiers;
pub mod packet;
pub mod packet_input;
pub mod registry;
pub mod token;

pub use account::{
	account_id_from_subxt, account_id_to_ss58, account_id_to_ss58_subxt, origin_ss58_format,
	ss58_to_account_id, AccountError, CryptoScheme, OriginAccount, OriginPair, ORIGIN_SS58_PREFIX,
};
pub use entity::{
	build_attributes, AccountUnbindEntryViewSdk, AttributeHistoryEntryViewSdk,
	AttributeUpdateInput, AttributesInput, ElementInput, EntityInfoInput, EntityInfoViewSdk,
	EntityOverviewSdk, EntityStateViewSdk, EventBlockView, MaxAdditionalAttributes,
	MaxRawDataLength,
};
pub use error::OriginSdkError;
pub use identifiers::{DecodedIdentifier, Ss58Identifier};
pub use packet::{PacketMetadataView, PacketPointer, PacketStateView, PacketStatus};
pub use packet_input::{PacketAttributeInput, PacketAttributesInput, PacketElementInput};
pub use registry::{
	DelegatePermissionsInput, LookupSpecViewSdk, PacketAttributeViewSdk, PacketMetadataViewSdk,
	PacketStateViewSdk, RegistryAttributeInput, RegistryAttributeViewSdk, RegistryCreateInput,
	RegistryLookupInput, RegistryPermissions, RegistryStateViewSdk, RegistryStatus,
};
pub use token::{
	MaxRawDataLength as TokenMaxRawDataLength, TokenAttributeInput, TokenElementInput,
	TokenEventBlockViewSdk, TokenLookupView, TokenStateEventViewSdk, TokenTimelineViewSdk,
};
pub type ViewValue = subxt::dynamic::DecodedValue;

pub use core::{EntityToken, OriginAccountId, PacketId, RegistryId, TokenDecodedId, TokenId};
pub type EntityNym = Vec<u8>;
