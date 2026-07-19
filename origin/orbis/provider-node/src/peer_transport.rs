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

//! Outbound HTTP/1 transport pinned to an exact finalized replication session.

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{
	header::{ACCEPT, CONTENT_LENGTH, CONTENT_TYPE},
	Method, Request, StatusCode, Uri,
};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
	client::legacy::{connect::HttpConnector, Client},
	rt::TokioExecutor,
};

use crate::{
	peer::{
		PeerChunkRequestV1, PeerChunkResponseV1, PeerSyncPageRequestV1, PeerSyncPageResponseV1,
		MAX_CHUNK_RESPONSE_ENCODED, MAX_PAGE_RESPONSE_ENCODED, MAX_REQUEST_ENCODED,
	},
	peer_http::{collect_bounded, CHUNK_PATH, PAGE_PATH, PEER_CONTENT_TYPE},
	replication_session::ReplicationSessionV1,
};

type Connector = HttpsConnector<HttpConnector>;
type HttpClient = Client<Connector, Full<Bytes>>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum PeerTransportError {
	#[error("replication peer endpoint is invalid")]
	Endpoint,
	#[error("replication peer request is invalid")]
	Request,
	#[error("replication peer transport failed")]
	Transport,
	#[error("replication peer response was refused")]
	Refused,
	#[error("replication peer response is invalid")]
	Response,
	#[error("replication peer request timed out")]
	Timeout,
}

#[async_trait]
pub(crate) trait PeerTransport: Send + Sync {
	async fn page(
		&self,
		session: &ReplicationSessionV1,
		request_bytes: &[u8],
	) -> Result<Vec<u8>, PeerTransportError>;

	async fn chunk(
		&self,
		session: &ReplicationSessionV1,
		request_bytes: &[u8],
	) -> Result<Vec<u8>, PeerTransportError>;
}

/// HTTP and HTTPS client with native trust roots and no redirect or credential policy.
pub(crate) struct HyperPeerTransport {
	client: HttpClient,
	timeout: Duration,
}

impl HyperPeerTransport {
	pub(crate) fn new(timeout: Duration) -> Result<Self, PeerTransportError> {
		if timeout.is_zero() {
			return Err(PeerTransportError::Endpoint);
		}
		let connector = HttpsConnectorBuilder::new()
			.with_native_roots()
			.map_err(|_| PeerTransportError::Transport)?
			.https_or_http()
			.enable_http1()
			.build();
		let client = Client::builder(TokioExecutor::new()).build(connector);
		Ok(Self { client, timeout })
	}

	async fn exchange(
		&self,
		session: &ReplicationSessionV1,
		request_bytes: &[u8],
		path: &'static str,
		response_limit: usize,
	) -> Result<Vec<u8>, PeerTransportError> {
		if request_bytes.is_empty() || request_bytes.len() > MAX_REQUEST_ENCODED {
			return Err(PeerTransportError::Request);
		}
		let uri = pinned_source_uri(session, path)?;
		let request = Request::builder()
			.method(Method::POST)
			.uri(uri)
			.header(CONTENT_TYPE, PEER_CONTENT_TYPE)
			.header(ACCEPT, PEER_CONTENT_TYPE)
			.header(CONTENT_LENGTH, request_bytes.len().to_string())
			.body(Full::new(Bytes::copy_from_slice(request_bytes)))
			.map_err(|_| PeerTransportError::Request)?;
		let response = tokio::time::timeout(self.timeout, self.client.request(request))
			.await
			.map_err(|_| PeerTransportError::Timeout)?
			.map_err(|_| PeerTransportError::Transport)?;
		if response.status() != StatusCode::OK {
			return Err(PeerTransportError::Refused);
		}
		if response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
			!= Some(PEER_CONTENT_TYPE)
		{
			return Err(PeerTransportError::Response);
		}
		let length = response
			.headers()
			.get(CONTENT_LENGTH)
			.and_then(|value| value.to_str().ok())
			.and_then(|value| value.parse::<usize>().ok())
			.ok_or(PeerTransportError::Response)?;
		if length == 0 || length > response_limit {
			return Err(PeerTransportError::Response);
		}
		let bytes = tokio::time::timeout(
			self.timeout,
			collect_bounded(response.into_body(), response_limit),
		)
		.await
		.map_err(|_| PeerTransportError::Timeout)?
		.map_err(|_| PeerTransportError::Response)?;
		if bytes.len() != length {
			return Err(PeerTransportError::Response);
		}
		Ok(bytes)
	}
}

