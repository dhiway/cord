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

use std::collections::BTreeMap;

use ciborium::value::Value;

use crate::product_sdk::host_outbox::{
	HostOutboxContextV1, HostOutboxEntryV1, HostOutboxError, HostOutboxFault, HostOutboxKeyRingV1,
	HostOutboxStoreV1, PrepareHostOutboxV1,
};

use super::{
	codec::{CodecError, Dto},
	desktop::{
		read_frame, write_frame, DesktopFrameDecoder, DesktopHostV2Transport,
		DesktopPeerBindingError, DesktopPeerIdentity, DesktopTransportError, DurableDesktopEvent,
		DurableDesktopHostV2, MAX_DESKTOP_FRAME_BYTES,
	},
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

fn outbox_vectors() -> serde_json::Value {
	serde_json::from_str(include_str!("../../../../docs/specs/host-outbox-v1.vectors.json"))
		.expect("frozen outbox vectors are JSON")
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

fn cancelled(request_id: [u8; 16], sequence: u64) -> Vec<u8> {
	Dto::<generated::CancelledEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(4.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![(Value::Integer(0.into()), Value::Integer(107.into()))]),
		),
	]))
	.expect("cancelled fixture is closed")
	.canonical()
	.to_vec()
}

fn outbox_context() -> HostOutboxContextV1 {
	HostOutboxContextV1 {
		profile_id: [0x11; 32],
		registry_hash: negotiated().registry_hash(),
		genesis_hash: negotiated().genesis(),
	}
}

fn outbox_keyring() -> HostOutboxKeyRingV1 {
	HostOutboxKeyRingV1::new(1, BTreeMap::from([(1, [0x8a; 32])]))
		.expect("test keyring is available")
}

fn outbox_input() -> PrepareHostOutboxV1 {
	let root = outbox_vectors();
	let hex = root["base_vector"]["canonical_cbor_hex"].as_str().expect("outbox vector hex");
	let entry = HostOutboxEntryV1::decode(&hex::decode(hex).expect("valid outbox vector hex"))
		.expect("valid frozen outbox entry");
	let mut authority =
		Dto::<generated::ProviderCapabilityV1>::decode(&entry.exact_authority_bytes)
			.expect("frozen authority is canonical")
			.value()
			.clone();
	let Value::Map(fields) = &mut authority else { unreachable!() };
	for (key, value) in fields {
		let Value::Integer(key) = key else { continue };
		match u64::try_from(*key).ok() {
			Some(1) => *value = Value::Bytes(negotiated().registry_hash().to_vec()),
			Some(2) => *value = Value::Bytes(negotiated().genesis().to_vec()),
			Some(8) => *value = Value::Bytes([0x11; 32].to_vec()),
			_ => {},
		}
	}
	let authority = Dto::<generated::ProviderCapabilityV1>::from_value(authority)
		.expect("bound authority remains canonical")
		.canonical()
		.to_vec();
	PrepareHostOutboxV1 {
		outbox_id: entry.outbox_id,
		exact_request_bytes: entry.exact_request_bytes,
		exact_authority_bytes: authority,
		request_id: entry.request_id,
		operation_id: entry.operation_id,
		generation: entry.generation,
		intended_cursor: entry.intended_cursor,
		negotiated_tuple: negotiated().binding_digest(),
		provider_id: entry.provider_id,
		provider_endpoint_hash: [0x22; 32],
		expected_response_kind: entry.expected_response_kind,
		created_at: entry.created_at,
		authority_expires_at: entry.authority_expires_at,
	}
}

