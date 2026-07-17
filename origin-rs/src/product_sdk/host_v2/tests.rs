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
	generated::{
		self, AcceptedEventV2, AcceptedState, DriveManifestV1, ErrorCode, EventV2, OperationCode,
		ProgressEventV2, RequestV2,
	},
	session::{negotiate, Negotiated, NegotiationError, NegotiationOffer, Session, SessionError},
};

fn fixture() -> serde_json::Value {
	serde_json::from_str(include_str!(
		"../../../../docs/specs/origin-host-registry-v2.vectors.json"
	))
	.expect("frozen vectors are JSON")
}

fn operations() -> serde_json::Value {
	serde_json::from_str(include_str!(
		"../../../../docs/specs/origin-host-registry-v2.operations.json"
	))
	.expect("frozen operations are JSON")
}

fn errors() -> serde_json::Value {
	serde_json::from_str(include_str!("../../../../docs/specs/origin-host-registry-v2.errors.json"))
		.expect("frozen errors are JSON")
}

fn vector(id: &str) -> Vec<u8> {
	let fixture = fixture();
	let hex = fixture["vectors"]
		.as_array()
		.and_then(|vectors| vectors.iter().find(|vector| vector["id"] == id))
		.and_then(|vector| vector["wire_hex"].as_str())
		.unwrap_or_else(|| panic!("missing frozen vector {id}"));
	hex::decode(hex).expect("frozen vector is hexadecimal")
}

fn offer() -> NegotiationOffer {
	NegotiationOffer {
		protocol: generated::PROTOCOL.into(),
		major: generated::MAJOR,
		minors: vec![generated::MINOR],
		genesis: [0x42; 32],
		finalized_spec_version: 31,
		finalized_transaction_version: 8,
		registry_sha256: generated::REGISTRY_SHA256.into(),
		features: generated::FEATURE_IDS.iter().map(|feature| (*feature).into()).collect(),
	}
}

fn negotiated() -> Negotiated {
	negotiate(&offer(), &offer()).expect("matching frozen descriptors negotiate")
}

fn accepted(request_id: [u8; 16], sequence: u64) -> Vec<u8> {
	Dto::<AcceptedEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(0.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![(Value::Integer(0.into()), Value::Integer(0.into()))]),
		),
	]))
	.expect("accepted fixture is closed")
	.canonical()
	.to_vec()
}

fn progress(request_id: [u8; 16], sequence: u64) -> Vec<u8> {
	Dto::<ProgressEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(1.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![(Value::Integer(0.into()), Value::Integer(1.into()))]),
		),
	]))
	.expect("progress fixture is closed")
	.canonical()
	.to_vec()
}

#[test]
fn generated_runtime_bindings_exactly_project_all_frozen_authorities() {
	assert_eq!(generated::PROTOCOL, "cord.origin.host/2");
	assert_eq!(generated::MAJOR, operations()["major"].as_u64().unwrap() as u8);
	assert_eq!(generated::MINOR, operations()["minor"].as_u64().unwrap() as u16);
	assert_eq!(generated::OPERATIONS.len(), 34);
	assert_eq!(generated::ERRORS.len(), 89);
	let semantic: serde_json::Value =
		serde_json::from_str(generated::SEMANTIC_TABLE_JSON).expect("semantic table is JSON");
	assert_eq!(semantic["schemas"].as_object().unwrap().len(), 363);

	let operation_registry = operations();
	for operation in operation_registry["operations"].as_array().unwrap() {
		let name = operation["name"].as_str().unwrap();
		let binding = generated::OPERATIONS.iter().find(|binding| binding.name == name).unwrap();
		assert_eq!(u64::from(binding.code), operation["code"]);
		assert_eq!(binding.feature_id, operation["feature_id"]);
		assert_eq!(binding.grant_scope, operation["grant_scope"]);
		assert_eq!(binding.consent_mode, operation["consent_mode"]);
		assert_eq!(binding.state_changing, operation["state_changing"]);
		assert_eq!(binding.operation_id_required, operation["operation_id_required"]);
		assert_eq!(binding.request, operation["cddl"]["Request"]);
		assert_eq!(binding.accepted, operation["cddl"]["Accepted"]);
		assert_eq!(binding.progress, operation["cddl"]["Progress"]);
		assert_eq!(binding.result, operation["cddl"]["Result"]);
		assert_eq!(binding.error, operation["cddl"]["Error"]);
		assert_eq!(
			OperationCode::from_u16(binding.code).map(|code| code as u16),
			Some(binding.code)
		);
		let expected_errors: Vec<_> = operation["allowed_errors"]
			.as_array()
			.unwrap()
			.iter()
			.map(|error| error["code"].as_u64().unwrap() as u16)
			.collect();
		assert_eq!(binding.allowed_errors, expected_errors);
		assert_eq!(binding.frame, format!("{}Frame", binding.request.trim_end_matches("Request")));
	}

	let error_registry = errors();
	for error in error_registry["errors"].as_array().unwrap() {
		let code = error["code"].as_u64().unwrap() as u16;
		let binding = generated::ERRORS.iter().find(|binding| binding.code == code).unwrap();
		assert_eq!(binding.name, error["name"]);
		assert_eq!(binding.retryable, error["retryable"]);
		assert_eq!(ErrorCode::from_u16(code).map(|error| error as u16), Some(code));
	}
	assert!(ErrorCode::from_u16(u16::MAX).is_none());
	assert!(OperationCode::from_u16(u16::MAX).is_none());
}