#[async_trait]
impl PeerTransport for HyperPeerTransport {
	async fn page(
		&self,
		session: &ReplicationSessionV1,
		request_bytes: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		let request = PeerSyncPageRequestV1::decode_authenticated(request_bytes)
			.map_err(|_| PeerTransportError::Request)?;
		if request.context() != session.context() {
			return Err(PeerTransportError::Request);
		}
		let bytes = self
			.exchange(session, request_bytes, PAGE_PATH, MAX_PAGE_RESPONSE_ENCODED)
			.await?;
		PeerSyncPageResponseV1::decode_canonical(&bytes, &request)
			.map_err(|_| PeerTransportError::Response)?;
		Ok(bytes)
	}

	async fn chunk(
		&self,
		session: &ReplicationSessionV1,
		request_bytes: &[u8],
	) -> Result<Vec<u8>, PeerTransportError> {
		let request = PeerChunkRequestV1::decode_authenticated(request_bytes)
			.map_err(|_| PeerTransportError::Request)?;
		if request.context() != session.context() {
			return Err(PeerTransportError::Request);
		}
		let bytes = self
			.exchange(session, request_bytes, CHUNK_PATH, MAX_CHUNK_RESPONSE_ENCODED)
			.await?;
		PeerChunkResponseV1::decode_canonical(&bytes, &request)
			.map_err(|_| PeerTransportError::Response)?;
		Ok(bytes)
	}
}

fn pinned_source_uri(
	session: &ReplicationSessionV1,
	path: &'static str,
) -> Result<Uri, PeerTransportError> {
	let endpoint = session.source().endpoint();
	if endpoint.is_empty()
		|| endpoint.len() > 256
		|| sp_crypto_hashing::blake2_256(endpoint) != session.source().endpoint_hash()
	{
		return Err(PeerTransportError::Endpoint);
	}
	let endpoint = std::str::from_utf8(endpoint).map_err(|_| PeerTransportError::Endpoint)?;
	let uri: Uri = endpoint.parse().map_err(|_| PeerTransportError::Endpoint)?;
	let scheme = uri.scheme_str().ok_or(PeerTransportError::Endpoint)?;
	if !matches!(scheme, "http" | "https")
		|| uri.authority().is_none()
		|| uri.path() != "/"
		|| uri.query().is_some()
		|| uri.authority().is_some_and(|authority| authority.as_str().contains('@'))
	{
		return Err(PeerTransportError::Endpoint);
	}
	format!("{scheme}://{}{path}", uri.authority().expect("checked above"))
		.parse()
		.map_err(|_| PeerTransportError::Endpoint)
}

#[cfg(test)]
mod tests {
	use std::{convert::Infallible, sync::Arc};

	use codec::Encode;
	use http_body_util::BodyExt;
	use hyper::{
		header::{AUTHORIZATION, TRANSFER_ENCODING},
		server::conn::http1,
		service::service_fn,
		Response,
	};
	use hyper_util::rt::TokioIo;
	use sp_core::{ed25519, Pair as _};
	use sp_crypto_hashing::blake2_256;
	use tempfile::TempDir;
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	use tokio::net::{TcpListener, TcpStream};

