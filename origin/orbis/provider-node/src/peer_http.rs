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

//! Private HTTP/1 ingress for canonical service-key-authenticated replication messages.

use std::{convert::Infallible, sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt, Full};
use hyper::{
	body::Incoming,
	header::{ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, TRANSFER_ENCODING},
	server::conn::http1,
	service::service_fn,
	Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::{
	chain::ReplicationAuthority, peer::MAX_REQUEST_ENCODED, peer_responder::PeerResponder,
};

pub(crate) const PEER_CONTENT_TYPE: &str = "application/vnd.cord.peer-scale-v1";
pub(crate) const PAGE_PATH: &str = "/_cord/peer/v1/page";
pub(crate) const CHUNK_PATH: &str = "/_cord/peer/v1/chunk";
pub(crate) const CONFIRMATION_PATH: &str = "/_cord/peer/v1/checkpoint-confirmation";
pub(crate) const CONFIRMATION_CONTENT_TYPE: &str =
	"application/vnd.cord.checkpoint-confirmation-scale-v1";
pub(crate) const MAX_CONFIRMATION_BYTES: usize = 64 * 1024;
const MAX_PEER_CONNECTIONS: usize = 64;
const PEER_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

type Body = Full<Bytes>;

#[async_trait]
pub(crate) trait PeerHttpResponder: Send + Sync {
	async fn page(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError>;
	async fn chunk(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError>;
	async fn confirmation(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError>;
}

#[async_trait]
impl<A: ReplicationAuthority> PeerHttpResponder for PeerResponder<A> {
	async fn page(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
		self.page(request).await
	}

	async fn chunk(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
		self.chunk(request).await
	}

	async fn confirmation(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
		self.confirmation(request).await
	}
}

/// Serve only the two private replication routes on an already-bound listener.
pub(crate) async fn serve_peer_http<A>(
	listener: TcpListener,
	responder: Arc<PeerResponder<A>>,
) -> Result<(), std::io::Error>
where
	A: ReplicationAuthority + 'static,
{
	serve_peer_http_with_limits(listener, responder, MAX_PEER_CONNECTIONS, PEER_CONNECTION_TIMEOUT)
		.await
}

pub(crate) async fn serve_peer_http_with_limits<H>(
	listener: TcpListener,
	responder: Arc<H>,
	max_connections: usize,
	connection_timeout: Duration,
) -> Result<(), std::io::Error>
where
	H: PeerHttpResponder + 'static,
{
	if max_connections == 0 || connection_timeout.is_zero() {
		return Err(std::io::Error::new(
			std::io::ErrorKind::InvalidInput,
			"peer ingress limits must be non-zero",
		));
	}
	let permits = Arc::new(Semaphore::new(max_connections));
	loop {
		let (stream, _) = listener.accept().await?;
		let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
			// Refuse excess peers without parsing or emitting an authentication oracle.
			continue;
		};
		let responder = Arc::clone(&responder);
		tokio::spawn(async move {
			let _permit = permit;
			let connection = http1::Builder::new().serve_connection(
				TokioIo::new(stream),
				service_fn(move |request| route(request, Arc::clone(&responder))),
			);
			if tokio::time::timeout(connection_timeout, connection).await.is_err() {
				// Slow or idle peers receive no timeout diagnostic.
			} else {
				// A private peer receives no transport diagnostic or protocol fallback.
			}
		});
	}
}

async fn route<H: PeerHttpResponder>(
	request: Request<Incoming>,
	responder: Arc<H>,
) -> Result<Response<Body>, Infallible> {
	Ok(match handle(request, responder).await {
		Ok((bytes, content_type)) => {
			response(StatusCode::OK, Bytes::from(bytes), Some(content_type))
		},
		Err(status) => response(status, Bytes::new(), None),
	})
}

async fn handle<H: PeerHttpResponder>(
	request: Request<Incoming>,
	responder: Arc<H>,
) -> Result<(Vec<u8>, &'static str), StatusCode> {
	if request.method() != Method::POST || request.uri().query().is_some() {
		return Err(StatusCode::NOT_FOUND);
	}
	let is_page = request.uri().path() == PAGE_PATH;
	let is_confirmation = request.uri().path() == CONFIRMATION_PATH;
	if !is_page && !is_confirmation && request.uri().path() != CHUNK_PATH {
		return Err(StatusCode::NOT_FOUND);
	}
	let content_type = if is_confirmation { CONFIRMATION_CONTENT_TYPE } else { PEER_CONTENT_TYPE };
	if request.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
		!= Some(content_type)
	{
		return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
	}
	if request.headers().contains_key(TRANSFER_ENCODING)
		|| request.headers().contains_key(AUTHORIZATION)
		|| request.headers().contains_key(COOKIE)
		|| request
			.headers()
			.get(ACCEPT)
			.is_some_and(|value| value.to_str().ok() != Some(content_type))
	{
		return Err(StatusCode::BAD_REQUEST);
	}
	let declared_length = request
		.headers()
		.get(CONTENT_LENGTH)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.parse::<usize>().ok())
		.ok_or(StatusCode::BAD_REQUEST)?;
	let request_limit = if is_confirmation { MAX_CONFIRMATION_BYTES } else { MAX_REQUEST_ENCODED };
	if declared_length == 0 || declared_length > request_limit {
		return Err(StatusCode::PAYLOAD_TOO_LARGE);
	}
	let bytes = collect_bounded(request.into_body(), request_limit).await?;
	if bytes.len() != declared_length {
		return Err(StatusCode::BAD_REQUEST);
	}
	let result = if is_confirmation {
		responder.confirmation(&bytes).await
	} else if is_page {
		responder.page(&bytes).await
	} else {
		responder.chunk(&bytes).await
	};
	// Authentication, finalized topology and local integrity failures deliberately share one wire
	// result. A caller learns no pre-auth parsing or provider-state detail.
	let response = result.map_err(|_| StatusCode::BAD_REQUEST)?;
	if response.is_empty() || (is_confirmation && response.len() > MAX_CONFIRMATION_BYTES) {
		return Err(StatusCode::BAD_REQUEST);
	}
	Ok((response, content_type))
}

pub(crate) async fn collect_bounded(
	mut body: Incoming,
	limit: usize,
) -> Result<Vec<u8>, StatusCode> {
	let mut bytes = BytesMut::new();
	while let Some(frame) = body.frame().await {
		let frame = frame.map_err(|_| StatusCode::BAD_REQUEST)?;
		if let Some(data) = frame.data_ref() {
			if bytes.len().checked_add(data.len()).is_none_or(|length| length > limit) {
				return Err(StatusCode::PAYLOAD_TOO_LARGE);
			}
			bytes.extend_from_slice(data);
		}
	}
	if bytes.is_empty() {
		return Err(StatusCode::BAD_REQUEST);
	}
	Ok(bytes.to_vec())
}

fn response(
	status: StatusCode,
	bytes: Bytes,
	content_type: Option<&'static str>,
) -> Response<Body> {
	let mut builder = Response::builder().status(status);
	if let Some(content_type) = content_type {
		builder = builder.header(CONTENT_TYPE, content_type);
	}
	builder
		.header(CONTENT_LENGTH, bytes.len().to_string())
		.body(Full::new(bytes))
		.unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

#[cfg(test)]
mod tests {
	use std::sync::{Arc, Mutex};

	use tokio::{
		io::{AsyncReadExt, AsyncWriteExt},
		net::{TcpListener, TcpStream},
	};

	use super::*;

	#[derive(Default)]
	struct FakeResponder {
		confirmations: Mutex<Vec<Vec<u8>>>,
	}

	#[async_trait]
	impl PeerHttpResponder for FakeResponder {
		async fn page(&self, _request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
			Err(crate::ContentError::IntegrityFailed)
		}

		async fn chunk(&self, _request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
			Err(crate::ContentError::IntegrityFailed)
		}

		async fn confirmation(&self, request: &[u8]) -> Result<Vec<u8>, crate::ContentError> {
			self.confirmations.lock().unwrap().push(request.to_vec());
			Ok(b"canonical-response".to_vec())
		}
	}

	async fn exchange(address: std::net::SocketAddr, request: &str) -> Vec<u8> {
		let mut stream = TcpStream::connect(address).await.unwrap();
		stream.write_all(request.as_bytes()).await.unwrap();
		let mut response = Vec::new();
		stream.read_to_end(&mut response).await.unwrap();
		response
	}

	#[tokio::test]
	async fn confirmation_http_is_strict_and_exact_replays_succeed() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let responder = Arc::new(FakeResponder::default());
		let server = tokio::spawn(serve_peer_http_with_limits(
			listener,
			Arc::clone(&responder),
			4,
			Duration::from_secs(2),
		));
		let canonical = format!(
			"POST {CONFIRMATION_PATH} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: {CONFIRMATION_CONTENT_TYPE}\r\nAccept: {CONFIRMATION_CONTENT_TYPE}\r\nContent-Length: 3\r\n\r\nabc"
		);
		for _ in 0..2 {
			let response = exchange(address, &canonical).await;
			assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
			assert!(response.ends_with(b"canonical-response"));
		}
		for request in [
			canonical.replace(CONFIRMATION_PATH, &format!("{CONFIRMATION_PATH}?token=x")),
			canonical.replace("Content-Length: 3\r\n", ""),
			canonical.replace(
				&format!("Content-Type: {CONFIRMATION_CONTENT_TYPE}"),
				"Content-Type: application/octet-stream",
			),
			canonical.replace("Accept:", "Authorization: Bearer secret\r\nAccept:"),
			canonical.replace("Accept:", "Cookie: session=x\r\nAccept:"),
			format!(
				"POST {CONFIRMATION_PATH} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: {CONFIRMATION_CONTENT_TYPE}\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n"
			),
		] {
			let response = exchange(address, &request).await;
			assert!(!response.starts_with(b"HTTP/1.1 200 OK\r\n"));
			let body = response
				.windows(4)
				.position(|window| window == b"\r\n\r\n")
				.map(|index| &response[index + 4..])
				.unwrap();
			assert!(body.is_empty());
		}
		assert_eq!(
			responder.confirmations.lock().unwrap().as_slice(),
			vec![b"abc".to_vec(), b"abc".to_vec()].as_slice()
		);
		server.abort();
	}
}