#[test]
fn every_operation_golden_round_trips_and_schema_negative_fails() {
	for operation in operations()["operations"].as_array().unwrap() {
		for id in operation["positive_vectors"].as_array().unwrap() {
			let id = id.as_str().unwrap();
			let wire = vector(id);
			let request = Dto::<RequestV2>::decode(&wire)
				.unwrap_or_else(|error| panic!("{id} failed: {error}"));
			assert_eq!(request.canonical(), wire, "{id}");
		}
		for id in operation["negative_vectors"].as_array().unwrap() {
			let id = id.as_str().unwrap();
			if id.ends_with("schema-negative") {
				assert!(Dto::<RequestV2>::decode(&vector(id)).is_err(), "{id}");
			}
		}
	}
}

#[test]
fn every_frozen_error_event_round_trips_through_the_closed_union() {
	let fixture = fixture();
	let error_vectors: Vec<_> = fixture["vectors"]
		.as_array()
		.unwrap()
		.iter()
		.filter(|vector| vector["id"].as_str().is_some_and(|id| id.starts_with("error-")))
		.collect();
	assert_eq!(error_vectors.len(), 89);
	for error in error_vectors {
		let id = error["id"].as_str().unwrap();
		let wire = vector(id);
		let event =
			Dto::<EventV2>::decode(&wire).unwrap_or_else(|error| panic!("{id} failed: {error}"));
		assert_eq!(event.canonical(), wire, "{id}");
	}
}

#[test]
fn codec_rejects_all_hostile_wire_classes_unknowns_and_invalid_ids() {
	for id in [
		"wire-noncanonical-long-version",
		"wire-noncanonical-indefinite-map",
		"wire-noncanonical-reversed-map",
		"wire-noncanonical-tag",
		"wire-noncanonical-indefinite-bytes",
	] {
		assert!(
			matches!(Dto::<RequestV2>::decode(&vector(id)), Err(CodecError::NonCanonical(_))),
			"{id}"
		);
	}
	for id in ["wire-schema-duplicate-key", "wire-schema-float", "wire-schema-invalid-utf8"] {
		assert!(
			matches!(Dto::<RequestV2>::decode(&vector(id)), Err(CodecError::Schema(_))),
			"{id}"
		);
	}
	assert!(Dto::<generated::RequestId>::from_value(Value::Bytes(vec![0; 15])).is_err());
	assert!(Dto::<generated::OperationId>::from_value(Value::Bytes(vec![0; 17])).is_err());
	assert!(Dto::<generated::Nonce>::from_value(Value::Bytes(vec![])).is_err());
	assert!(Dto::<AcceptedState>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(0.into())),
		(Value::Integer(1.into()), Value::Integer(0.into())),
	]))
	.is_err());
	let unknown_error = Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(vec![0; 16])),
		(Value::Integer(2.into()), Value::Integer(0.into())),
		(Value::Integer(3.into()), Value::Integer(3.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![
				(Value::Integer(0.into()), Value::Integer(65535.into())),
				(Value::Integer(1.into()), Value::Text("UNKNOWN".into())),
				(Value::Integer(2.into()), Value::Bool(false)),
				(Value::Integer(3.into()), Value::Map(vec![])),
			]),
		),
	]);
	assert!(Dto::<EventV2>::from_value(unknown_error).is_err());
}