	use super::*;
	use crate::{
		chain::{ChainError, ReplicationProviderSnapshot, ReplicationTopologySnapshot},
		checkpoint_stack::CheckpointStack,
		peer::{
			PeerChunkExpectationV1, PeerMmrCommitmentV1, PeerPageExpectationV1,
			PeerRequestIdentityV1,
		},
		peer_http::{serve_peer_http, serve_peer_http_with_limits},
		peer_responder::PeerResponder,
		storage::{bucket_mmr::BucketMmrStore, StreamingDescriptor, StreamingStore},
		BucketId, CanonicalCid, OperationId, CHUNK_BYTES,
	};

	#[derive(Clone)]
	struct MockAuthority {
		topology: ReplicationTopologySnapshot,
	}

	#[async_trait]
	impl crate::chain::ReplicationAuthority for MockAuthority {
		async fn replication_topology(
			&self,
			bucket_id: [u8; 32],
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			if bucket_id != self.topology.bucket_id {
				return Err(ChainError::Rejected("wrong bucket".into()));
			}
			Ok(self.topology.clone())
		}

		async fn replication_topology_at(
			&self,
			bucket_id: [u8; 32],
			finalized_hash: [u8; 32],
			finalized_number: u32,
		) -> Result<ReplicationTopologySnapshot, ChainError> {
			if bucket_id != self.topology.bucket_id
				|| finalized_hash != self.topology.finalized_hash
				|| finalized_number != self.topology.finalized_number
			{
				return Err(ChainError::Rejected("wrong pinned topology".into()));
			}
			Ok(self.topology.clone())
		}
	}

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn provider(
		id: u8,
		order: u8,
		primary: bool,
		key: [u8; 32],
		endpoint: Vec<u8>,
	) -> ReplicationProviderSnapshot {
		ReplicationProviderSnapshot {
			provider: [id; 32],
			order,
			primary,
			record_present: true,
			endpoint_hash: Some(blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(key),
			active_service_key_version: Some(u64::from(order) + 1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(8),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: None,
		}
	}

	fn topology(source_endpoint: Vec<u8>) -> ReplicationTopologySnapshot {
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: [1; 32],
			finalized_hash: [2; 32],
			finalized_number: 10,
			governed_finalized_checkpoint: Some(8),
			bucket_id: [3; 32],
			bucket_version: 4,
			primary: [4; 32],
			replicas: vec![[5; 32]],
			providers: vec![
				provider(4, 0, true, pair(11).public().0, source_endpoint),
				provider(5, 1, false, pair(12).public().0, b"https://target.invalid".to_vec()),
			],
			current_checkpoint: None,
			snapshot_hash: [0; 32],
		};
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut input);
		topology.snapshot_hash = blake2_256(&input);
		topology
	}

