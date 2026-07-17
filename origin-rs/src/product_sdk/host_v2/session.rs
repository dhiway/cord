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

use ciborium::value::Value;

use super::{
	codec::{CodecError, Dto},
	generated::EventV2,
};

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
}

impl Session {
	pub(crate) fn new(request_id: [u8; 16]) -> Self {
		Self { request_id, next_sequence: 0, accepted: false, terminal: false }
	}

	pub(crate) fn is_terminal(&self) -> bool {
		self.terminal
	}

	pub(crate) fn accept(&mut self, bytes: &[u8]) -> Result<Dto<EventV2>, SessionError> {
		if self.terminal {
			return sequence("event received after terminal event")
		}
		let event = Dto::<EventV2>::decode(bytes)?;
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
			return sequence("event request ID does not match session")
		}
		let sequence_number = match field(2) {
			Some(Value::Integer(value)) => u64::try_from(*value).ok(),
			_ => None,
		}
		.ok_or_else(|| SessionError::Sequence("event sequence missing".into()))?;
		if sequence_number != self.next_sequence {
			return sequence("event sequence is duplicated or skipped")
		}
		let kind = match field(3) {
			Some(Value::Integer(value)) => u64::try_from(*value).ok(),
			_ => None,
		}
		.ok_or_else(|| SessionError::Sequence("event kind missing".into()))?;
		if sequence_number == 0 && kind != 0 {
			return sequence("first event must be accepted")
		}
		if sequence_number != 0 && kind == 0 {
			return sequence("accepted event may occur only once")
		}
		if kind == 0 {
			self.accepted = true;
		}
		if !self.accepted {
			return sequence("event preceded acceptance")
		}
		self.next_sequence += 1;
		self.terminal = matches!(kind, 2 | 3 | 4);
		Ok(event)
	}
}

fn sequence<T>(message: &str) -> Result<T, SessionError> {
	Err(SessionError::Sequence(message.into()))
}
