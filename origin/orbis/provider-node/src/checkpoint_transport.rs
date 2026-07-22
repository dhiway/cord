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

//! Bounded private transport for authenticated checkpoint confirmations.

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{
	header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, TRANSFER_ENCODING},
	Method, Request, StatusCode, Uri,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
	client::legacy::{connect::HttpConnector, Client},
	rt::TokioExecutor,
};

use crate::peer_http::{
	collect_bounded, CONFIRMATION_CONTENT_TYPE, CONFIRMATION_PATH, MAX_CONFIRMATION_BYTES,
};

type Connector = HttpsConnector<HttpConnector>;
type HttpClient = Client<Connector, Full<Bytes>>;

#[derive(Clone, Debug)]
pub(crate) struct CheckpointConfirmationEndpoint(Uri);

impl CheckpointConfirmationEndpoint {
	pub(crate) fn pinned(bytes: &[u8], expected_hash: [u8; 32]) -> Result<Self, ()> {
		if bytes.is_empty()
			|| bytes.len() > 256
			|| sp_crypto_hashing::blake2_256(bytes) != expected_hash
		{
			return Err(());
		}
		let endpoint = std::str::from_utf8(bytes).map_err(|_| ())?;
		let uri: Uri = endpoint.parse().map_err(|_| ())?;
		let scheme = uri.scheme_str().ok_or(())?;
		if !matches!(scheme, "http" | "https")
			|| uri.authority().is_none()
			|| uri.path() != "/"
			|| uri.query().is_some()
			|| uri.authority().is_some_and(|authority| authority.as_str().contains('@'))
		{
			return Err(());
		}
		let target =
			format!("{scheme}://{}{CONFIRMATION_PATH}", uri.authority().expect("checked above"))
				.parse()
				.map_err(|_| ())?;
		Ok(Self(target))
	}
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckpointTransportError {
	#[error("checkpoint confirmation transport rejected the request")]
	Rejected,
	#[error("checkpoint confirmation transport timed out")]
	Timeout,
}

#[async_trait]
pub(crate) trait CheckpointConfirmationTransport: Send + Sync {
	async fn confirm(
		&self,
		endpoint: &CheckpointConfirmationEndpoint,
		request: &[u8],
	) -> Result<Vec<u8>, CheckpointTransportError>;
}

pub(crate) struct HyperCheckpointConfirmationTransport {
	client: HttpClient,
	timeout: Duration,
}

impl HyperCheckpointConfirmationTransport {
	pub(crate) fn new(timeout: Duration) -> Result<Self, CheckpointTransportError> {
		if timeout.is_zero() {
			return Err(CheckpointTransportError::Rejected);
		}
		let connector = HttpsConnectorBuilder::new()
			.with_native_roots()
			.map_err(|_| CheckpointTransportError::Rejected)?
			.https_or_http()
			.enable_http1()
			.build();
		Ok(Self { client: Client::builder(TokioExecutor::new()).build(connector), timeout })
	}
}

#[async_trait]
impl CheckpointConfirmationTransport for HyperCheckpointConfirmationTransport {
	async fn confirm(
		&self,
		endpoint: &CheckpointConfirmationEndpoint,
		request: &[u8],
	) -> Result<Vec<u8>, CheckpointTransportError> {
		if request.is_empty() || request.len() > MAX_CONFIRMATION_BYTES {
			return Err(CheckpointTransportError::Rejected);
		}
		let request = Request::builder()
			.method(Method::POST)
			.uri(endpoint.0.clone())
			.header(CONTENT_TYPE, CONFIRMATION_CONTENT_TYPE)
			.header(ACCEPT, CONFIRMATION_CONTENT_TYPE)
			.header(CONTENT_LENGTH, request.len().to_string())
			.body(Full::new(Bytes::copy_from_slice(request)))
			.map_err(|_| CheckpointTransportError::Rejected)?;
		let response = tokio::time::timeout(self.timeout, self.client.request(request))
			.await
			.map_err(|_| CheckpointTransportError::Timeout)?
			.map_err(|_| CheckpointTransportError::Rejected)?;
		if response.status() != StatusCode::OK
			|| response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
				!= Some(CONFIRMATION_CONTENT_TYPE)
			|| response.headers().contains_key(TRANSFER_ENCODING)
			|| response.headers().contains_key(AUTHORIZATION)
			|| response.headers().contains_key(COOKIE)
		{
			return Err(CheckpointTransportError::Rejected);
		}
		let length = response
			.headers()
			.get(CONTENT_LENGTH)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.parse::<usize>().ok())
			.filter(|length| *length > 0 && *length <= MAX_CONFIRMATION_BYTES)
			.ok_or(CheckpointTransportError::Rejected)?;
		let bytes = tokio::time::timeout(
			self.timeout,
			collect_bounded(response.into_body(), MAX_CONFIRMATION_BYTES),
		)
		.await
		.map_err(|_| CheckpointTransportError::Timeout)?
		.map_err(|_| CheckpointTransportError::Rejected)?;
		if bytes.len() != length {
			return Err(CheckpointTransportError::Rejected);
		}
		Ok(bytes)
	}
}
