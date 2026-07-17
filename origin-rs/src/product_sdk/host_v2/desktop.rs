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
	HostOutboxError, HostOutboxRetryV1, HostOutboxStoreV1, PrepareHostOutboxV1,
};

use super::{
	codec::{CodecError, Dto},
	generated::{ProviderCapabilityV1, RequestV2, ResponseAckV1, ResumeTokenV1},
	session::{negotiate, Negotiated, NegotiationError, NegotiationOffer, Session, SessionError},
};

pub(crate) const MAX_DESKTOP_FRAME_BYTES: usize = 4_194_304;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DesktopPeerIdentity {
	pub(crate) endpoint: PathBuf,
	pub(crate) process_id: Option<u32>,
	pub(crate) user_id: Option<u32>,
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
		Ok(Self { stream, negotiated, session: None, closed: false })
	}

	fn begin(&mut self, request_id: [u8; 16]) -> Result<(), DesktopTransportError> {
		if self.closed || self.session.is_some() {
			return Err(DesktopTransportError::Closed);
		}
		self.session = Some(Session::new(self.negotiated.clone(), request_id));
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

	fn close(&mut self) {
		self.closed = true;
	}
}

/// Couples the desktop stream to the encrypted outbox at every loss boundary.
pub(crate) struct DurableDesktopHostV2<'a, S> {
	transport: DesktopHostV2Transport<S>,
	outbox: &'a HostOutboxStoreV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DurableDesktopEvent {
	NonTerminal(Vec<u8>),
	Terminal { event: Vec<u8>, response_hash: [u8; 32] },
}

impl<'a, S: Read + Write> DurableDesktopHostV2<'a, S> {
	pub(crate) fn new(transport: DesktopHostV2Transport<S>, outbox: &'a HostOutboxStoreV1) -> Self {
		Self { transport, outbox }
	}

	/// Validate first, fsync Prepared, and only then write the first provider-visible byte.
	pub(crate) fn prepare_and_send(
		&mut self,
		input: PrepareHostOutboxV1,
		prepare_nonce: [u8; 24],
		mark_sent_nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, DesktopTransportError> {
		let request_id =
			validate_wire_material(&input.exact_request_bytes, &input.exact_authority_bytes)?;
		if request_id != input.request_id {
			return Err(DesktopTransportError::RequestBinding);
		}
		let outbox_id = input.outbox_id;
		let retry = self.outbox.prepare(input, prepare_nonce)?;
		self.transport.begin(request_id)?;
		self.transport.send(&retry.request, &retry.authority)?;
		if let Err(error) = self.outbox.mark_sent(outbox_id, mark_sent_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		Ok(retry)
	}

	/// Restart from only the exact durable request and authority bytes.
	pub(crate) fn resume_and_send(
		&mut self,
		outbox_id: [u8; 16],
		finalized: u64,
	) -> Result<HostOutboxRetryV1, DesktopTransportError> {
		let retry = self.outbox.retry_request(outbox_id, finalized)?;
		let request_id = validate_wire_material(&retry.request, &retry.authority)?;
		self.transport.begin(request_id)?;
		self.transport.send(&retry.request, &retry.authority)?;
		Ok(retry)
	}

	/// Receive one event, always installing a terminal response before its acknowledgement can
	/// leave.
	pub(crate) fn receive_event(
		&mut self,
		outbox_id: [u8; 16],
		terminal_block: u64,
		install_nonce: [u8; 24],
		mark_ack_nonce: [u8; 24],
	) -> Result<DurableDesktopEvent, DesktopTransportError> {
		let (response, terminal) = self.transport.receive()?;
		if !terminal {
			return Ok(DurableDesktopEvent::NonTerminal(response));
		}
		let event = response.clone();
		let response_hash = match self.outbox.install_response(
			outbox_id,
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
		let ack = match self.outbox.retry_response_ack(outbox_id) {
			Ok(ack) => ack,
			Err(error) => {
				self.transport.close();
				return Err(error.into());
			},
		};
		self.transport.send_ack(&ack.bytes)?;
		if let Err(error) = self.outbox.mark_ack_sent(outbox_id, mark_ack_nonce) {
			self.transport.close();
			return Err(error.into());
		}
		Ok(DurableDesktopEvent::Terminal { event, response_hash })
	}

	/// Repeat only the acknowledgement bytes installed with the response after a restart.
	pub(crate) fn resume_ack(
		&mut self,
		outbox_id: [u8; 16],
		mark_ack_nonce: [u8; 24],
	) -> Result<[u8; 32], DesktopTransportError> {
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
		self.outbox.confirm_ack(outbox_id, response_hash, confirm_nonce)?;
		self.outbox.compact_acknowledged(outbox_id, compact_nonce)?;
		Ok(())
	}
}

fn validate_wire_material(
	request: &[u8],
	authority: &[u8],
) -> Result<[u8; 16], DesktopTransportError> {
	let request = Dto::<RequestV2>::decode(request)?;
	if Dto::<ProviderCapabilityV1>::decode(authority).is_err() &&
		Dto::<ResumeTokenV1>::decode(authority).is_err()
	{
		return Err(DesktopTransportError::RequestBinding);
	}
	let Value::Map(fields) = request.value() else {
		return Err(DesktopTransportError::RequestBinding);
	};
	let request_id = fields.iter().find_map(|(key, value)| match (key, value) {
		(Value::Integer(key), Value::Bytes(bytes))
			if u64::try_from(*key).ok() == Some(1) && bytes.len() == 16 =>
			Some(bytes.clone()),
		_ => None,
	});
	request_id
		.and_then(|bytes| bytes.try_into().ok())
		.ok_or(DesktopTransportError::RequestBinding)
}
