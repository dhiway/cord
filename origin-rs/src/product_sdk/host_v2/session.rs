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

use std::collections::BTreeSet;

use ciborium::value::Value;

use super::{
	codec::{CodecError, Dto},
	generated::{EventV2, FEATURE_IDS, MAJOR, MINOR, PROTOCOL, REGISTRY_SHA256},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NegotiationOffer {
	pub protocol: String,
	pub major: u8,
	pub minors: Vec<u16>,
	pub genesis: [u8; 32],
	pub finalized_spec_version: u32,
	pub finalized_transaction_version: u32,
	pub registry_sha256: String,
	pub features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Negotiated {
	protocol: &'static str,
	major: u8,
	minor: u16,
	genesis: [u8; 32],
	finalized_spec_version: u32,
	finalized_transaction_version: u32,
	registry_sha256: &'static str,
	features: Vec<&'static str>,
}

impl Negotiated {
	pub(crate) fn minor(&self) -> u16 {
		self.minor
	}

	pub(crate) fn features(&self) -> &[&'static str] {
		&self.features
	}
}

#[derive(Clone, Copy, Debug, thiserror::Error, Eq, PartialEq)]
pub(crate) enum NegotiationError {
	#[error("WIRE_VERSION_MISMATCH")]
	Version,
	#[error("WIRE_GENESIS_MISMATCH")]
	Genesis,
	#[error("WIRE_DESCRIPTOR_MISMATCH")]
	Descriptor,
}

fn validate_offer(offer: &NegotiationOffer) -> Result<(), NegotiationError> {
	if offer.protocol != PROTOCOL || offer.major != MAJOR || offer.minors.is_empty() {
		return Err(NegotiationError::Version);
	}
	let mut minors = BTreeSet::new();
	if offer.minors.iter().any(|minor| *minor > MINOR || !minors.insert(*minor)) {
		return Err(NegotiationError::Version);
	}
	if offer.registry_sha256 != REGISTRY_SHA256 {
		return Err(NegotiationError::Descriptor);
	}
	let mut features = BTreeSet::new();
	if offer.features.iter().any(|feature| {
		!FEATURE_IDS.contains(&feature.as_str()) || !features.insert(feature.as_str())
	}) {
		return Err(NegotiationError::Descriptor);
	}
	Ok(())
}

pub(crate) fn negotiate(
	local: &NegotiationOffer,
	remote: &NegotiationOffer,
) -> Result<Negotiated, NegotiationError> {
	validate_offer(local)?;
	validate_offer(remote)?;
	if local.genesis != remote.genesis {
		return Err(NegotiationError::Genesis);
	}
	if local.finalized_spec_version != remote.finalized_spec_version
		|| local.finalized_transaction_version != remote.finalized_transaction_version
	{
		return Err(NegotiationError::Version);
	}
	let remote_minors: BTreeSet<_> = remote.minors.iter().copied().collect();
	let minor = local
		.minors
		.iter()
		.copied()
		.filter(|minor| remote_minors.contains(minor))
		.max()
		.ok_or(NegotiationError::Version)?;
	let features = FEATURE_IDS
		.iter()
		.copied()
		.filter(|feature| {
			local.features.iter().any(|candidate| candidate == feature)
				&& remote.features.iter().any(|candidate| candidate == feature)
		})
		.collect();
	Ok(Negotiated {
		protocol: PROTOCOL,
		major: MAJOR,
		minor,
		genesis: local.genesis,
		finalized_spec_version: local.finalized_spec_version,
		finalized_transaction_version: local.finalized_transaction_version,
		registry_sha256: REGISTRY_SHA256,
		features,
	})
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum SessionError {
	#[error(transparent)]
	Codec(#[from] CodecError),
	#[error("WIRE_SEQUENCE_INVALID: {0}")]
	Sequence(String),
}

pub(crate) struct Session {
	request_id: [u8; 16],
	next_sequence: u64,
	accepted: bool,
	terminal: bool,
	closed: bool,
	negotiated: Negotiated,
}

impl Session {
	pub(crate) fn new(negotiated: Negotiated, request_id: [u8; 16]) -> Self {
		Self {
			request_id,
			next_sequence: 0,
			accepted: false,
			terminal: false,
			closed: false,
			negotiated,
		}
	}

	pub(crate) fn is_terminal(&self) -> bool {
		self.terminal
	}

	pub(crate) fn is_closed(&self) -> bool {
		self.closed
	}

	pub(crate) fn negotiated(&self) -> &Negotiated {
		&self.negotiated
	}

	pub(crate) fn accept(&mut self, bytes: &[u8]) -> Result<Dto<EventV2>, SessionError> {
		if self.closed {
			return sequence("host-v2 session is permanently closed");
		}
		let event = match Dto::<EventV2>::decode(bytes) {
			Ok(event) => event,
			Err(error) => {
				self.closed = true;
				return Err(SessionError::Codec(error));
			},
		};
		let Value::Map(fields) = event.value() else { return sequence("event is not a map") };
		let field = |wanted| {
			fields.iter().find_map(|(key, value)| {
				matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
					.then_some(value)
			})
		};
		let request_id = match field(1) {
			Some(Value::Bytes(value)) => value,
			_ => return sequence("event request ID missing"),
		};
		if request_id.as_slice() != self.request_id {
			return self.sequence_fault("event request ID does not match session");
		}
		let sequence_number = match field(2) {
			Some(Value::Integer(value)) => u64::try_from(*value).ok(),
			_ => None,
		}
		.ok_or_else(|| SessionError::Sequence("event sequence missing".into()))?;
		if sequence_number != self.next_sequence {
			return self.sequence_fault("event sequence is duplicated or skipped");
		}
		let kind = match field(3) {
			Some(Value::Integer(value)) => u64::try_from(*value).ok(),
			_ => None,
		}
		.ok_or_else(|| SessionError::Sequence("event kind missing".into()))?;
		if sequence_number == 0 && kind != 0 {
			return self.sequence_fault("first event must be accepted");
		}
		if sequence_number != 0 && kind == 0 {
			return self.sequence_fault("accepted event may occur only once");
		}
		if kind == 0 {
			self.accepted = true;
		}
		if !self.accepted {
			return self.sequence_fault("event preceded acceptance");
		}
		self.next_sequence += 1;
		self.terminal = matches!(kind, 2 | 3 | 4);
		self.closed = self.terminal;
		Ok(event)
	}

	fn sequence_fault<T>(&mut self, message: &str) -> Result<T, SessionError> {
		self.closed = true;
		sequence(message)
	}
}

fn sequence<T>(message: &str) -> Result<T, SessionError> {
	Err(SessionError::Sequence(message.into()))
}