	fn install_source(temp: &TempDir) -> (PeerMmrCommitmentV1, CanonicalCid, Vec<u8>) {
		let bytes = vec![31; CHUNK_BYTES + 7];
		let cid = CanonicalCid::from_digest(blake2_256(&bytes));
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: OperationId::from_bytes([7; 16]),
					bucket_id: BucketId::from_bytes([3; 32]),
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let candidate =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes([3; 32]), 0).unwrap();
		let commitment = PeerMmrCommitmentV1::new(candidate.mmr_root.0, 0, 1, 0).unwrap();
		(commitment, cid, bytes)
	}

	async fn raw(
		client: &HttpClient,
		uri: String,
		method: Method,
		content_type: Option<&str>,
		body: &[u8],
	) -> hyper::Response<hyper::body::Incoming> {
		let mut builder = Request::builder().method(method).uri(uri);
		if let Some(content_type) = content_type {
			builder = builder.header(CONTENT_TYPE, content_type);
		}
		client
			.request(builder.body(Full::new(Bytes::copy_from_slice(body))).unwrap())
			.await
			.unwrap()
	}

	fn page_for_endpoint(endpoint: &[u8]) -> (ReplicationSessionV1, PeerSyncPageRequestV1) {
		let session = ReplicationSessionV1::from_topology(
			topology(endpoint.to_vec()),
			[5; 32],
			pair(12).public().0,
			[4; 32],
			[5; 32],
			PeerMmrCommitmentV1::new([20; 32], 0, 1, 0).unwrap(),
		)
		.unwrap();
		let expected = PeerPageExpectationV1::new(
			session.context().clone(),
			PeerRequestIdentityV1::new([15; 16], [18; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap();
		(session, request)
	}

	async fn static_peer(
		status: StatusCode,
		content_type: &'static str,
		declared_length: usize,
		body: Vec<u8>,
		delay: Duration,
	) -> (String, tokio::task::JoinHandle<()>) {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let endpoint = format!("http://{}", listener.local_addr().unwrap());
		let task = tokio::spawn(async move {
			let (stream, _) = listener.accept().await.unwrap();
			let mut builder = http1::Builder::new();
			builder.keep_alive(false);
			builder
				.serve_connection(
					TokioIo::new(stream),
					service_fn(move |request| {
						let body = body.clone();
						async move {
							assert_eq!(request.method(), Method::POST);
							assert_eq!(request.uri().path(), PAGE_PATH);
							assert!(request.uri().query().is_none());
							assert_eq!(
								request
									.headers()
									.get(CONTENT_TYPE)
									.and_then(|value| value.to_str().ok()),
								Some(PEER_CONTENT_TYPE)
							);
							assert!(request
								.headers()
								.get(CONTENT_LENGTH)
								.and_then(|value| value.to_str().ok())
								.and_then(|value| value.parse::<usize>().ok())
								.is_some_and(|length| length > 0 && length <= MAX_REQUEST_ENCODED));
							assert!(request.headers().get(AUTHORIZATION).is_none());
							tokio::time::sleep(delay).await;
							Ok::<_, Infallible>(
								Response::builder()
									.status(status)
									.header(CONTENT_TYPE, content_type)
									.header(CONTENT_LENGTH, declared_length.to_string())
									.body(Full::new(Bytes::from(body)))
									.unwrap(),
							)
						}
					}),
				)
				.await
				.ok();
		});
		(endpoint, task)
	}

	#[tokio::test]
	async fn real_server_and_tls_capable_client_round_trip_exact_page_and_chunk() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let endpoint = format!("http://{}", listener.local_addr().unwrap());
		let topology = topology(endpoint.as_bytes().to_vec());
		let temp = TempDir::new().unwrap();
		let (commitment, _, content) = install_source(&temp);
		let responder = Arc::new(
			PeerResponder::new(
				Arc::new(MockAuthority { topology: topology.clone() }),
				Arc::new(CheckpointStack::open(temp.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		let server = tokio::spawn(serve_peer_http(listener, responder));
		let session = ReplicationSessionV1::from_topology(
			topology,
			[5; 32],
			pair(12).public().0,
			[4; 32],
			[5; 32],
			commitment,
		)
		.unwrap();
		let page_expectation = PeerPageExpectationV1::new(
			session.context().clone(),
			PeerRequestIdentityV1::new([15; 16], [16; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let page = PeerSyncPageRequestV1::new_signed(&page_expectation, &pair(12)).unwrap();
		let client = HyperPeerTransport::new(Duration::from_secs(5)).unwrap();
		let page_bytes = client.page(&session, &page.encode_wire()).await.unwrap();
		let response = PeerSyncPageResponseV1::decode_canonical(&page_bytes, &page).unwrap();
		let object = response.items()[0].clone();
		let chunk_expectation = PeerChunkExpectationV1::new(
			session.context().clone(),
			PeerRequestIdentityV1::new([15; 16], [17; 16]).unwrap(),
			object,
			1,
		)
		.unwrap();
		let chunk = PeerChunkRequestV1::new_signed(&chunk_expectation, &pair(12)).unwrap();
		let chunk_bytes = client.chunk(&session, &chunk.encode_wire()).await.unwrap();
		assert_eq!(
			PeerChunkResponseV1::decode_canonical(&chunk_bytes, &chunk)
				.unwrap()
				.verified_chunk()
				.2,
			&content[CHUNK_BYTES..]
		);
		server.abort();
	}

	#[tokio::test]
	async fn private_listener_rejects_every_non_exact_route_shape_without_details() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let endpoint = format!("http://{}", listener.local_addr().unwrap());
		let topology = topology(endpoint.as_bytes().to_vec());
		let temp = TempDir::new().unwrap();
		let _ = install_source(&temp);
		let responder = Arc::new(
			PeerResponder::new(
				Arc::new(MockAuthority { topology }),
				Arc::new(CheckpointStack::open(temp.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		let server = tokio::spawn(serve_peer_http(listener, responder));
		let client = HyperPeerTransport::new(Duration::from_secs(5)).unwrap();
		for (uri, method, content_type, status) in [
			(
				format!("{endpoint}{PAGE_PATH}"),
				Method::GET,
				Some(PEER_CONTENT_TYPE),
				StatusCode::NOT_FOUND,
			),
			(
				format!("{endpoint}{PAGE_PATH}?x=1"),
				Method::POST,
				Some(PEER_CONTENT_TYPE),
				StatusCode::NOT_FOUND,
			),
			(
				format!("{endpoint}/page"),
				Method::POST,
				Some(PEER_CONTENT_TYPE),
				StatusCode::NOT_FOUND,
			),
			(
				format!("{endpoint}{PAGE_PATH}"),
				Method::POST,
				Some("application/octet-stream"),
				StatusCode::UNSUPPORTED_MEDIA_TYPE,
			),
		] {
			let response = raw(&client.client, uri, method, content_type, &[0]).await;
			assert_eq!(response.status(), status);
			assert!(response.into_body().collect().await.unwrap().to_bytes().is_empty());
		}
		let response = raw(
			&client.client,
			format!("{endpoint}{PAGE_PATH}"),
			Method::POST,
			Some(PEER_CONTENT_TYPE),
			&[0],
		)
		.await;
		assert_eq!(response.status(), StatusCode::BAD_REQUEST);
		assert!(response.into_body().collect().await.unwrap().to_bytes().is_empty());
		let oversized = vec![0; MAX_REQUEST_ENCODED + 1];
		let response = raw(
			&client.client,
			format!("{endpoint}{CHUNK_PATH}"),
			Method::POST,
			Some(PEER_CONTENT_TYPE),
			&oversized,
		)
		.await;
		assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
		assert!(response.into_body().collect().await.unwrap().to_bytes().is_empty());
		for (header, value) in
			[(TRANSFER_ENCODING, "chunked"), (AUTHORIZATION, "Bearer must-not-be-used")]
		{
			let request = Request::builder()
				.method(Method::POST)
				.uri(format!("{endpoint}{PAGE_PATH}"))
				.header(CONTENT_TYPE, PEER_CONTENT_TYPE)
				.header(header, value)
				.body(Full::new(Bytes::from_static(&[0])))
				.unwrap();
			let response = client.client.request(request).await.unwrap();
			assert_eq!(response.status(), StatusCode::BAD_REQUEST);
			assert!(response.into_body().collect().await.unwrap().to_bytes().is_empty());
		}
		server.abort();
	}

	#[tokio::test]
	async fn client_rejects_status_media_length_and_timeout_without_credentials() {
		let cases = [
			(
				StatusCode::TEMPORARY_REDIRECT,
				PEER_CONTENT_TYPE,
				0,
				Vec::new(),
				Duration::ZERO,
				PeerTransportError::Refused,
			),
			(
				StatusCode::OK,
				"application/octet-stream",
				1,
				vec![0],
				Duration::ZERO,
				PeerTransportError::Response,
			),
			(
				StatusCode::OK,
				PEER_CONTENT_TYPE,
				MAX_PAGE_RESPONSE_ENCODED + 1,
				Vec::new(),
				Duration::ZERO,
				PeerTransportError::Response,
			),
		];
		for (status, media, length, body, delay, expected_error) in cases {
			let (endpoint, server) = static_peer(status, media, length, body, delay).await;
			let (session, request) = page_for_endpoint(endpoint.as_bytes());
			let client = HyperPeerTransport::new(Duration::from_secs(1)).unwrap();
			let error = client.page(&session, &request.encode_wire()).await.unwrap_err();
			assert_eq!(std::mem::discriminant(&error), std::mem::discriminant(&expected_error));
			server.await.unwrap();
		}

		let (endpoint, server) =
			static_peer(StatusCode::OK, PEER_CONTENT_TYPE, 1, vec![0], Duration::from_millis(100))
				.await;
		let (session, request) = page_for_endpoint(endpoint.as_bytes());
		let client = HyperPeerTransport::new(Duration::from_millis(10)).unwrap();
		assert!(matches!(
			client.page(&session, &request.encode_wire()).await,
			Err(PeerTransportError::Timeout)
		));
		server.abort();
	}

	#[tokio::test]
	async fn client_refuses_non_root_endpoint_and_never_sends_bearer_credentials() {
		let topology = topology(b"http://127.0.0.1:9/base".to_vec());
		let commitment = PeerMmrCommitmentV1::new([20; 32], 0, 1, 0).unwrap();
		let session = ReplicationSessionV1::from_topology(
			topology,
			[5; 32],
			pair(12).public().0,
			[4; 32],
			[5; 32],
			commitment,
		)
		.unwrap();
		let expected = PeerPageExpectationV1::new(
			session.context().clone(),
			PeerRequestIdentityV1::new([15; 16], [18; 16]).unwrap(),
			None,
			1,
		)
		.unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap();
		let client = HyperPeerTransport::new(Duration::from_secs(1)).unwrap();
		assert!(matches!(
			client.page(&session, &request.encode_wire()).await,
			Err(PeerTransportError::Endpoint)
		));
	}

	#[tokio::test]
	async fn peer_ingress_refuses_excess_connections_and_times_out_idle_peer() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let endpoint = format!("http://{address}");
		let topology = topology(endpoint.as_bytes().to_vec());
		let temp = TempDir::new().unwrap();
		let _ = install_source(&temp);
		let responder = Arc::new(
			PeerResponder::new(
				Arc::new(MockAuthority { topology }),
				Arc::new(CheckpointStack::open(temp.path()).unwrap()),
				[4; 32],
				pair(11),
			)
			.unwrap(),
		);
		let server = tokio::spawn(serve_peer_http_with_limits(
			listener,
			responder,
			1,
			Duration::from_millis(80),
		));
		let _idle = TcpStream::connect(address).await.unwrap();
		tokio::time::sleep(Duration::from_millis(10)).await;

		let mut excess = TcpStream::connect(address).await.unwrap();
		excess
			.write_all(b"GET /_cord/peer/v1/page HTTP/1.1\r\nHost: peer\r\n\r\n")
			.await
			.unwrap();
		let mut refused = [0; 1];
		let refused = tokio::time::timeout(Duration::from_millis(50), excess.read(&mut refused))
			.await
			.unwrap();
		assert!(matches!(refused, Ok(0) | Err(_)));

		tokio::time::sleep(Duration::from_millis(100)).await;
		let client = HyperPeerTransport::new(Duration::from_secs(1)).unwrap();
		let accepted =
			raw(&client.client, format!("{endpoint}/not-a-peer-route"), Method::GET, None, &[])
				.await;
		assert_eq!(accepted.status(), StatusCode::NOT_FOUND);
		server.abort();
	}
}
