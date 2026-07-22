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

use std::sync::Arc;

use codec::{Decode, Encode};
use frame_support::view_functions::ViewFunctionDispatchError;
use sp_runtime::traits::SaturatedConversion;

use super::{connection::Connection, signer::Signer};
use crate::types::{auth, error::OriginSdkError};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

/// Minimal view caller: encodes args, hits the pallet view, and decodes into a concrete type.
#[derive(Clone)]
pub struct ViewClient {
	connection: Arc<Connection>,
}

impl ViewClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	/// Generic view invocation: caller supplies the exact argument shape (including
	/// Authorization when required). The call is sent through the runtime API without
	/// metadata-driven shape guessing, and the response is decoded directly into `T`.
	pub async fn call<T>(
		&self,
		pallet: &str,
		function: &str,
		args: impl Encode,
	) -> Result<T, OriginSdkError>
	where
		T: Decode,
	{
		let metadata = self.connection.metadata();
		let vf = metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?
			.view_functions()
			.find(|vf| vf.name() == function)
			.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;
		let query_id = *vf.query_id();

		let args_bytes = args.encode();
		let params = (query_id, args_bytes).encode();

		let api = self.connection.online().runtime_api();
		let at = api.at_latest().await.map_err(|e| OriginSdkError::View(e.to_string()))?;
		let raw = at
			.call_raw("RuntimeViewFunction_execute_view_function", Some(&params))
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?;

		let inner: Result<Vec<u8>, ViewFunctionDispatchError> = Decode::decode(&mut &*raw)
			.map_err(|e| OriginSdkError::Decode(format!("{pallet}.{function} dispatch: {e}")))?;

		let bytes = inner.map_err(|e| OriginSdkError::View(format!("{e:?}")))?;

		T::decode(&mut &*bytes)
			.map_err(|e| OriginSdkError::Decode(format!("{pallet}.{function} decode: {e}")))
	}

	/// Build the Authorization envelope expected by pallet view functions.
	pub async fn authorization_for<S: Signer>(
		&self,
		signer: &S,
		pallet: &str,
		function: &str,
	) -> Result<Auth, OriginSdkError> {
		let reference_block = self
			.connection
			.online()
			.blocks()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?
			.number()
			.saturated_into::<u32>();

		Ok(auth::build_authorization(signer, pallet, function, reference_block).await)
	}
}
