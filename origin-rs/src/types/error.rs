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

use origin_primitives::authorization::AuthorizationError;
use subxt::error::MetadataError;
use thiserror::Error;

/// Unified SDK error type.
#[derive(Debug, Clone, Error)]
pub enum OriginSdkError {
	#[error("connection error: {0}")]
	Connection(String),
	#[error("metadata error: {0}")]
	Metadata(String),
	#[error("encode error: {0}")]
	Encode(String),
	#[error("decode error: {0}")]
	Decode(String),
	#[error("view auth error: {0:?}")]
	ViewAuth(AuthorizationError),
	#[error("view error: {0}")]
	View(String),
	#[error("config error: {0}")]
	Config(String),
	#[error("transaction error: {0}")]
	Tx(String),
	#[error("nonce error: {0}")]
	Nonce(String),
	#[error("meta-tx error: {0}")]
	MetaTx(String),
	#[error("schema error: {0}")]
	Schema(String),
	#[error("timeout")]
	Timeout,
	#[error("invalid input: {0}")]
	InvalidInput(String),
}

impl From<subxt::Error> for OriginSdkError {
	fn from(err: subxt::Error) -> Self {
		OriginSdkError::Tx(err.to_string())
	}
}

impl From<subxt::ext::scale_decode::Error> for OriginSdkError {
	fn from(err: subxt::ext::scale_decode::Error) -> Self {
		OriginSdkError::Decode(err.to_string())
	}
}

impl From<subxt::ext::scale_encode::Error> for OriginSdkError {
	fn from(err: subxt::ext::scale_encode::Error) -> Self {
		OriginSdkError::Encode(err.to_string())
	}
}

impl From<MetadataError> for OriginSdkError {
	fn from(err: MetadataError) -> Self {
		OriginSdkError::Metadata(err.to_string())
	}
}

impl From<AuthorizationError> for OriginSdkError {
	fn from(err: AuthorizationError) -> Self {
		OriginSdkError::ViewAuth(err)
	}
}
