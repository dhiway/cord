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

use crate::types::core::TokenDecodedId;
use origin_primitives::{entity::EventBlockView, token::TokenStateEventView};
use sp_core::H256;

pub type TokenStateEventViewSdk = TokenStateEventView<H256>;
pub type TokenTimelineViewSdk = origin_primitives::token::TokenTimelineView<H256>;
pub type TokenLookupView = TokenDecodedId;
pub type TokenEventBlockViewSdk = EventBlockView;

// --- Extrinsic input mirrors ---
use codec::{Decode, Encode};
use origin_primitives::element::ElementView;
use scale_info::TypeInfo;

/// Bounds mirror entity/registry raw length.
pub type MaxRawDataLength = crate::types::entity::MaxRawDataLength;
pub type TokenElementInput = crate::types::entity::ElementInput;

/// Token attribute update using bounded Element.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct TokenAttributeInput {
	pub key: Vec<u8>,
	pub value: TokenElementInput,
}

impl TokenAttributeInput {
	pub fn from_view(
		key: &[u8],
		view: &ElementView,
	) -> Result<Self, crate::types::error::OriginSdkError> {
		let value = crate::schema::entity::element_from_view(view)?;
		Ok(Self { key: key.to_vec(), value })
	}
}
