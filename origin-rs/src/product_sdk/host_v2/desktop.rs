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

//! Private desktop binding for the canonical host-v2 protocol.

use std::{
	io::{self, Read, Write},
	path::PathBuf,
};

use ciborium::value::Value;

use crate::product_sdk::host_outbox::{
	HostOutboxBindingV1, HostOutboxError, HostOutboxRetryV1, HostOutboxStoreV1, PrepareHostOutboxV1,
};

use super::{
	codec::{CodecError, Dto},
	generated::{
		self, CancelledEventV2, OperationCode, Production, ProviderCapabilityV1, RequestV2,
		ResponseAckV1, ResumeTokenV1,
	},
	session::{negotiate, Negotiated, NegotiationError, NegotiationOffer, Session, SessionError},
};

pub(crate) const MAX_DESKTOP_FRAME_BYTES: usize = 4_194_304;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DesktopPeerIdentity {
	pub(crate) endpoint: PathBuf,
	pub(crate) process_id: Option<u32>,
	pub(crate) user_id: Option<u32>,
	pub(crate) provider_id: [u8; 32],
	pub(crate) provider_endpoint_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("DESKTOP_PEER_BINDING_REJECTED")]
pub(crate) struct DesktopPeerBindingError;

pub(crate) trait DesktopPeerBinding {
	fn bind(&self, peer: &DesktopPeerIdentity) -> Result<(), DesktopPeerBindingError>;
}

