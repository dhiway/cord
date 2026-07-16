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

use std::{convert::Infallible, sync::Arc};

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

use crate::{
	chain::ReplicationAuthority, peer::MAX_REQUEST_ENCODED, peer_responder::PeerResponder,
};

pub(crate) const PEER_CONTENT_TYPE: &str = "application/vnd.cord.peer-scale-v1";
pub(crate) const PAGE_PATH: &str = "/_cord/peer/v1/page";
pub(crate) const CHUNK_PATH: &str = "/_cord/peer/v1/chunk";

type Body = Full<Bytes>;

/// Serve only the two private replication routes on an already-bound listener.
pub(crate) async fn serve_peer_http<A>(
	listener: TcpListener,
	responder: Arc<PeerResponder<A>>,
) -> Result<(), std::io::Error>
where
	A: ReplicationAuthority + 'static,
{
	loop {
		let (stream, _) = listener.accept().await?;
		let responder = Arc::clone(&responder);
		tokio::spawn(async move {
			let connection = http1::Builder::new().serve_connection(
				TokioIo::new(stream),
				service_fn(move |request| route(request, Arc::clone(&responder))),
			);
			if connection.await.is_err() {
				// A private peer receives no transport diagnostic or protocol fallback.
			}
		});
	}
}

async fn route<A: ReplicationAuthority>(
	request: Request<Incoming>,
	responder: Arc<PeerResponder<A>>,
) -> Result<Response<Body>, Infallible> {
	Ok(match handle(request, responder).await {
		Ok(bytes) => response(StatusCode::OK, Bytes::from(bytes), true),
		Err(status) => response(status, Bytes::new(), false),
	})
}

async fn handle<A: ReplicationAuthority>(
	request: Request<Incoming>,
	responder: Arc<PeerResponder<A>>,
) -> Result<Vec<u8>, StatusCode> {
	if request.method() != Method::POST || request.uri().query().is_some() {
		return Err(StatusCode::NOT_FOUND);
	}
	let is_page = request.uri().path() == PAGE_PATH;
	if !is_page && request.uri().path() != CHUNK_PATH {
		return Err(StatusCode::NOT_FOUND);
	}
	if request.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
		!= Some(PEER_CONTENT_TYPE)
	{
		return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
	}
	if request.headers().contains_key(TRANSFER_ENCODING)
		|| request.headers().contains_key(AUTHORIZATION)
		|| request.headers().contains_key(COOKIE)
		|| request
			.headers()
			.get(ACCEPT)
			.is_some_and(|value| value.to_str().ok() != Some(PEER_CONTENT_TYPE))
	{
		return Err(StatusCode::BAD_REQUEST);
	}
	let declared_length = request
		.headers()
		.get(CONTENT_LENGTH)
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.parse::<usize>().ok())
		.ok_or(StatusCode::BAD_REQUEST)?;
	if declared_length == 0 || declared_length > MAX_REQUEST_ENCODED {
		return Err(StatusCode::PAYLOAD_TOO_LARGE);
	}
	let bytes = collect_bounded(request.into_body(), MAX_REQUEST_ENCODED).await?;
	if bytes.len() != declared_length {
		return Err(StatusCode::BAD_REQUEST);
	}
	let result = if is_page { responder.page(&bytes).await } else { responder.chunk(&bytes).await };
	// Authentication, finalized topology and local integrity failures deliberately share one wire
	// result. A caller learns no pre-auth parsing or provider-state detail.
	result.map_err(|_| StatusCode::BAD_REQUEST)
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

fn response(status: StatusCode, bytes: Bytes, success: bool) -> Response<Body> {
	let mut builder = Response::builder().status(status);
	if success {
		builder = builder.header(CONTENT_TYPE, PEER_CONTENT_TYPE);
	}
	builder
		.header(CONTENT_LENGTH, bytes.len().to_string())
		.body(Full::new(bytes))
		.unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}