#[test]
fn drive_manifest_text_cids_are_sorted_unique_and_ts_byte_stable() {
	let hash = Value::Bytes(vec![0; 32]);
	let manifest = Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(1.into())),
		(Value::Integer(1.into()), hash.clone()),
		(Value::Integer(2.into()), Value::Integer(0.into())),
		(
			Value::Integer(3.into()),
			Value::Array(vec![Value::Text("a".into()), Value::Text("b".into())]),
		),
		(Value::Integer(4.into()), hash.clone()),
	]);
	let dto = Dto::<DriveManifestV1>::from_value(manifest).expect("sorted text CIDs are valid");
	assert_eq!(
		hex::encode(dto.canonical()),
		"a50001015820000000000000000000000000000000000000000000000000000000000000000002000382616161620458200000000000000000000000000000000000000000000000000000000000000000"
	);
	let unsorted = Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(1.into())),
		(Value::Integer(1.into()), hash.clone()),
		(Value::Integer(2.into()), Value::Integer(0.into())),
		(
			Value::Integer(3.into()),
			Value::Array(vec![Value::Text("b".into()), Value::Text("a".into())]),
		),
		(Value::Integer(4.into()), hash),
	]);
	assert!(Dto::<DriveManifestV1>::from_value(unsorted).is_err());
}

#[test]
fn negotiation_binds_descriptor_genesis_versions_minor_and_feature_intersection() {
	let mut local = offer();
	local.features = vec!["storage.content".into(), "storage.s3".into(), "identity.account".into()];
	let mut remote = offer();
	remote.features = vec!["identity.account".into(), "storage.s3".into()];
	let result = negotiate(&local, &remote).expect("offers negotiate");
	assert_eq!(result.minor(), generated::MINOR);
	assert_eq!(result.features(), &["identity.account", "storage.s3"]);

	let mut wrong = offer();
	wrong.major = 3;
	assert_eq!(negotiate(&wrong, &offer()), Err(NegotiationError::Version));
	wrong = offer();
	wrong.genesis = [1; 32];
	assert_eq!(negotiate(&offer(), &wrong), Err(NegotiationError::Genesis));
	wrong = offer();
	wrong.registry_sha256 = "0".repeat(64);
	assert_eq!(negotiate(&wrong, &offer()), Err(NegotiationError::Descriptor));
	wrong = offer();
	wrong.finalized_spec_version += 1;
	assert_eq!(negotiate(&offer(), &wrong), Err(NegotiationError::Version));
	wrong = offer();
	wrong.features = vec!["storage.s3".into(), "storage.s3".into()];
	assert_eq!(negotiate(&wrong, &offer()), Err(NegotiationError::Descriptor));
}

#[test]
fn session_enforces_sequence_and_permanently_closes_on_fault_or_terminal() {
	let request_id = [0x11; 16];
	let error = vector("error-100-wire_schema_invalid");
	let mut session = Session::new(negotiated(), request_id);
	assert!(session.accept(&accepted(request_id, 0)).is_ok());
	assert!(session.accept(&error).is_ok());
	assert!(session.is_terminal());
	assert!(session.is_closed());
	assert!(matches!(session.accept(&error), Err(SessionError::Sequence(_))));

	let mut skipped = Session::new(negotiated(), request_id);
	assert!(skipped.accept(&accepted(request_id, 0)).is_ok());
	assert!(matches!(skipped.accept(&progress(request_id, 2)), Err(SessionError::Sequence(_))));
	assert!(skipped.is_closed());
	assert!(matches!(skipped.accept(&progress(request_id, 1)), Err(SessionError::Sequence(_))));

	let mut duplicate = Session::new(negotiated(), request_id);
	assert!(duplicate.accept(&accepted(request_id, 0)).is_ok());
	assert!(matches!(duplicate.accept(&accepted(request_id, 1)), Err(SessionError::Sequence(_))));
	assert!(duplicate.is_closed());

	let mut before_accepted = Session::new(negotiated(), request_id);
	assert!(matches!(
		before_accepted.accept(&progress(request_id, 0)),
		Err(SessionError::Sequence(_))
	));
	assert!(before_accepted.is_closed());
	assert_eq!(before_accepted.negotiated().minor(), generated::MINOR);
}