impl<F> DesktopPeerBinding for F
where
	F: Fn(&DesktopPeerIdentity) -> Result<(), DesktopPeerBindingError>,
{
	fn bind(&self, peer: &DesktopPeerIdentity) -> Result<(), DesktopPeerBindingError> {
		self(peer)
	}
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DesktopTransportError {
	#[error("DESKTOP_FRAME_TOO_LARGE")]
	FrameTooLarge,
	#[error("DESKTOP_FRAME_TRUNCATED")]
	FrameTruncated,
	#[error("DESKTOP_TRANSPORT_CLOSED")]
	Closed,
	#[error("DESKTOP_REQUEST_BINDING_INVALID")]
	RequestBinding,
	#[error(transparent)]
	Peer(#[from] DesktopPeerBindingError),
	#[error(transparent)]
	Negotiation(#[from] NegotiationError),
	#[error(transparent)]
	Codec(#[from] CodecError),
	#[error(transparent)]
	Session(#[from] SessionError),
	#[error(transparent)]
	Outbox(#[from] HostOutboxError),
	#[error("DESKTOP_IO: {0}")]
	Io(io::Error),
}

impl From<io::Error> for DesktopTransportError {
	fn from(error: io::Error) -> Self {
		Self::Io(error)
	}
}

/// Incremental parser used by local IPC adapters whose reads can split or coalesce frames.
pub(crate) struct DesktopFrameDecoder {
	buffer: Vec<u8>,
	closed: bool,
}

impl DesktopFrameDecoder {
	pub(crate) fn new() -> Self {
		Self { buffer: Vec::new(), closed: false }
	}

	pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, DesktopTransportError> {
		if self.closed {
			return Err(DesktopTransportError::Closed);
		}
		self.buffer.extend_from_slice(chunk);
		let mut frames = Vec::new();
		loop {
			if self.buffer.len() < 4 {
				break;
			}
			let length =
				u32::from_be_bytes(self.buffer[..4].try_into().expect("four bytes")) as usize;
			if length > MAX_DESKTOP_FRAME_BYTES {
				self.closed = true;
				self.buffer.clear();
				return Err(DesktopTransportError::FrameTooLarge);
			}
			let end = 4 + length;
			if self.buffer.len() < end {
				break;
			}
			frames.push(self.buffer[4..end].to_vec());
			self.buffer.drain(..end);
		}
		Ok(frames)
	}

	pub(crate) fn finish(&mut self) -> Result<(), DesktopTransportError> {
		self.closed = true;
		if self.buffer.is_empty() {
			Ok(())
		} else {
			self.buffer.clear();
			Err(DesktopTransportError::FrameTruncated)
		}
	}
}

pub(crate) fn write_frame(
	writer: &mut impl Write,
	payload: &[u8],
) -> Result<(), DesktopTransportError> {
	let length: u32 = payload.len().try_into().map_err(|_| DesktopTransportError::FrameTooLarge)?;
	if payload.len() > MAX_DESKTOP_FRAME_BYTES {
		return Err(DesktopTransportError::FrameTooLarge);
	}
	writer.write_all(&length.to_be_bytes())?;
	writer.write_all(payload)?;
	writer.flush()?;
	Ok(())
}

pub(crate) fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>, DesktopTransportError> {
	let mut header = [0u8; 4];
	read_exact_frame(reader, &mut header)?;
	let length = u32::from_be_bytes(header) as usize;
	if length > MAX_DESKTOP_FRAME_BYTES {
		return Err(DesktopTransportError::FrameTooLarge);
	}
	let mut payload = vec![0; length];
	read_exact_frame(reader, &mut payload)?;
	Ok(payload)
}

fn read_exact_frame(
	reader: &mut impl Read,
	mut target: &mut [u8],
) -> Result<(), DesktopTransportError> {
	while !target.is_empty() {
		match reader.read(target) {
			Ok(0) => return Err(DesktopTransportError::FrameTruncated),
			Ok(read) => target = &mut target[read..],
			Err(error) if error.kind() == io::ErrorKind::Interrupted => {},
			Err(error) => return Err(error.into()),
		}
	}
	Ok(())
}

/// A desktop stream cannot exist in an unbound or unnegotiated state.
pub(crate) struct DesktopHostV2Transport<S> {
	stream: S,
	negotiated: Negotiated,
	session: Option<Session>,
	closed: bool,
	binding: HostOutboxBindingV1,
	_peer: DesktopPeerIdentity,
}

impl<S: Read + Write> DesktopHostV2Transport<S> {
	pub(crate) fn connect(
		stream: S,
		peer: &DesktopPeerIdentity,
		binding: &impl DesktopPeerBinding,
		local: &NegotiationOffer,
		remote: &NegotiationOffer,
	) -> Result<Self, DesktopTransportError> {
		binding.bind(peer)?;
		let negotiated = negotiate(local, remote)?;
		let transport_binding = HostOutboxBindingV1 {
			registry_hash: negotiated.registry_hash(),
			genesis_hash: negotiated.genesis(),
			negotiated_tuple: negotiated.binding_digest(),
			provider_id: peer.provider_id,
			provider_endpoint_hash: peer.provider_endpoint_hash,
			request_id: [0; 16],
			operation_id: [0; 16],
			expected_response_kind: 0,
			intended_cursor: 0,
		};
		Ok(Self {
			stream,
			negotiated,
			session: None,
			closed: false,
			binding: transport_binding,
			_peer: peer.clone(),
		})
	}

	fn begin(&mut self, request_id: [u8; 16]) -> Result<(), DesktopTransportError> {
		if self.closed || self.session.is_some() {
			return Err(DesktopTransportError::Closed);
		}
		self.session = Some(Session::new(self.negotiated.clone(), request_id));
		Ok(())
	}

	fn resume_session(
		&mut self,
		request_id: [u8; 16],
		next_sequence: u32,
	) -> Result<(), DesktopTransportError> {
		if self.closed || self.session.is_some() {
			return Err(DesktopTransportError::Closed);
		}
		self.session = Some(Session::resume(self.negotiated.clone(), request_id, next_sequence));
		Ok(())
	}

	fn send(&mut self, request: &[u8], authority: &[u8]) -> Result<(), DesktopTransportError> {
		if self.closed || self.session.is_none() {
			return Err(DesktopTransportError::Closed);
		}
		if let Err(error) = write_frame(&mut self.stream, request)
			.and_then(|()| write_frame(&mut self.stream, authority))
		{
			self.closed = true;
			return Err(error);
		}
		Ok(())
	}

	fn receive(&mut self) -> Result<(Vec<u8>, bool), DesktopTransportError> {
		if self.closed {
			return Err(DesktopTransportError::Closed);
		}
		let bytes = match read_frame(&mut self.stream) {
			Ok(bytes) => bytes,
			Err(error) => {
				self.closed = true;
				return Err(error);
			},
		};
		let session = self.session.as_mut().ok_or(DesktopTransportError::Closed)?;
		if let Err(error) = session.accept(&bytes) {
			self.closed = true;
			return Err(error.into());
		}
		Ok((bytes, session.is_terminal()))
	}

	fn send_ack(&mut self, ack: &[u8]) -> Result<(), DesktopTransportError> {
		Dto::<ResponseAckV1>::decode(ack)?;
		if let Err(error) = write_frame(&mut self.stream, ack) {
			self.closed = true;
			return Err(error);
		}
		Ok(())
	}

	fn finish_terminal(&mut self) -> Result<(), DesktopTransportError> {
		if !self.session.as_ref().is_some_and(Session::is_terminal) {
			return Err(DesktopTransportError::RequestBinding);
		}
		self.session = None;
		Ok(())
	}

	fn close(&mut self) {
		self.closed = true;
	}
}

/// Couples the desktop stream to the encrypted outbox at every loss boundary.
pub(crate) struct DurableDesktopHostV2<'a, S> {
	transport: DesktopHostV2Transport<S>,
	outbox: &'a HostOutboxStoreV1,
	active: Option<ActiveDesktopRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveDesktopRequest {
	outbox_id: [u8; 16],
	request_id: [u8; 16],
	operation_id: [u8; 16],
	expected_response_kind: u16,
	operation: Option<OperationCode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DurableDesktopEvent {
	NonTerminal(Vec<u8>),
	Terminal { event: Vec<u8>, response_hash: [u8; 32] },
}

impl<'a, S: Read + Write> DurableDesktopHostV2<'a, S> {
	pub(crate) fn new(transport: DesktopHostV2Transport<S>, outbox: &'a HostOutboxStoreV1) -> Self {
		Self { transport, outbox, active: None }
	}

	/// Validate first, fsync Prepared, and only then write the first provider-visible byte.
	pub(crate) fn prepare_and_send(
		&mut self,
		input: PrepareHostOutboxV1,
		prepare_nonce: [u8; 24],
		mark_sent_nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, DesktopTransportError> {
		self.validate_context()?;
		let material = validate_wire_material(
			&input.exact_request_bytes,
			&input.exact_authority_bytes,
			&self.transport.binding,
		)?;
		if material.request_id != input.request_id ||
			material.operation_id.is_some_and(|id| id != input.operation_id) ||
			input.negotiated_tuple != self.transport.binding.negotiated_tuple ||
			input.provider_id != self.transport.binding.provider_id ||
			input.provider_endpoint_hash != self.transport.binding.provider_endpoint_hash
		{
			return Err(DesktopTransportError::RequestBinding);
		}
		let outbox_id = input.outbox_id;
		let active = ActiveDesktopRequest {
			outbox_id,
			request_id: input.request_id,
			operation_id: input.operation_id,
			expected_response_kind: input.expected_response_kind,
			operation: Some(material.operation.ok_or(DesktopTransportError::RequestBinding)?),
		};
		let retry = self.outbox.prepare(input, prepare_nonce)?;
		self.transport.begin(material.request_id)?;
		self.transport.send(&retry.request, &retry.authority)?;
		if let Err(error) = self.outbox.mark_sent(outbox_id, mark_sent_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		self.active = Some(active);
		Ok(retry)
	}

	/// Restart from only the exact durable request and authority bytes.
	pub(crate) fn resume_and_send(
		&mut self,
		outbox_id: [u8; 16],
		finalized: u64,
	) -> Result<HostOutboxRetryV1, DesktopTransportError> {
		self.validate_context()?;
		let binding = self.outbox.binding(outbox_id)?;
		self.validate_outbox_binding(&binding)?;
		let retry = self.outbox.retry_request(outbox_id, finalized)?;
		let material =
			validate_wire_material(&retry.request, &retry.authority, &self.transport.binding)?;
		if material.request_id != binding.request_id ||
			material.operation_id.is_some_and(|id| id != binding.operation_id)
		{
			return Err(DesktopTransportError::RequestBinding);
		}
		if material.cancel {
			self.transport.resume_session(binding.request_id, binding.intended_cursor)?;
		} else {
			self.transport.begin(binding.request_id)?;
		}
		self.transport.send(&retry.request, &retry.authority)?;
		self.active = Some(ActiveDesktopRequest {
			outbox_id,
			request_id: binding.request_id,
			operation_id: binding.operation_id,
			expected_response_kind: binding.expected_response_kind,
			operation: material.operation,
		});
		Ok(retry)
	}

	/// Durably replace the live request with an idempotent cancel before sending it.
	pub(crate) fn prepare_cancel_and_send(
		&mut self,
		exact_cancel_bytes: Vec<u8>,
		prepare_nonce: [u8; 24],
		mark_sent_nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, DesktopTransportError> {
		let active = self.active.clone().ok_or(DesktopTransportError::Closed)?;
		let cancel = Dto::<CancelledEventV2>::decode(&exact_cancel_bytes)?;
		let (request_id, sequence, kind) = event_contract(cancel.value())?;
		let next_sequence = self
			.transport
			.session
			.as_ref()
			.ok_or(DesktopTransportError::Closed)?
			.next_sequence();
		if request_id != active.request_id || sequence != next_sequence || kind != 4 {
			return Err(DesktopTransportError::RequestBinding);
		}
		let retry = self.outbox.prepare_cancel(
			active.outbox_id,
			exact_cancel_bytes,
			next_sequence,
			prepare_nonce,
		)?;
		self.transport.send(&retry.request, &retry.authority)?;
		if let Err(error) = self.outbox.mark_sent(active.outbox_id, mark_sent_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		self.active =
			Some(ActiveDesktopRequest { expected_response_kind: 4, operation: None, ..active });
		Ok(retry)
	}

	/// Receive one event, always installing a terminal response before its acknowledgement can
	/// leave.
	pub(crate) fn receive_event(
		&mut self,
		terminal_block: u64,
		install_nonce: [u8; 24],
		mark_ack_nonce: [u8; 24],
	) -> Result<DurableDesktopEvent, DesktopTransportError> {
		let active = self.active.clone().ok_or(DesktopTransportError::Closed)?;
		let binding = self.outbox.binding(active.outbox_id)?;
		self.validate_outbox_binding(&binding)?;
		if binding.request_id != active.request_id ||
			binding.operation_id != active.operation_id ||
			binding.expected_response_kind != active.expected_response_kind
		{
			return Err(DesktopTransportError::RequestBinding);
		}
		let (response, terminal) = self.transport.receive()?;
		if !terminal {
			return Ok(DurableDesktopEvent::NonTerminal(response));
		}
		let event_dto = Dto::<super::generated::EventV2>::decode(&response)?;
		let (_, _, kind) = event_contract(event_dto.value())?;
		if validate_terminal_contract(
			active.operation,
			active.expected_response_kind,
			kind,
			event_dto.value(),
		)
		.is_err()
		{
			self.transport.close();
			return Err(DesktopTransportError::RequestBinding);
		}
		let event = response.clone();
		let response_hash = match self.outbox.install_response(
			active.outbox_id,
			response,
			None,
			None,
			Some(terminal_block),
			install_nonce,
		) {
			Ok(hash) => hash,
			Err(error) => {
				self.transport.close();
				return Err(error.into());
			},
		};
		let ack = match self.outbox.retry_response_ack(active.outbox_id) {
			Ok(ack) => ack,
			Err(error) => {
				self.transport.close();
				return Err(error.into());
			},
		};
		self.transport.send_ack(&ack.bytes)?;
		if let Err(error) = self.outbox.mark_ack_sent(active.outbox_id, mark_ack_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		self.transport.finish_terminal()?;
		self.active = None;
		Ok(DurableDesktopEvent::Terminal { event, response_hash })
	}

	/// Repeat only the acknowledgement bytes installed with the response after a restart.
	pub(crate) fn resume_ack(
		&mut self,
		outbox_id: [u8; 16],
		mark_ack_nonce: [u8; 24],
	) -> Result<[u8; 32], DesktopTransportError> {
		self.validate_context()?;
		let binding = self.outbox.binding(outbox_id)?;
		self.validate_outbox_binding(&binding)?;
		let ack = self.outbox.retry_response_ack(outbox_id)?;
		self.transport.send_ack(&ack.bytes)?;
		if let Err(error) = self.outbox.mark_ack_sent(outbox_id, mark_ack_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		Ok(ack.response_hash)
	}

	/// Provider confirmation is durable before terminal ciphertext is compacted or later GC'd.
	pub(crate) fn confirm_terminal(
		&self,
		outbox_id: [u8; 16],
		response_hash: [u8; 32],
		confirm_nonce: [u8; 24],
		compact_nonce: [u8; 24],
	) -> Result<(), DesktopTransportError> {
		let binding = self.outbox.binding(outbox_id)?;
		self.validate_outbox_binding(&binding)?;
		self.outbox.confirm_ack(outbox_id, response_hash, confirm_nonce)?;
		self.outbox.compact_acknowledged(outbox_id, compact_nonce)?;
		Ok(())
	}

	fn validate_context(&self) -> Result<(), DesktopTransportError> {
		let (registry, genesis) = self.outbox.context_binding();
		if registry != self.transport.binding.registry_hash ||
			genesis != self.transport.binding.genesis_hash
		{
			return Err(DesktopTransportError::RequestBinding);
		}
		Ok(())
	}

	fn validate_outbox_binding(
		&self,
		binding: &HostOutboxBindingV1,
	) -> Result<(), DesktopTransportError> {
		self.validate_context()?;
		if binding.registry_hash != self.transport.binding.registry_hash ||
			binding.genesis_hash != self.transport.binding.genesis_hash ||
			binding.negotiated_tuple != self.transport.binding.negotiated_tuple ||
			binding.provider_id != self.transport.binding.provider_id ||
			binding.provider_endpoint_hash != self.transport.binding.provider_endpoint_hash
		{
			return Err(DesktopTransportError::RequestBinding);
		}
		Ok(())
	}
}

struct WireMaterial {
	request_id: [u8; 16],
	operation_id: Option<[u8; 16]>,
	cancel: bool,
	operation: Option<OperationCode>,
}

fn validate_wire_material(
	request: &[u8],
	authority: &[u8],
	binding: &HostOutboxBindingV1,
) -> Result<WireMaterial, DesktopTransportError> {
	let authority = if let Ok(capability) = Dto::<ProviderCapabilityV1>::decode(authority) {
		(capability.value().clone(), 8)
	} else if let Ok(resume) = Dto::<ResumeTokenV1>::decode(authority) {
		(resume.value().clone(), 3)
	} else {
		return Err(DesktopTransportError::RequestBinding);
	};
	if fixed_field(&authority.0, 1, 32)? != binding.registry_hash ||
		fixed_field(&authority.0, 2, 32)? != binding.genesis_hash ||
		fixed_field(&authority.0, authority.1, 32)? != binding.provider_id
	{
		return Err(DesktopTransportError::RequestBinding);
	}
	if let Ok(request) = Dto::<RequestV2>::decode(request) {
		let request_id = fixed_field(request.value(), 1, 16)?
			.try_into()
			.map_err(|_| DesktopTransportError::RequestBinding)?;
		let operation_id = optional_fixed_field(request.value(), 5, 16)?
			.map(|bytes| bytes.try_into().expect("length checked"));
		let operation = uint_field(request.value(), 3)
			.and_then(|value| u16::try_from(value).ok())
			.and_then(OperationCode::from_u16)
			.ok_or(DesktopTransportError::RequestBinding)?;
		return Ok(WireMaterial {
			request_id,
			operation_id,
			cancel: false,
			operation: Some(operation),
		});
	}
	let cancel = Dto::<CancelledEventV2>::decode(request)?;
	let (request_id, _, kind) = event_contract(cancel.value())?;
	if kind != 4 {
		return Err(DesktopTransportError::RequestBinding);
	}
	Ok(WireMaterial { request_id, operation_id: None, cancel: true, operation: None })
}

fn uint_field(value: &Value, wanted: u64) -> Option<u64> {
	let Value::Map(fields) = value else { return None };
	fields.iter().find_map(|(key, value)| match (key, value) {
		(Value::Integer(key), Value::Integer(value))
			if u64::try_from(*key).ok() == Some(wanted) =>
			u64::try_from(*value).ok(),
		_ => None,
	})
}

fn value_field(value: &Value, wanted: u64) -> Result<&Value, DesktopTransportError> {
	let Value::Map(fields) = value else { return Err(DesktopTransportError::RequestBinding) };
	fields
		.iter()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
				.then_some(value)
		})
		.ok_or(DesktopTransportError::RequestBinding)
}

fn validate_production<P: Production>(value: &Value) -> Result<(), DesktopTransportError> {
	Dto::<P>::from_value(value.clone())
		.map(|_| ())
		.map_err(|_| DesktopTransportError::RequestBinding)
}

macro_rules! operation_payload_table {
	($operation:expr, $payload:expr, $validator:ident) => {
		match $operation {
			OperationCode::StorageBucketCreate => {
				$validator!($payload, StorageBucketCreateResult, StorageBucketCreateError)
			},
			OperationCode::StorageBucketGet => {
				$validator!($payload, StorageBucketGetResult, StorageBucketGetError)
			},
			OperationCode::StorageBucketGrant => {
				$validator!($payload, StorageBucketGrantResult, StorageBucketGrantError)
			},
			OperationCode::StorageBucketRevoke => {
				$validator!($payload, StorageBucketRevokeResult, StorageBucketRevokeError)
			},
			OperationCode::StorageObjectPut => {
				$validator!($payload, StorageObjectPutResult, StorageObjectPutError)
			},
			OperationCode::StorageObjectGet => {
				$validator!($payload, StorageObjectGetResult, StorageObjectGetError)
			},
			OperationCode::StorageObjectRange => {
				$validator!($payload, StorageObjectRangeResult, StorageObjectRangeError)
			},
			OperationCode::StorageObjectDelete => {
				$validator!($payload, StorageObjectDeleteResult, StorageObjectDeleteError)
			},
			OperationCode::StorageObjectStatus => {
				$validator!($payload, StorageObjectStatusResult, StorageObjectStatusError)
			},
			OperationCode::StorageCheckpointStatus => {
				$validator!($payload, StorageCheckpointStatusResult, StorageCheckpointStatusError)
			},
			OperationCode::StorageCheckpointSubscribe => $validator!(
				$payload,
				StorageCheckpointSubscribeResult,
				StorageCheckpointSubscribeError
			),
			OperationCode::StorageReplicaStatus => {
				$validator!($payload, StorageReplicaStatusResult, StorageReplicaStatusError)
			},
			OperationCode::StorageReplicaSubscribe => {
				$validator!($payload, StorageReplicaSubscribeResult, StorageReplicaSubscribeError)
			},
			OperationCode::StorageDeletionStatus => {
				$validator!($payload, StorageDeletionStatusResult, StorageDeletionStatusError)
			},
			OperationCode::StorageDeletionSubscribe => {
				$validator!($payload, StorageDeletionSubscribeResult, StorageDeletionSubscribeError)
			},
			OperationCode::StorageDriveRead => {
				$validator!($payload, StorageDriveReadResult, StorageDriveReadError)
			},
			OperationCode::StorageDriveCommit => {
				$validator!($payload, StorageDriveCommitResult, StorageDriveCommitError)
			},
			OperationCode::StorageDriveShare => {
				$validator!($payload, StorageDriveShareResult, StorageDriveShareError)
			},
			OperationCode::StorageS3Put => {
				$validator!($payload, StorageS3PutResult, StorageS3PutError)
			},
			OperationCode::StorageS3Get => {
				$validator!($payload, StorageS3GetResult, StorageS3GetError)
			},
			OperationCode::StorageS3List => {
				$validator!($payload, StorageS3ListResult, StorageS3ListError)
			},
			OperationCode::StorageS3Delete => {
				$validator!($payload, StorageS3DeleteResult, StorageS3DeleteError)
			},
			OperationCode::StoragePublish => {
				$validator!($payload, StoragePublishResult, StoragePublishError)
			},
			OperationCode::StorageResolve => {
				$validator!($payload, StorageResolveResult, StorageResolveError)
			},
			OperationCode::StorageKeysExport => {
				$validator!($payload, StorageKeysExportResult, StorageKeysExportError)
			},
			OperationCode::StorageKeysImport => {
				$validator!($payload, StorageKeysImportResult, StorageKeysImportError)
			},
			OperationCode::IdentityAccount => {
				$validator!($payload, IdentityAccountResult, IdentityAccountError)
			},
			OperationCode::IdentityProfileRead => {
				$validator!($payload, IdentityProfileReadResult, IdentityProfileReadError)
			},
			OperationCode::IdentityProfileDisclose => {
				$validator!($payload, IdentityProfileDiscloseResult, IdentityProfileDiscloseError)
			},
			OperationCode::IdentityHumanityStatus => {
				$validator!($payload, IdentityHumanityStatusResult, IdentityHumanityStatusError)
			},
			OperationCode::IdentityHumanityProve => {
				$validator!($payload, IdentityHumanityProveResult, IdentityHumanityProveError)
			},
			OperationCode::IdentitySubjectDerive => {
				$validator!($payload, IdentitySubjectDeriveResult, IdentitySubjectDeriveError)
			},
			OperationCode::IdentityEntitlementsRead => {
				$validator!($payload, IdentityEntitlementsReadResult, IdentityEntitlementsReadError)
			},
			OperationCode::TransactionSign => {
				$validator!($payload, TransactionSignResult, TransactionSignError)
			},
		}
	};
}

macro_rules! validate_result {
	($payload:expr, $result:ident, $error:ident) => {
		validate_production::<generated::$result>($payload)
	};
}

macro_rules! validate_error {
	($payload:expr, $result:ident, $error:ident) => {
		validate_production::<generated::$error>($payload)
	};
}

fn validate_terminal_contract(
	operation: Option<OperationCode>,
	expected_response_kind: u16,
	kind: u64,
	event: &Value,
) -> Result<(), DesktopTransportError> {
	match (expected_response_kind, kind) {
		(2, 2) => operation_payload_table!(
			operation.ok_or(DesktopTransportError::RequestBinding)?,
			value_field(event, 4)?,
			validate_result
		),
		(2, 3) => {
			let operation = operation.ok_or(DesktopTransportError::RequestBinding)?;
			let payload = value_field(event, 4)?;
			operation_payload_table!(operation, payload, validate_error)?;
			let error_code = uint_field(payload, 0)
				.and_then(|value| u16::try_from(value).ok())
				.ok_or(DesktopTransportError::RequestBinding)?;
			let binding = generated::OPERATIONS
				.iter()
				.find(|binding| binding.code == operation as u16)
				.ok_or(DesktopTransportError::RequestBinding)?;
			if !binding.allowed_errors.contains(&error_code) {
				return Err(DesktopTransportError::RequestBinding);
			}
			Ok(())
		},
		(4, 4) if operation.is_none() => Ok(()),
		_ => Err(DesktopTransportError::RequestBinding),
	}
}

fn event_contract(value: &Value) -> Result<([u8; 16], u32, u64), DesktopTransportError> {
	let request_id: [u8; 16] = fixed_field(value, 1, 16)?
		.try_into()
		.map_err(|_| DesktopTransportError::RequestBinding)?;
	let Value::Map(fields) = value else { return Err(DesktopTransportError::RequestBinding) };
	let uint = |wanted| {
		fields.iter().find_map(|(key, value)| match (key, value) {
			(Value::Integer(key), Value::Integer(value))
				if u64::try_from(*key).ok() == Some(wanted) =>
				u64::try_from(*value).ok(),
			_ => None,
		})
	};
	let sequence = uint(2)
		.and_then(|value| value.try_into().ok())
		.ok_or(DesktopTransportError::RequestBinding)?;
	let kind = uint(3).ok_or(DesktopTransportError::RequestBinding)?;
	Ok((request_id, sequence, kind))
}

fn fixed_field(value: &Value, key: u64, length: usize) -> Result<Vec<u8>, DesktopTransportError> {
	optional_fixed_field(value, key, length)?.ok_or(DesktopTransportError::RequestBinding)
}

fn optional_fixed_field(
	value: &Value,
	key: u64,
	length: usize,
) -> Result<Option<Vec<u8>>, DesktopTransportError> {
	let Value::Map(fields) = value else { return Err(DesktopTransportError::RequestBinding) };
	for (candidate, value) in fields {
		if matches!(candidate, Value::Integer(candidate) if u64::try_from(*candidate).ok() == Some(key))
		{
			let Value::Bytes(bytes) = value else {
				return Err(DesktopTransportError::RequestBinding);
			};
			if bytes.len() != length {
				return Err(DesktopTransportError::RequestBinding);
			}
			return Ok(Some(bytes.clone()));
		}
	}
	Ok(None)
}