fn desktop_peer() -> DesktopPeerIdentity {
	DesktopPeerIdentity {
		endpoint: "/private/test/cord-origin-host-v2.sock".into(),
		process_id: Some(std::process::id()),
		user_id: Some(1_000),
		provider_id: [0x11; 32],
		provider_endpoint_hash: [0x22; 32],
	}
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

#[test]
fn desktop_frames_enforce_big_endian_cap_and_split_coalesced_streams() {
	let first = b"first-frame";
	let second = b"second-frame";
	let mut wire = Vec::new();
	write_frame(&mut wire, first).unwrap();
	write_frame(&mut wire, second).unwrap();
	assert_eq!(&wire[..4], &(first.len() as u32).to_be_bytes());

	let mut decoder = DesktopFrameDecoder::new();
	assert!(decoder.push(&wire[..2]).unwrap().is_empty());
	assert!(decoder.push(&wire[2..7]).unwrap().is_empty());
	assert_eq!(decoder.push(&wire[7..wire.len() - 3]).unwrap(), vec![first.to_vec()]);
	assert_eq!(decoder.push(&wire[wire.len() - 3..]).unwrap(), vec![second.to_vec()]);
	decoder.finish().unwrap();
	assert!(matches!(decoder.push(&[]), Err(DesktopTransportError::Closed)));

	let mut truncated = DesktopFrameDecoder::new();
	assert!(truncated.push(&[0, 0, 0, 2, 0xaa]).unwrap().is_empty());
	assert!(matches!(truncated.finish(), Err(DesktopTransportError::FrameTruncated)));

	let mut oversized = DesktopFrameDecoder::new();
	let forbidden = (MAX_DESKTOP_FRAME_BYTES as u32 + 1).to_be_bytes();
	assert!(matches!(oversized.push(&forbidden), Err(DesktopTransportError::FrameTooLarge)));
	assert!(matches!(oversized.push(&[]), Err(DesktopTransportError::Closed)));

	let exact = vec![0x5a; MAX_DESKTOP_FRAME_BYTES];
	let mut exact_wire = Vec::new();
	write_frame(&mut exact_wire, &exact).unwrap();
	assert_eq!(exact_wire.len(), MAX_DESKTOP_FRAME_BYTES + 4);
	assert!(matches!(
		write_frame(&mut Vec::new(), &[0; MAX_DESKTOP_FRAME_BYTES + 1]),
		Err(DesktopTransportError::FrameTooLarge)
	));
}

#[cfg(unix)]
#[test]
fn desktop_peer_binding_and_negotiation_fail_before_any_request_byte() {
	use std::{io::Read, os::unix::net::UnixStream, time::Duration};

	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let peer = desktop_peer();
	let rejected = DesktopHostV2Transport::connect(
		client,
		&peer,
		&|candidate: &DesktopPeerIdentity| {
			assert_eq!(candidate.endpoint, peer.endpoint);
			assert_eq!(candidate.process_id, Some(std::process::id()));
			Err(DesktopPeerBindingError)
		},
		&offer(),
		&offer(),
	);
	assert!(matches!(rejected, Err(DesktopTransportError::Peer(_))));
	let mut byte = [0u8; 1];
	assert_eq!(server.read(&mut byte).unwrap(), 0, "rejected peer emitted bytes");

	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let mut incompatible = offer();
	incompatible.genesis = [0x99; 32];
	let rejected = DesktopHostV2Transport::connect(
		client,
		&peer,
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&incompatible,
	);
	assert!(matches!(rejected, Err(DesktopTransportError::Negotiation(NegotiationError::Genesis))));
	assert_eq!(server.read(&mut byte).unwrap(), 0, "failed negotiation emitted bytes");
}

#[cfg(unix)]
#[test]
fn desktop_outbox_commit_restart_resume_cancel_ack_and_gc_are_loss_safe() {
	use std::{io::Write, os::unix::net::UnixStream, thread};

	let temp = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(temp.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let request = input.exact_request_bytes.clone();
	let authority = input.exact_authority_bytes.clone();
	let accepted = accepted(input.request_id, 0);
	let cancelled = cancelled(input.request_id, 1);

	let (client, mut server) = UnixStream::pair().unwrap();
	let accepted_for_server = accepted.clone();
	let cancelled_for_server = cancelled.clone();
	let first_server = thread::spawn(move || {
		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), authority);
		let mut coalesced = Vec::new();
		write_frame(&mut coalesced, &accepted_for_server).unwrap();
		write_frame(&mut coalesced, &cancelled_for_server).unwrap();
		server.write_all(&coalesced).unwrap();
		matches!(read_frame(&mut server), Err(DesktopTransportError::FrameTruncated))
	});
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|peer: &DesktopPeerIdentity| {
			(peer.user_id == Some(1_000)).then_some(()).ok_or(DesktopPeerBindingError)
		},
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input, [1; 24], [2; 24]).unwrap();
	assert_eq!(
		desktop.receive_event(200, [8; 24], [9; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(accepted)
	);
	store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [3; 24], [4; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	drop(desktop);
	drop(store);
	assert!(first_server.join().unwrap(), "ack escaped before terminal install returned durable");

	let store = HostOutboxStoreV1::open(temp.path(), outbox_context(), outbox_keyring()).unwrap();
	let installed = store.installed_response(id).unwrap();
	assert_eq!(installed.response, cancelled);
	assert!(installed.terminal);
	let exact_ack = installed.response_ack.clone();
	let response_hash = installed.response_hash;

	let (client, mut server) = UnixStream::pair().unwrap();
	let ack_server = thread::spawn(move || read_frame(&mut server).unwrap());
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert_eq!(desktop.resume_ack(id, [5; 24]).unwrap(), response_hash);
	assert_eq!(ack_server.join().unwrap(), exact_ack);
	desktop.confirm_terminal(id, response_hash, [6; 24], [7; 24]).unwrap();
	assert_eq!(store.installed_response(id), Err(HostOutboxError::Expired));
	assert_eq!(store.gc(455, 1).unwrap(), 0);
	assert_eq!(store.gc(456, 1).unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn desktop_outbox_capacity_corruption_and_pre_send_crash_never_leak_bytes() {
	use std::{fs, io::Read, os::unix::net::UnixStream, thread, time::Duration};

	let capacity_root = tempfile::tempdir().unwrap();
	let capacity_store = HostOutboxStoreV1::open_with_limits(
		capacity_root.path(),
		outbox_context(),
		outbox_keyring(),
		1,
		1,
	)
	.unwrap();
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &capacity_store);
	assert!(matches!(
		desktop.prepare_and_send(outbox_input(), [1; 24], [2; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Full))
	));
	let mut byte = [0u8; 1];
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(capacity_store);

	let crash_root = tempfile::tempdir().unwrap();
	let store =
		HostOutboxStoreV1::open(crash_root.path(), outbox_context(), outbox_keyring()).unwrap();
	store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let expected_request = input.exact_request_bytes.clone();
	let expected_authority = input.exact_authority_bytes.clone();
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert!(matches!(
		desktop.prepare_and_send(input, [3; 24], [4; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(store);

	let store =
		HostOutboxStoreV1::open(crash_root.path(), outbox_context(), outbox_keyring()).unwrap();
	let exact_retry = store.retry_request(id, 200).unwrap();
	let (client, mut server) = UnixStream::pair().unwrap();
	let resumed_server =
		thread::spawn(move || (read_frame(&mut server).unwrap(), read_frame(&mut server).unwrap()));
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert_eq!(desktop.resume_and_send(id, 200).unwrap(), exact_retry);
	drop(desktop);
	assert_eq!(resumed_server.join().unwrap(), (expected_request, expected_authority));
	drop(store);

	let path = crash_root
		.path()
		.join("host-outbox-v1")
		.join(format!("{}.outbox", hex::encode(id)));
	let mut ciphertext = fs::read(&path).unwrap();
	*ciphertext.last_mut().unwrap() ^= 1;
	fs::write(&path, ciphertext).unwrap();
	assert!(matches!(
		HostOutboxStoreV1::open(crash_root.path(), outbox_context(), outbox_keyring()),
		Err(HostOutboxError::Corrupt)
	));
}

#[cfg(unix)]
#[test]
fn desktop_stream_abort_after_send_preserves_the_exact_durable_retry() {
	use std::{os::unix::net::UnixStream, thread};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let request = input.exact_request_bytes.clone();
	let authority = input.exact_authority_bytes.clone();
	let (client, mut server) = UnixStream::pair().unwrap();
	let server = thread::spawn(move || {
		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), authority);
	});
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	let durable = desktop.prepare_and_send(input, [1; 24], [2; 24]).unwrap();
	server.join().unwrap();
	assert!(matches!(
		desktop.receive_event(200, [3; 24], [4; 24]),
		Err(DesktopTransportError::FrameTruncated)
	));
	drop(desktop);
	drop(store);

	let reopened =
		HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	assert_eq!(reopened.retry_request(id, 200).unwrap(), durable);
}

#[cfg(unix)]
#[test]
fn desktop_binding_operation_and_response_contract_mismatches_emit_no_ack_or_request() {
	use std::{io::Read, os::unix::net::UnixStream, thread, time::Duration};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	store.prepare(input.clone(), [1; 24]).unwrap();

	let mut wrong_peer = desktop_peer();
	wrong_peer.provider_endpoint_hash = [0x99; 32];
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&wrong_peer,
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert!(matches!(desktop.resume_and_send(id, 200), Err(DesktopTransportError::RequestBinding)));
	let mut byte = [0u8; 1];
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);

	let response = cancelled(input.request_id, 0);
	store.install_response(id, response, None, None, Some(200), [2; 24]).unwrap();
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&wrong_peer,
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert!(matches!(desktop.resume_ack(id, [3; 24]), Err(DesktopTransportError::RequestBinding)));
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(store);

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut mismatched = outbox_input();
	mismatched.operation_id = [0xee; 16];
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert!(matches!(
		desktop.prepare_and_send(mismatched, [4; 24], [5; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);

	let mut input = outbox_input();
	input.outbox_id = [0x77; 16];
	input.expected_response_kind = 2;
	let request_id = input.request_id;
	let (client, mut server) = UnixStream::pair().unwrap();
	let accepted = accepted(request_id, 0);
	let cancel = cancelled(request_id, 1);
	let server_thread = thread::spawn(move || {
		read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &accepted).unwrap();
		write_frame(&mut server, &cancel).unwrap();
		matches!(read_frame(&mut server), Err(DesktopTransportError::FrameTruncated))
	});
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input, [6; 24], [7; 24]).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [10; 24], [11; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));
	assert!(matches!(
		desktop.receive_event(200, [8; 24], [9; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
	drop(desktop);
	assert!(server_thread.join().unwrap(), "response-kind mismatch emitted an ack");
}

#[cfg(unix)]
#[test]
fn desktop_cancel_is_durable_before_send_and_resumes_after_each_loss_boundary() {
	use std::{io::Read, os::unix::net::UnixStream, thread, time::Duration};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let accepted = accepted(input.request_id, 0);
	let cancel = cancelled(input.request_id, 1);
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input, [1; 24], [2; 24]).unwrap();
	assert_eq!(read_frame(&mut server).unwrap().len() > 0, true);
	assert_eq!(read_frame(&mut server).unwrap().len() > 0, true);
	write_frame(&mut server, &accepted).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [3; 24], [4; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));

	store.inject_fault_once(HostOutboxFault::BeforeTempFsync).unwrap();
	assert!(matches!(
		desktop.prepare_cancel_and_send(cancel.clone(), [5; 24], [6; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	let mut byte = [0u8; 1];
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(store);
	drop(server);

	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let (client, mut server) = UnixStream::pair().unwrap();
	server.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.resume_and_send(id, 200).unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &accepted).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [13; 24], [14; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));
	store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
	assert!(matches!(
		desktop.prepare_cancel_and_send(cancel.clone(), [7; 24], [8; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(store);
	drop(server);

	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let durable_cancel = store.retry_request(id, 200).unwrap();
	assert_eq!(durable_cancel.request, cancel);
	let (client, mut server) = UnixStream::pair().unwrap();
	let cancelled_response = cancel.clone();
	let server_thread = thread::spawn(move || {
		let sent_cancel = read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &cancelled_response).unwrap();
		let ack = read_frame(&mut server).unwrap();
		(sent_cancel, ack)
	});
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert_eq!(desktop.resume_and_send(id, 200).unwrap(), durable_cancel);
	let terminal = desktop.receive_event(200, [9; 24], [10; 24]).unwrap();
	let DurableDesktopEvent::Terminal { response_hash, .. } = terminal else { panic!() };
	let (sent_cancel, ack) = server_thread.join().unwrap();
	assert_eq!(sent_cancel, cancel);
	assert_eq!(ack, store.retry_response_ack(id).unwrap().bytes);
	desktop.confirm_terminal(id, response_hash, [11; 24], [12; 24]).unwrap();
}
