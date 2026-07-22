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

use crate::{
	extrinsic::calls::token,
	tx::{handle::TxHandle, AccountTx},
	types::{error::OriginSdkError, token::TokenAttributeInput},
};
use codec::Encode;
use origin_primitives::Ss58Identifier;

pub struct TokenTx<'a> {
	account: &'a AccountTx,
}

impl<'a> TokenTx<'a> {
	pub(crate) fn new(account: &'a AccountTx) -> Self {
		Self { account }
	}

	/// Rotate a token attribute using raw bytes.
	pub async fn submit_rotate_attribute(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: &[u8],
	) -> Result<TxHandle, OriginSdkError> {
		let payload =
			token::rotate_attribute_call(&self.account.client().metadata(), token_id, key, value)?;
		self.account.submit(payload).await
	}

	/// Convenience: accept an ElementView and SCALE-encode to bytes.
	pub async fn submit_rotate_attribute_view(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: origin_primitives::element::ElementView,
	) -> Result<TxHandle, OriginSdkError> {
		let input = TokenAttributeInput::from_view(key, &value)?;
		let bytes = input.value.encode();
		self.submit_rotate_attribute(token_id, &input.key, &bytes).await
	}
}
