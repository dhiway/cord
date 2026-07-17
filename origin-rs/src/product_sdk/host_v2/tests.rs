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
	generated::{self, AcceptedEventV2, EventV2, StorageBucketGetFrame},
	session::{Session, SessionError},
};

fn vector(id: &str) -> Vec<u8> {
	let fixture: serde_json::Value = serde_json::from_str(include_str!(
		"../../../../docs/specs/origin-host-registry-v2.vectors.json"
	))
	.expect("frozen vectors are JSON");
	let hex = fixture["vectors"]
		.as_array()
		.and_then(|vectors| vectors.iter().find(|vector| vector["id"] == id))
		.and_then(|vector| vector["wire_hex"].as_str())
		.unwrap_or_else(|| panic!("missing frozen vector {id}"));
	hex::decode(hex).expect("frozen vector is hexadecimal")
}

#[test]
fn generated_bindings_and_golden_codec_are_closed() {
	assert_eq!(generated::PROTOCOL, "cord.origin.host/2");
	assert_eq!(generated::MAJOR, 2);
	assert_eq!(generated::OPERATIONS.len(), 34);
	assert_eq!(
		generated::REGISTRY_SHA256,
		"d17c24596fbae30c300d57ae8e51bc0c7b149ab2e91c2b9c751bedd3fbc1eeba"
	);
	let wire = vector("wire-1001-canonical");
	let request = Dto::<StorageBucketGetFrame>::decode(&wire).expect("golden request decodes");
	assert_eq!(request.canonical(), wire);
	assert!(matches!(
		Dto::<StorageBucketGetFrame>::decode(&vector("wire-noncanonical-reversed-map")),
		Err(CodecError::NonCanonical(_))
	));
	assert!(matches!(
		Dto::<StorageBucketGetFrame>::decode(&vector("wire-noncanonical-tag")),
		Err(CodecError::NonCanonical(_))
	));
}

#[test]
fn golden_session_requires_acceptance_and_closes_after_error() {
	let request_id = [0x11; 16];
	let accepted = Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(0.into())),
		(Value::Integer(3.into()), Value::Integer(0.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![(Value::Integer(0.into()), Value::Integer(0.into()))]),
		),
	]);
	let accepted = Dto::<AcceptedEventV2>::from_value(accepted).expect("accepted DTO is closed");
	let error = vector("error-100-wire_schema_invalid");
	let mut session = Session::new(request_id);
	assert!(session.accept(accepted.canonical()).is_ok());
	assert!(session.accept(&error).is_ok());
	assert!(session.is_terminal());
	assert!(matches!(session.accept(&error), Err(SessionError::Sequence(_))));
	assert!(matches!(Session::new(request_id).accept(&error), Err(SessionError::Sequence(_))));
	assert!(Dto::<EventV2>::decode(&error).is_ok());
}
