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
		provider_ack_confirmation_bytes, read_frame, resume_event_sequence, write_frame,
		DesktopFrameDecoder, DesktopHostV2Transport, DesktopPeerBindingError, DesktopPeerIdentity,
		DesktopTransportError, DurableDesktopEvent, DurableDesktopHostV2, MAX_DESKTOP_FRAME_BYTES,
	},
	execution::{
		CordProviderByteBackendV2, DurableCordProviderV2, HostCallV2, HostExecutionErrorV2,
		HostExecutionV2, HostRequestMetaV2, HostStorageEncryptionV2, ProviderByteStorageV2,
		ProviderContinuationSourceV2, ProviderOutboxContextV2, ProviderSuccessorV2,
	},
	generated::{
		self, AcceptedEventV2, AcceptedState, DriveManifestV1, ErrorCode, EventV2, OperationCode,
		ProgressEventV2, ProviderCapabilityV1, RequestV2, ResumeTokenV1,
	},
	session::{negotiate, Negotiated, NegotiationError, NegotiationOffer, Session, SessionError},
};

#[test]
fn private_query_event_sequence_is_independent_from_short_batches_and_range_offsets() {
	assert_eq!(resume_event_sequence(Some(OperationCode::StorageObjectGet), 1, 17).unwrap(), 2);
	assert_eq!(resume_event_sequence(Some(OperationCode::StorageObjectGet), 2, 31).unwrap(), 3);
	assert_eq!(
		resume_event_sequence(Some(OperationCode::StorageObjectRange), 1, 8_388_731).unwrap(),
		2
	);
	assert_eq!(
		resume_event_sequence(Some(OperationCode::StorageObjectRange), 2, 8_388_748).unwrap(),
		3
	);
}

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

fn mobile_vectors() -> serde_json::Value {
	serde_json::from_str(include_str!(
		"../../../../product-sdk/examples/festival/host-v2-mobile-conformance-vectors.json"
	))
	.expect("Festival mobile projection vectors are JSON")
}

fn provider_transfer_chunk(operation_id: [u8; 16], index: u32) -> Vec<u8> {
	let fixture: serde_json::Value = serde_json::from_str(include_str!(
		"../../../../docs/specs/provider-transfer-chunk-v1.vectors.json"
	))
	.expect("provider transfer chunk vectors are JSON");
	let bytes = hex::decode(
		fixture["vectors"][0]["bytes_hex"]
			.as_str()
			.expect("provider transfer chunk vector has bytes"),
	)
	.expect("provider transfer chunk bytes are hexadecimal");
	Dto::<generated::ProviderTransferChunkV1>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(1.into())),
		(Value::Integer(1.into()), Value::Bytes(operation_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(u64::from(index).into())),
		(Value::Integer(3.into()), Value::Bytes(bytes.clone())),
		(Value::Integer(4.into()), Value::Bytes(sp_crypto_hashing::blake2_256(&bytes).to_vec())),
	]))
	.expect("provider transfer chunk remains canonical")
	.canonical()
	.to_vec()
}

fn put_request_with_length(exact: &[u8], object_len: u64) -> Vec<u8> {
	let mut value = Dto::<generated::StorageObjectPutFrame>::decode(exact)
		.expect("PUT request is canonical")
		.value()
		.clone();
	let Value::Map(fields) = &mut value else { unreachable!() };
	let Value::Map(body) = &mut fields
		.iter_mut()
		.find(|(key, _)| *key == Value::Integer(8.into()))
		.expect("PUT request has a body")
		.1
	else {
		unreachable!()
	};
	body.iter_mut()
		.find(|(key, _)| *key == Value::Integer(2.into()))
		.expect("PUT request has an object length")
		.1 = Value::Integer(object_len.into());
	Dto::<generated::StorageObjectPutFrame>::from_value(value)
		.expect("updated PUT request remains canonical")
		.canonical()
		.to_vec()
}

fn mobile_tagged_value(tagged: &serde_json::Value) -> Value {
	let tagged = tagged.as_object().expect("tagged projection is an object");
	if let Some(value) = tagged.get("uint") {
		return Value::Integer(
			value
				.as_str()
				.expect("uint is decimal")
				.parse::<u64>()
				.expect("uint fits u64")
				.into(),
		);
	}
	if let Some(value) = tagged.get("bytes") {
		return Value::Bytes(
			hex::decode(value.as_str().expect("bytes are hexadecimal")).expect("valid hex"),
		);
	}
	if let Some(value) = tagged.get("text") {
		return Value::Text(value.as_str().expect("text is a string").into());
	}
	if let Some(value) = tagged.get("bool") {
		return Value::Bool(value.as_bool().expect("bool is a boolean"));
	}
	if let Some(value) = tagged.get("array") {
		return Value::Array(
			value
				.as_array()
				.expect("array is an array")
				.iter()
				.map(mobile_tagged_value)
				.collect(),
		);
	}
	Value::Map(
		tagged["map"]
			.as_array()
			.expect("map is an array")
			.iter()
			.map(|entry| {
				let entry = entry.as_array().expect("map entry is a pair");
				(
					Value::Integer(
						entry[0]
							.as_str()
							.expect("map key is decimal")
							.parse::<u64>()
							.expect("map key fits u64")
							.into(),
					),
					mobile_tagged_value(&entry[1]),
				)
			})
			.collect(),
	)
}

fn assert_mobile_vector<P: generated::Production>(vector: &serde_json::Value) {
	use sha2::{Digest, Sha256};

	let id = vector["id"].as_str().expect("vector ID");
	let bytes = hex::decode(vector["canonical_cbor_hex"].as_str().expect("canonical CBOR hex"))
		.expect("canonical CBOR is hexadecimal");
	let projected = mobile_tagged_value(&vector["projection"]);
	let decoded = Dto::<P>::decode(&bytes).unwrap_or_else(|error| panic!("{id}: {error}"));
	assert_eq!(decoded.canonical(), bytes, "{id}: Rust canonical bytes");
	assert_eq!(decoded.value(), &projected, "{id}: lossless Rust projection");
	assert_eq!(
		Dto::<P>::from_value(projected)
			.expect("projected value satisfies production")
			.canonical(),
		bytes,
		"{id}: projected Rust bytes"
	);
	assert_eq!(
		hex::encode(Sha256::digest(&bytes)),
		vector["canonical_sha256"].as_str().expect("canonical SHA-256"),
		"{id}: Rust canonical SHA-256"
	);
}

fn mobile_value_field(value: &Value, wanted: u64) -> Option<&Value> {
	let Value::Map(fields) = value else { return None };
	fields.iter().find_map(|(key, value)| {
		matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted))
			.then_some(value)
	})
}

fn mobile_request_coverage(vectors: &[serde_json::Value]) -> Result<(), String> {
	let operations = operations();
	let authoritative = operations["operations"].as_array().expect("operations are an array");
	let requests: Vec<_> =
		vectors.iter().filter(|vector| vector["production"] == "RequestV2").collect();
	let mut names = BTreeMap::new();
	let mut codes = BTreeMap::new();
	let mut sources = BTreeMap::new();
	for vector in &requests {
		let name = vector["operation"].as_str().ok_or("operation missing")?;
		let code = vector["code"].as_u64().ok_or("code missing")?;
		let source = vector["source_vector"].as_str().ok_or("source missing")?;
		if names.insert(name, ()).is_some() ||
			codes.insert(code, ()).is_some() ||
			sources.insert(source, ()).is_some()
		{
			return Err("duplicate request metadata replaced an omission".into());
		}
		let decoded = Dto::<RequestV2>::decode(
			&hex::decode(vector["canonical_cbor_hex"].as_str().ok_or("wire missing")?)
				.map_err(|error| error.to_string())?,
		)
		.map_err(|error| error.to_string())?;
		let decoded_code = mobile_value_field(decoded.value(), 3)
			.and_then(|value| match value {
				Value::Integer(value) => u64::try_from(*value).ok(),
				_ => None,
			})
			.ok_or("decoded code missing")?;
		if decoded_code != code {
			return Err("decoded RequestV2 code does not match metadata".into());
		}
	}
	let expected_names: BTreeMap<_, _> = authoritative
		.iter()
		.map(|operation| (operation["name"].as_str().unwrap(), ()))
		.collect();
	let expected_codes: BTreeMap<_, _> = authoritative
		.iter()
		.map(|operation| (operation["code"].as_u64().unwrap(), ()))
		.collect();
	let expected_sources: BTreeMap<_, _> = authoritative
		.iter()
		.flat_map(|operation| operation["positive_vectors"].as_array().unwrap())
		.map(|source| (source.as_str().unwrap(), ()))
		.collect();
	if requests.len() != authoritative.len() ||
		names != expected_names ||
		codes != expected_codes ||
		sources != expected_sources
	{
		return Err("mobile request set is not the exact authoritative operation set".into());
	}
	Ok(())
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

fn object_get_progress(request_id: [u8; 16], sequence: u64, offset: u64) -> Vec<u8> {
	Dto::<ProgressEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(1.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![
				(Value::Integer(0.into()), Value::Integer(offset.into())),
				(Value::Integer(1.into()), Value::Bytes(b"verified".to_vec())),
			]),
		),
	]))
	.expect("GET progress fixture is closed")
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

fn error_event(request_id: [u8; 16], sequence: u64, code: u64, name: &str) -> Vec<u8> {
	Dto::<generated::ErrorEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(3.into())),
		(
			Value::Integer(4.into()),
			Value::Map(vec![
				(Value::Integer(0.into()), Value::Integer(code.into())),
				(Value::Integer(1.into()), Value::Text(name.into())),
				(Value::Integer(2.into()), Value::Bool(false)),
				(Value::Integer(3.into()), Value::Map(Vec::new())),
			]),
		),
	]))
	.expect("error fixture is closed")
	.canonical()
	.to_vec()
}

fn result_event(request_id: [u8; 16], sequence: u64, payload: Value) -> Vec<u8> {
	Dto::<generated::ResultEventV2>::from_value(Value::Map(vec![
		(Value::Integer(0.into()), Value::Integer(2.into())),
		(Value::Integer(1.into()), Value::Bytes(request_id.to_vec())),
		(Value::Integer(2.into()), Value::Integer(sequence.into())),
		(Value::Integer(3.into()), Value::Integer(2.into())),
		(Value::Integer(4.into()), payload),
	]))
	.expect("result fixture is closed")
	.canonical()
	.to_vec()
}

fn object_put_result(request_id: [u8; 16], sequence: u64) -> Vec<u8> {
	result_event(
		request_id,
		sequence,
		Value::Map(vec![
			(
				Value::Integer(0.into()),
				Value::Map(vec![
					(Value::Integer(0.into()), Value::Bytes([0x11; 32].to_vec())),
					(Value::Integer(1.into()), Value::Text("bafk-cord-result".into())),
					(Value::Integer(2.into()), Value::Integer(12.into())),
					(Value::Integer(3.into()), Value::Bytes([0x55; 64].to_vec())),
				]),
			),
			(Value::Integer(1.into()), Value::Bool(true)),
			(
				Value::Integer(2.into()),
				Value::Map(vec![
					(Value::Integer(0.into()), Value::Integer(42.into())),
					(Value::Integer(1.into()), Value::Bytes([0x66; 32].to_vec())),
				]),
			),
		]),
	)
}

fn object_get_result(request_id: [u8; 16], sequence: u64) -> Vec<u8> {
	result_event(
		request_id,
		sequence,
		Value::Map(vec![
			(Value::Integer(0.into()), Value::Text("bafk-foreign-result".into())),
			(Value::Integer(1.into()), Value::Integer(12.into())),
			(
				Value::Integer(2.into()),
				Value::Map(vec![
					(Value::Integer(0.into()), Value::Bytes([0x77; 32].to_vec())),
					(Value::Integer(1.into()), Value::Integer(1.into())),
					(Value::Integer(2.into()), Value::Integer(2.into())),
					(Value::Integer(3.into()), Value::Integer(1.into())),
				]),
			),
		]),
	)
}

fn resume_token_for(input: &PrepareHostOutboxV1, cursor: u64, generation: u64) -> Vec<u8> {
	let fixture = mobile_vectors();
	let vector = fixture["vectors"]
		.as_array()
		.unwrap()
		.iter()
		.find(|vector| vector["production"] == "ResumeTokenV1")
		.unwrap();
	let mut value = mobile_tagged_value(&vector["projection"]);
	let Value::Map(fields) = &mut value else { unreachable!() };
	for (key, value) in fields {
		let Value::Integer(key) = key else { continue };
		match u64::try_from(*key).ok() {
			Some(1) => *value = Value::Bytes(negotiated().registry_hash().to_vec()),
			Some(2) => *value = Value::Bytes(negotiated().genesis().to_vec()),
			Some(3) => *value = Value::Bytes(input.provider_id.to_vec()),
			Some(5) => *value = Value::Bytes(input.operation_id.to_vec()),
			Some(9) => *value = Value::Integer(cursor.into()),
			Some(10) => *value = Value::Integer(generation.into()),
			Some(11) => *value = Value::Integer(input.created_at.into()),
			Some(12) => *value = Value::Integer(input.authority_expires_at.into()),
			Some(14) => *value = Value::Bool(false),
			_ => {},
		}
	}
	Dto::<ResumeTokenV1>::from_value(value).unwrap().canonical().to_vec()
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
		exact_payload_bytes: None,
		request_id: entry.request_id,
		operation_id: entry.operation_id,
		generation: entry.generation,
		intended_cursor: entry.intended_cursor,
		negotiated_tuple: negotiated().binding_digest(),
		provider_id: entry.provider_id,
		provider_endpoint_hash: [0x22; 32],
		expected_response_kind: 2,
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

#[cfg(unix)]
fn confirm_provider_ack(stream: &mut std::os::unix::net::UnixStream) -> Vec<u8> {
	let exact = read_frame(stream).expect("host sends a framed ACK");
	let ack = Dto::<generated::ResponseAckV1>::decode(&exact).expect("host ACK is canonical");
	let Value::Map(fields) = ack.value() else { unreachable!() };
	let bytes = |key: u64, length: usize| -> Vec<u8> {
		let value = fields
			.iter()
			.find_map(|(candidate, value)| {
				matches!(candidate, Value::Integer(candidate) if u64::try_from(*candidate).ok() == Some(key))
					.then_some(value)
			})
			.expect("ACK field exists");
		let Value::Bytes(bytes) = value else { panic!("ACK field is not bytes") };
		assert_eq!(bytes.len(), length);
		bytes.clone()
	};
	let generation = fields
		.iter()
		.find_map(|(candidate, value)| match (candidate, value) {
			(Value::Integer(candidate), Value::Integer(value))
				if u64::try_from(*candidate).ok() == Some(2) =>
				u64::try_from(*value).ok(),
			_ => None,
		})
		.expect("ACK generation exists");
	let confirmation = provider_ack_confirmation_bytes(
		bytes(0, 16).try_into().unwrap(),
		bytes(1, 16).try_into().unwrap(),
		generation,
		bytes(3, 32).try_into().unwrap(),
	);
	write_frame(stream, &confirmation).expect("provider confirmation is framed");
	exact
}

#[cfg(unix)]
fn confirm_next_provider_ack(
	stream: &std::os::unix::net::UnixStream,
) -> std::thread::JoinHandle<Vec<u8>> {
	let mut stream = stream.try_clone().expect("test stream clones");
	std::thread::spawn(move || confirm_provider_ack(&mut stream))
}

#[cfg(unix)]
fn bounded_unix_pair() -> (std::os::unix::net::UnixStream, std::os::unix::net::UnixStream) {
	let (client, server) = std::os::unix::net::UnixStream::pair().expect("test socket pair");
	let timeout = Some(std::time::Duration::from_secs(5));
	client.set_read_timeout(timeout).expect("client read timeout");
	client.set_write_timeout(timeout).expect("client write timeout");
	server.set_read_timeout(timeout).expect("server read timeout");
	server.set_write_timeout(timeout).expect("server write timeout");
	(client, server)
}

struct QueuedContinuations {
	upload_chunks: Option<Vec<Vec<u8>>>,
	successors: std::collections::VecDeque<ProviderSuccessorV2>,
}

impl ProviderContinuationSourceV2 for QueuedContinuations {
	fn upload_chunks(
		&mut self,
		operation: OperationCode,
		_exact_request: &[u8],
	) -> Result<Option<Vec<Vec<u8>>>, HostExecutionErrorV2> {
		if operation != OperationCode::StorageObjectPut {
			return Err(HostExecutionErrorV2::AppBinding);
		}
		Ok(self.upload_chunks.take())
	}

	fn next_generation(
		&mut self,
		operation: OperationCode,
		predecessor: &ProviderOutboxContextV2,
		_exact_resume_token: &[u8],
		cursor: u32,
		_response_hash: [u8; 32],
	) -> Result<Option<ProviderSuccessorV2>, HostExecutionErrorV2> {
		let successor = self.successors.pop_front();
		if operation != OperationCode::StorageObjectPut ||
			successor.as_ref().is_some_and(|successor| {
				successor.outbox.generation != predecessor.generation + 1 ||
					successor.outbox.intended_cursor != cursor
			}) {
			return Err(HostExecutionErrorV2::AppBinding);
		}
		Ok(successor)
	}
}

struct NoStorageEncryption;

impl HostStorageEncryptionV2 for NoStorageEncryption {
	fn prepare(
		&mut self,
		_operation: OperationCode,
		_exact_frame: &[u8],
	) -> Result<(), HostExecutionErrorV2> {
		Ok(())
	}

	fn complete(
		&mut self,
		_operation: OperationCode,
		_execution: &mut HostExecutionV2,
	) -> Result<(), HostExecutionErrorV2> {
		Ok(())
	}
}

fn provider_outbox_context(
	input: &PrepareHostOutboxV1,
	outbox_id: [u8; 16],
	nonce: u8,
) -> ProviderOutboxContextV2 {
	ProviderOutboxContextV2 {
		outbox_id,
		generation: input.generation,
		intended_cursor: input.intended_cursor,
		negotiated_tuple: input.negotiated_tuple,
		provider_id: input.provider_id,
		provider_endpoint_hash: input.provider_endpoint_hash,
		created_at: input.created_at,
		authority_expires_at: input.authority_expires_at,
		terminal_block: 200,
		prepare_nonce: [nonce; 24],
		mark_sent_nonce: [nonce + 1; 24],
		install_nonce: [nonce + 2; 24],
		mark_ack_nonce: [nonce + 3; 24],
		confirm_nonce: [nonce + 4; 24],
		compact_nonce: [nonce + 5; 24],
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
fn festival_mobile_projection_matches_rust_host_v2_bytes_hashes_and_values() {
	let fixture = mobile_vectors();
	let vectors = fixture["vectors"].as_array().expect("mobile vectors are an array");
	assert_eq!(vectors.len(), 39);
	mobile_request_coverage(vectors)
		.expect("mobile requests exactly cover authoritative operations");
	for vector in vectors {
		match vector["production"].as_str().expect("production name") {
			"RequestV2" => assert_mobile_vector::<RequestV2>(vector),
			"ProviderCapabilityV1" => assert_mobile_vector::<ProviderCapabilityV1>(vector),
			"ResumeTokenV1" => assert_mobile_vector::<ResumeTokenV1>(vector),
			"EventV2" => assert_mobile_vector::<EventV2>(vector),
			production => panic!("unsupported mobile projection production {production}"),
		}
	}
	assert_eq!(
		fixture["excluded_legacy_surfaces"],
		serde_json::json!(["personhood", "PeopleLite", "preimage", "TransactionStorage"]),
	);

	let request_vector = vectors.iter().find(|vector| vector["code"] == 1000).unwrap();
	let resume_vector =
		vectors.iter().find(|vector| vector["production"] == "ResumeTokenV1").unwrap();
	let request = Dto::<RequestV2>::decode(
		&hex::decode(request_vector["canonical_cbor_hex"].as_str().unwrap()).unwrap(),
	)
	.unwrap();
	let resume = Dto::<ResumeTokenV1>::decode(
		&hex::decode(resume_vector["canonical_cbor_hex"].as_str().unwrap()).unwrap(),
	)
	.unwrap();
	let mut resumed = Session::resume_bound(negotiated(), &request, &resume, 100).unwrap();
	assert_eq!(resumed.next_sequence(), 5);
	resumed.accept(&cancelled([0x11; 16], 5)).unwrap();
	assert!(resumed.is_terminal());
	assert!(resumed.is_closed());
}

#[test]
fn festival_mobile_projection_rejects_coverage_substitution_and_resume_misbinding() {
	let fixture = mobile_vectors();
	let vectors = fixture["vectors"].as_array().unwrap();
	let mut omitted = vectors.clone();
	omitted.remove(0);
	assert!(mobile_request_coverage(&omitted).is_err());
	let mut duplicated = vectors.clone();
	duplicated[1] = duplicated[0].clone();
	assert!(mobile_request_coverage(&duplicated).is_err());
	let mut duplicate_code = vectors.clone();
	let first_code = duplicate_code[0]["code"].clone();
	duplicate_code[1]["code"] = first_code;
	assert!(mobile_request_coverage(&duplicate_code).is_err());
	let mut duplicate_source = vectors.clone();
	let first_source = duplicate_source[0]["source_vector"].clone();
	duplicate_source[1]["source_vector"] = first_source;
	assert!(mobile_request_coverage(&duplicate_source).is_err());

	let request_vector = vectors.iter().find(|vector| vector["code"] == 1000).unwrap();
	let resume_vector =
		vectors.iter().find(|vector| vector["production"] == "ResumeTokenV1").unwrap();
	let request = Dto::<RequestV2>::decode(
		&hex::decode(request_vector["canonical_cbor_hex"].as_str().unwrap()).unwrap(),
	)
	.unwrap();
	let resume_value = mobile_tagged_value(&resume_vector["projection"]);
	let resume = Dto::<ResumeTokenV1>::from_value(resume_value.clone()).unwrap();
	assert!(Session::resume_bound(negotiated(), &request, &resume, 101).is_err());

	let mut wrong_operation = resume_value.clone();
	let Value::Map(fields) = &mut wrong_operation else { unreachable!() };
	*fields
		.iter_mut()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(5))
				.then_some(value)
		})
		.unwrap() = Value::Bytes([0x99; 16].to_vec());
	let wrong_operation = Dto::<ResumeTokenV1>::from_value(wrong_operation).unwrap();
	assert!(Session::resume_bound(negotiated(), &request, &wrong_operation, 100).is_err());

	let mut different_object_len = resume_value.clone();
	let Value::Map(fields) = &mut different_object_len else { unreachable!() };
	*fields
		.iter_mut()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(8))
				.then_some(value)
		})
		.unwrap() = Value::Integer(8_192_u64.into());
	let different_object_len = Dto::<ResumeTokenV1>::from_value(different_object_len).unwrap();
	let mut object_len_independent =
		Session::resume_bound(negotiated(), &request, &different_object_len, 100).unwrap();
	assert_eq!(object_len_independent.next_sequence(), 5);
	object_len_independent.accept(&cancelled([0x11; 16], 5)).unwrap();

	let mut exhausted_cursor = resume_value.clone();
	let Value::Map(fields) = &mut exhausted_cursor else { unreachable!() };
	*fields
		.iter_mut()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(9))
				.then_some(value)
		})
		.unwrap() = Value::Integer(u64::from(u32::MAX).into());
	let exhausted_cursor = Dto::<ResumeTokenV1>::from_value(exhausted_cursor).unwrap();
	assert!(Session::resume_bound(negotiated(), &request, &exhausted_cursor, 100).is_err());

	let mut wrong_generation = resume_value;
	let Value::Map(fields) = &mut wrong_generation else { unreachable!() };
	*fields
		.iter_mut()
		.find_map(|(key, value)| {
			matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(10))
				.then_some(value)
		})
		.unwrap() = Value::Integer(101_u64.into());
	let wrong_generation = Dto::<ResumeTokenV1>::from_value(wrong_generation).unwrap();
	assert!(Session::resume_bound(negotiated(), &request, &wrong_generation, 100).is_err());

	let mut wrong_lifecycle = Session::resume_bound(negotiated(), &request, &resume, 100).unwrap();
	assert!(wrong_lifecycle.accept(&cancelled([0x12; 16], 5)).is_err());
	assert!(wrong_lifecycle.is_closed());
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
	use std::{io::Read, time::Duration};

	let (client, mut server) = bounded_unix_pair();
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

	let (client, mut server) = bounded_unix_pair();
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
	use std::{io::Write, thread};

	let temp = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(temp.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let request = input.exact_request_bytes.clone();
	let authority = input.exact_authority_bytes.clone();
	let accepted = accepted(input.request_id, 0);
	let terminal = error_event(input.request_id, 1, 204, "STORAGE_CID_MISMATCH");

	let (client, mut server) = bounded_unix_pair();
	let accepted_for_server = accepted.clone();
	let terminal_for_server = terminal.clone();
	let first_server = thread::spawn(move || {
		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), authority);
		let mut coalesced = Vec::new();
		write_frame(&mut coalesced, &accepted_for_server).unwrap();
		write_frame(&mut coalesced, &terminal_for_server).unwrap();
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
		desktop.receive_event(200, [8; 24], [9; 24], [200; 24], [201; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(accepted)
	);
	store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [3; 24], [4; 24], [200; 24], [201; 24]),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	drop(desktop);
	drop(store);
	assert!(first_server.join().unwrap(), "ack escaped before terminal install returned durable");

	let store = HostOutboxStoreV1::open(temp.path(), outbox_context(), outbox_keyring()).unwrap();
	let installed = store.installed_response(id).unwrap();
	assert_eq!(installed.response, terminal);
	assert!(installed.terminal);
	let exact_ack = installed.response_ack.clone();
	let response_hash = installed.response_hash;

	let (client, mut server) = bounded_unix_pair();
	let ack_server = thread::spawn(move || confirm_provider_ack(&mut server));
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert_eq!(desktop.resume_ack(id, [5; 24], [202; 24], [203; 24]).unwrap(), response_hash);
	assert_eq!(ack_server.join().unwrap(), exact_ack);
	assert_eq!(store.installed_response(id), Err(HostOutboxError::Expired));
	assert_eq!(store.gc(455, 1).unwrap(), 0);
	assert_eq!(store.gc(456, 1).unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn desktop_rejects_misbound_provider_ack_confirmation_and_replays_after_restart() {
	use std::thread;

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let outbox_id = input.outbox_id;
	let request_id = input.request_id;
	let operation_id = input.operation_id;
	let terminal = error_event(request_id, 1, 204, "STORAGE_CID_MISMATCH");
	let (client, mut server) = bounded_unix_pair();
	let first_server = thread::spawn(move || {
		read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &accepted(request_id, 0)).unwrap();
		write_frame(&mut server, &terminal).unwrap();
		let ack = read_frame(&mut server).unwrap();
		Dto::<generated::ResponseAckV1>::decode(&ack).unwrap();
		write_frame(
			&mut server,
			&provider_ack_confirmation_bytes(request_id, operation_id, 0, [0xff; 32]),
		)
		.unwrap();
		ack
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
	desktop.prepare_and_send(input, [1; 24], [2; 24]).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [10; 24], [11; 24], [12; 24], [13; 24]),
		Ok(DurableDesktopEvent::NonTerminal(_))
	));
	assert!(matches!(
		desktop.receive_event(200, [3; 24], [4; 24], [5; 24], [6; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
	let exact_ack = first_server.join().unwrap();
	let installed = store.installed_response(outbox_id).unwrap();
	assert!(installed.terminal);
	assert_eq!(installed.response_ack, exact_ack);
	let response_hash = installed.response_hash;
	drop(desktop);
	drop(store);

	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let (client, mut server) = bounded_unix_pair();
	let replay_server = thread::spawn(move || confirm_provider_ack(&mut server));
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	assert_eq!(desktop.resume_ack(outbox_id, [7; 24], [8; 24], [9; 24]).unwrap(), response_hash);
	assert_eq!(replay_server.join().unwrap(), exact_ack);
	assert_eq!(store.installed_response(outbox_id), Err(HostOutboxError::Expired));
}

#[cfg(unix)]
#[test]
fn ack_confirmed_restart_finishes_erase_and_compaction_without_replaying_ack() {
	use std::{io::Read, time::Duration};

	for fail_erase in [true, false] {
		let root = tempfile::tempdir().unwrap();
		let input = outbox_input();
		let id = input.outbox_id;
		let operation_id = input.operation_id;
		let response = cancelled(input.request_id, 0);
		let store =
			HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
		store.prepare(input, [1; 24]).unwrap();
		let response_hash =
			store.install_response(id, response, None, None, Some(200), [2; 24]).unwrap();
		store.mark_ack_sent(id, [3; 24]).unwrap();
		store.confirm_ack(id, response_hash, [4; 24]).unwrap();
		assert!(store.is_ack_confirmed(id, response_hash).unwrap());
		if fail_erase {
			store.inject_fault_once(HostOutboxFault::BeforeUploadErase).unwrap();
			assert_eq!(store.erase_upload(operation_id), Err(HostOutboxError::Unavailable));
		} else {
			store.erase_upload(operation_id).unwrap();
			store.inject_fault_once(HostOutboxFault::BeforeTempFsync).unwrap();
			assert_eq!(store.compact_acknowledged(id, [5; 24]), Err(HostOutboxError::Unavailable));
		}
		drop(store);

		let store =
			HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
		assert!(store.is_ack_confirmed(id, response_hash).unwrap());
		let (client, mut server) = bounded_unix_pair();
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
		assert_eq!(desktop.resume_ack(id, [6; 24], [7; 24], [8; 24]).unwrap(), response_hash);
		let mut escaped = [0; 1];
		assert!(matches!(
			server.read(&mut escaped).unwrap_err().kind(),
			std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
		));
		drop(desktop);
		assert_eq!(store.installed_response(id), Err(HostOutboxError::Expired));
	}
}

#[cfg(unix)]
#[test]
fn desktop_outbox_capacity_corruption_and_pre_send_crash_never_leak_bytes() {
	use std::{fs, io::Read, thread, time::Duration};

	let capacity_root = tempfile::tempdir().unwrap();
	let capacity_store = HostOutboxStoreV1::open_with_limits(
		capacity_root.path(),
		outbox_context(),
		outbox_keyring(),
		1,
		1,
	)
	.unwrap();
	let (client, mut server) = bounded_unix_pair();
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
	let (client, mut server) = bounded_unix_pair();
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
	let (client, mut server) = bounded_unix_pair();
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
	use std::thread;

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let request = input.exact_request_bytes.clone();
	let authority = input.exact_authority_bytes.clone();
	let (client, mut server) = bounded_unix_pair();
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
		desktop.receive_event(200, [3; 24], [4; 24], [200; 24], [201; 24]),
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
fn desktop_successor_payload_is_durable_before_send_and_replayed_after_crash() {
	use std::{fs, io::Read, thread, time::Duration};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.outbox_id = [0x79; 16];
	input.exact_request_bytes = put_request_with_length(&input.exact_request_bytes, 15);
	let token = resume_token_for(&input, 0, 1);
	store.prepare(input.clone(), [1; 24]).unwrap();
	let hash = store
		.install_response(
			input.outbox_id,
			accepted(input.request_id, 0),
			Some(token.clone()),
			Some(0),
			None,
			[2; 24],
		)
		.unwrap();
	store.confirm_ack(input.outbox_id, hash, [3; 24]).unwrap();
	let mut successor = input.clone();
	successor.outbox_id = [0x7a; 16];
	successor.exact_authority_bytes = token.clone();
	successor.exact_payload_bytes = Some(provider_transfer_chunk(input.operation_id, 0));
	successor.generation = 1;
	successor.intended_cursor = 0;

	let (client, mut server) = bounded_unix_pair();
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
	store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
	assert!(matches!(
		desktop.prepare_successor_and_send(
			input.outbox_id,
			successor.clone(),
			[4; 24],
			[5; 24],
			[6; 24],
		),
		Err(DesktopTransportError::Outbox(HostOutboxError::Unavailable))
	));
	let mut byte = [0; 1];
	assert!(matches!(
		server.read(&mut byte).unwrap_err().kind(),
		std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
	));
	drop(desktop);
	drop(store);

	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let request = successor.exact_request_bytes.clone();
	let payload = successor.exact_payload_bytes.clone().unwrap();
	let durable = store.retry_request(successor.outbox_id, 200).unwrap();
	assert_eq!(durable.request, request);
	assert_eq!(durable.authority, token);
	assert_eq!(durable.payload.as_deref(), Some(payload.as_slice()));
	let (client, mut server) = bounded_unix_pair();
	let sent_request = request.clone();
	let sent_token = token.clone();
	let sent_payload = payload.clone();
	let sent = thread::spawn(move || {
		assert_eq!(read_frame(&mut server).unwrap(), sent_request);
		assert_eq!(read_frame(&mut server).unwrap(), sent_token);
		assert_eq!(read_frame(&mut server).unwrap(), sent_payload);
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
	assert_eq!(desktop.resume_and_send(successor.outbox_id, 200).unwrap(), durable);
	sent.join().unwrap();
	drop(desktop);
	drop(store);

	let ciphertext = fs::read(
		root.path()
			.join("host-outbox-v1")
			.join(format!("{}.outbox", hex::encode(successor.outbox_id))),
	)
	.unwrap();
	assert!(!ciphertext.windows(payload.len()).any(|window| window == payload));
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	assert_eq!(store.retry_request(successor.outbox_id, 200).unwrap(), durable);

	let next_token = resume_token_for(&successor, 1, 2);
	let (client, mut server) = bounded_unix_pair();
	let replay_request = request.clone();
	let replay_token = token.clone();
	let replay_payload = payload.clone();
	let server = thread::spawn(move || {
		assert_eq!(read_frame(&mut server).unwrap(), replay_request);
		assert_eq!(read_frame(&mut server).unwrap(), replay_token);
		assert_eq!(read_frame(&mut server).unwrap(), replay_payload);
		write_frame(&mut server, &progress(input.request_id, 1)).unwrap();
		write_frame(&mut server, &next_token).unwrap();
		confirm_provider_ack(&mut server);
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
	assert_eq!(desktop.resume_and_send(successor.outbox_id, 200).unwrap(), durable);
	assert!(matches!(
		desktop
			.receive_continuation(&mut |_: &[u8]| Ok(()), 200, [7; 24], [8; 24], [9; 24], [10; 24],)
			.unwrap(),
		DurableDesktopEvent::Continuation { cursor: 1, .. }
	));
	server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn durable_provider_runs_each_put_generation_from_verified_continuations() {
	use std::thread;

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.exact_request_bytes = put_request_with_length(&input.exact_request_bytes, 15);
	let exact_upload_request = input.exact_request_bytes.clone();
	let upload_operation_id = input.operation_id;
	let frame = Dto::<generated::StorageObjectPutFrame>::decode(&input.exact_request_bytes)
		.expect("frozen put frame is canonical");
	let request_id = input.request_id;
	let request = input.exact_request_bytes.clone();
	let authority = input.exact_authority_bytes.clone();
	let first_token = resume_token_for(&input, 0, 1);
	let second_token = resume_token_for(&input, 1, 2);
	let exact_chunk = provider_transfer_chunk(input.operation_id, 0);
	let server_first_token = first_token.clone();
	let server_second_token = second_token.clone();
	let server_chunk = exact_chunk.clone();
	let (client, mut server) = bounded_unix_pair();
	let server_thread = thread::spawn(move || {
		let mut acks = Vec::new();
		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), authority);
		let first_event = accepted(request_id, 0);
		write_frame(&mut server, &first_event).unwrap();
		write_frame(&mut server, &server_first_token).unwrap();
		acks.push(confirm_provider_ack(&mut server));

		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), server_first_token);
		assert_eq!(read_frame(&mut server).unwrap(), server_chunk);
		let second_event = progress(request_id, 1);
		write_frame(&mut server, &second_event).unwrap();
		write_frame(&mut server, &server_second_token).unwrap();
		acks.push(confirm_provider_ack(&mut server));

		assert_eq!(read_frame(&mut server).unwrap(), request);
		assert_eq!(read_frame(&mut server).unwrap(), server_second_token);
		let terminal_event = object_put_result(request_id, 2);
		write_frame(&mut server, &terminal_event).unwrap();
		acks.push(confirm_provider_ack(&mut server));
		for ack in &acks {
			Dto::<generated::ResponseAckV1>::decode(ack).expect("host emits canonical ACK");
		}
		acks
	});
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut first_successor = provider_outbox_context(&input, [0x72; 16], 40);
	first_successor.generation = 1;
	first_successor.intended_cursor = 0;
	let mut second_successor = provider_outbox_context(&input, [0x73; 16], 60);
	second_successor.generation = 2;
	second_successor.intended_cursor = 1;
	let verifier_first_token = first_token.clone();
	let verifier_second_token = second_token.clone();
	let provider = DurableCordProviderV2::new(
		DurableDesktopHostV2::new(transport, &store),
		move |exact: &[u8]| {
			if exact == verifier_first_token || exact == verifier_second_token {
				Dto::<ResumeTokenV1>::decode(exact)?;
				Ok(())
			} else {
				Err(DesktopTransportError::ResumeTokenUnverified)
			}
		},
		QueuedContinuations {
			upload_chunks: Some(vec![exact_chunk.clone()]),
			successors: std::collections::VecDeque::from([
				ProviderSuccessorV2 {
					exact_request: input.exact_request_bytes.clone(),
					outbox: first_successor,
				},
				ProviderSuccessorV2 {
					exact_request: input.exact_request_bytes.clone(),
					outbox: second_successor,
				},
			]),
		},
	);
	let mut backend = CordProviderByteBackendV2::new(provider, NoStorageEncryption);
	let outbox = provider_outbox_context(&input, [0x71; 16], 20);
	let execution = backend
		.object_put(HostCallV2 {
			frame: &frame,
			authority: &input.exact_authority_bytes,
			meta: HostRequestMetaV2 {
				request_id: input.request_id,
				operation_id: Some(input.operation_id),
				deadline_block: 200,
			},
			outbox: &outbox,
		})
		.expect("durable provider operation completes");
	assert_eq!(execution.events.len(), 3);
	assert!(execution.terminal_response_hash.is_some());
	let acks = server_thread.join().unwrap();
	assert_eq!(acks.len(), 3);
	drop(backend);
	drop(store);
	let reopened =
		HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	assert_eq!(
		reopened.staged_upload_payload(&exact_upload_request, upload_operation_id, 0),
		Err(HostOutboxError::StateInvalid)
	);
}

#[cfg(unix)]
#[test]
fn desktop_continuation_installs_event_and_verified_token_before_ack_and_successor_send() {
	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.outbox_id = [0x73; 16];
	let predecessor_id = input.outbox_id;
	let token = resume_token_for(&input, 0, 1);
	let (client, mut server) = bounded_unix_pair();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input.clone(), [41; 24], [42; 24]).unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	let accepted = accepted(input.request_id, 0);
	write_frame(&mut server, &accepted).unwrap();
	write_frame(&mut server, &token).unwrap();
	let first_ack = confirm_next_provider_ack(&server);
	let mut verified = false;
	let continuation = desktop
		.receive_continuation(
			&mut |exact: &[u8]| {
				verified = exact == token;
				verified.then_some(()).ok_or(DesktopTransportError::ResumeTokenUnverified)
			},
			200,
			[43; 24],
			[44; 24],
			[45; 24],
			[0; 24],
		)
		.unwrap();
	match continuation {
		DurableDesktopEvent::Continuation { events, resume_token, cursor, .. } => {
			assert_eq!(events, vec![accepted]);
			assert_eq!(resume_token, token);
			assert_eq!(cursor, 0);
		},
		_ => panic!("accepted generation was not installed as a continuation"),
	}
	assert!(verified);
	Dto::<generated::ResponseAckV1>::decode(&first_ack.join().unwrap()).unwrap();

	let mut successor = input;
	successor.outbox_id = [0x74; 16];
	successor.exact_authority_bytes = token.clone();
	successor.generation = 1;
	successor.intended_cursor = 0;
	desktop
		.prepare_successor_and_send(predecessor_id, successor.clone(), [46; 24], [47; 24], [48; 24])
		.unwrap();
	assert_eq!(read_frame(&mut server).unwrap(), successor.exact_request_bytes);
	assert_eq!(read_frame(&mut server).unwrap(), token);
	let progress = progress(successor.request_id, 1);
	let next_token = resume_token_for(&successor, 1, 2);
	write_frame(&mut server, &progress).unwrap();
	write_frame(&mut server, &next_token).unwrap();
	let second_ack = confirm_next_provider_ack(&server);
	let continued = desktop
		.receive_continuation(
			&mut |exact: &[u8]| {
				(exact == next_token)
					.then_some(())
					.ok_or(DesktopTransportError::ResumeTokenUnverified)
			},
			200,
			[49; 24],
			[50; 24],
			[51; 24],
			[52; 24],
		)
		.unwrap();
	assert!(matches!(continued, DurableDesktopEvent::Continuation { cursor: 1, .. }));
	Dto::<generated::ResponseAckV1>::decode(&second_ack.join().unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn desktop_continuation_reads_the_complete_bounded_event_batch_before_the_token() {
	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.outbox_id = [0x7b; 16];
	let token = resume_token_for(&input, 1, 1);
	let accepted = accepted(input.request_id, 0);
	let progress = progress(input.request_id, 1);
	let (client, mut server) = bounded_unix_pair();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input.clone(), [81; 24], [82; 24]).unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &accepted).unwrap();
	write_frame(&mut server, &progress).unwrap();
	write_frame(&mut server, &token).unwrap();
	let ack = confirm_next_provider_ack(&server);
	let continued = desktop
		.receive_continuation(&mut |_: &[u8]| Ok(()), 200, [83; 24], [84; 24], [85; 24], [86; 24])
		.unwrap();
	let DurableDesktopEvent::Continuation { events, cursor, .. } = continued else {
		panic!("event batch did not produce a continuation")
	};
	assert_eq!(events, vec![accepted, progress]);
	assert_eq!(cursor, 1);
	Dto::<generated::ResponseAckV1>::decode(&ack.join().unwrap()).unwrap();
}

#[cfg(unix)]
#[test]
fn desktop_get_resume_uses_persisted_generation_not_a_short_verified_byte_cursor() {
	const SHORT_BATCH_BYTES: u32 = 17;
	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.outbox_id = [0x7c; 16];
	input.exact_request_bytes = vector("1011-positive");
	let request = Dto::<RequestV2>::decode(&input.exact_request_bytes).unwrap();
	let Value::Map(fields) = request.value() else { unreachable!() };
	let Value::Bytes(request_id) =
		&fields.iter().find(|(key, _)| *key == Value::Integer(1.into())).unwrap().1
	else {
		unreachable!()
	};
	input.request_id = request_id.clone().try_into().unwrap();
	let mut operation_material = b"cord/provider/private-object-query/v1".to_vec();
	operation_material.extend_from_slice(&(OperationCode::StorageObjectGet as u16).to_be_bytes());
	operation_material.extend_from_slice(&input.request_id);
	input.operation_id = sp_crypto_hashing::sha2_256(&operation_material)[..16].try_into().unwrap();
	let first_token = resume_token_for(&input, u64::from(SHORT_BATCH_BYTES), 1);
	let (client, mut server) = bounded_unix_pair();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop.prepare_and_send(input.clone(), [91; 24], [92; 24]).unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &accepted(input.request_id, 0)).unwrap();
	write_frame(&mut server, &object_get_progress(input.request_id, 1, 0)).unwrap();
	write_frame(&mut server, &first_token).unwrap();
	let first_ack = confirm_next_provider_ack(&server);
	let first = desktop
		.receive_continuation(&mut |_: &[u8]| Ok(()), 200, [93; 24], [94; 24], [95; 24], [0; 24])
		.unwrap();
	let DurableDesktopEvent::Continuation { cursor, .. } = first else {
		panic!("first GET generation did not continue")
	};
	assert_eq!(cursor, SHORT_BATCH_BYTES);
	Dto::<generated::ResponseAckV1>::decode(&first_ack.join().unwrap()).unwrap();

	let mut successor = input.clone();
	successor.outbox_id = [0x7d; 16];
	successor.exact_authority_bytes = first_token;
	successor.generation = 1;
	successor.intended_cursor = SHORT_BATCH_BYTES;
	desktop
		.prepare_successor_and_send(
			input.outbox_id,
			successor.clone(),
			[96; 24],
			[97; 24],
			[98; 24],
		)
		.unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &object_get_progress(input.request_id, 2, u64::from(SHORT_BATCH_BYTES)))
		.unwrap();
	write_frame(&mut server, &object_get_result(input.request_id, 3)).unwrap();
	let terminal_ack = confirm_next_provider_ack(&server);
	let terminal = desktop
		.receive_continuation(
			&mut |_: &[u8]| Ok(()),
			200,
			[99; 24],
			[100; 24],
			[101; 24],
			[102; 24],
		)
		.unwrap();
	let DurableDesktopEvent::Terminal { events, .. } = terminal else {
		panic!("second GET generation was not terminal")
	};
	assert_eq!(events.len(), 2);
	Dto::<generated::ResponseAckV1>::decode(&terminal_ack.join().unwrap()).unwrap();
	drop(desktop);
	drop(store);
	let reopened =
		HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	assert_eq!(reopened.binding(successor.outbox_id), Err(HostOutboxError::Expired));
}

#[cfg(unix)]
#[test]
fn desktop_binding_operation_and_response_contract_mismatches_emit_no_ack_or_request() {
	use std::{io::Read, thread, time::Duration};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	store.prepare(input.clone(), [1; 24]).unwrap();

	let mut wrong_peer = desktop_peer();
	wrong_peer.provider_endpoint_hash = [0x99; 32];
	let (client, mut server) = bounded_unix_pair();
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
	let (client, mut server) = bounded_unix_pair();
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
	assert!(matches!(
		desktop.resume_ack(id, [3; 24], [202; 24], [203; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
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
	let (client, mut server) = bounded_unix_pair();
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
	let (client, mut server) = bounded_unix_pair();
	let accepted_event = accepted(request_id, 0);
	let cancel = cancelled(request_id, 1);
	let server_thread = thread::spawn(move || {
		read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &accepted_event).unwrap();
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
		desktop.receive_event(200, [10; 24], [11; 24], [200; 24], [201; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));
	assert!(matches!(
		desktop.receive_event(200, [8; 24], [9; 24], [200; 24], [201; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
	drop(desktop);
	assert!(server_thread.join().unwrap(), "response-kind mismatch emitted an ack");

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.outbox_id = [0x78; 16];
	let request_id = input.request_id;
	let (client, mut server) = bounded_unix_pair();
	let server_thread = thread::spawn(move || {
		read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &accepted(request_id, 0)).unwrap();
		write_frame(&mut server, &object_get_result(request_id, 1)).unwrap();
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
	desktop.prepare_and_send(input, [12; 24], [13; 24]).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [14; 24], [15; 24], [200; 24], [201; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));
	assert!(matches!(
		desktop.receive_event(200, [16; 24], [17; 24], [200; 24], [201; 24]),
		Err(DesktopTransportError::RequestBinding)
	));
	drop(desktop);
	assert!(server_thread.join().unwrap(), "foreign result payload emitted an ack");
}

#[cfg(unix)]
#[test]
fn desktop_cancel_is_durable_before_send_and_resumes_after_each_loss_boundary() {
	use std::{io::Read, thread, time::Duration};

	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let input = outbox_input();
	let id = input.outbox_id;
	let accepted = accepted(input.request_id, 0);
	let cancel = cancelled(input.request_id, 1);
	let (client, mut server) = bounded_unix_pair();
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
		desktop.receive_event(200, [3; 24], [4; 24], [200; 24], [201; 24]).unwrap(),
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
	let (client, mut server) = bounded_unix_pair();
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
		desktop.receive_event(200, [13; 24], [14; 24], [200; 24], [201; 24]).unwrap(),
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
	let (client, mut server) = bounded_unix_pair();
	let cancelled_response = cancel.clone();
	let server_thread = thread::spawn(move || {
		let sent_cancel = read_frame(&mut server).unwrap();
		read_frame(&mut server).unwrap();
		write_frame(&mut server, &cancelled_response).unwrap();
		let ack = confirm_provider_ack(&mut server);
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
	let terminal = desktop.receive_event(200, [9; 24], [10; 24], [200; 24], [201; 24]).unwrap();
	let DurableDesktopEvent::Terminal { .. } = terminal else { panic!() };
	let (sent_cancel, ack) = server_thread.join().unwrap();
	assert_eq!(sent_cancel, cancel);
	Dto::<generated::ResponseAckV1>::decode(&ack).expect("cancel ACK is canonical");
	assert_eq!(store.retry_response_ack(id), Err(HostOutboxError::Expired));
}

#[cfg(unix)]
#[test]
fn desktop_live_cancel_accepts_only_cancel_terminal_and_emits_exact_ack() {
	let root = tempfile::tempdir().unwrap();
	let store = HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	let mut input = outbox_input();
	input.exact_request_bytes = put_request_with_length(&input.exact_request_bytes, 15);
	let request_id = input.request_id;
	let operation_id = input.operation_id;
	let exact_request = input.exact_request_bytes.clone();
	let payload = provider_transfer_chunk(operation_id, 0);
	let cancel = cancelled(request_id, 1);
	let (client, mut server) = bounded_unix_pair();
	let transport = DesktopHostV2Transport::connect(
		client,
		&desktop_peer(),
		&|_: &DesktopPeerIdentity| Ok(()),
		&offer(),
		&offer(),
	)
	.unwrap();
	let mut desktop = DurableDesktopHostV2::new(transport, &store);
	desktop
		.stage_upload(
			&exact_request,
			operation_id,
			vec![payload],
			input.created_at,
			input.authority_expires_at,
			[30; 24],
		)
		.unwrap();
	desktop.prepare_and_send(input, [31; 24], [32; 24]).unwrap();
	read_frame(&mut server).unwrap();
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &accepted(request_id, 0)).unwrap();
	assert!(matches!(
		desktop.receive_event(200, [33; 24], [34; 24], [200; 24], [201; 24]).unwrap(),
		DurableDesktopEvent::NonTerminal(_)
	));
	desktop.prepare_cancel_and_send(cancel.clone(), [35; 24], [36; 24]).unwrap();
	assert_eq!(
		store.staged_upload_payload(&exact_request, operation_id, 0),
		Err(HostOutboxError::StateInvalid)
	);
	assert_eq!(read_frame(&mut server).unwrap(), cancel);
	read_frame(&mut server).unwrap();
	write_frame(&mut server, &cancel).unwrap();
	let ack = confirm_next_provider_ack(&server);
	let events = match desktop.receive_event(200, [37; 24], [38; 24], [200; 24], [201; 24]).unwrap()
	{
		DurableDesktopEvent::Terminal { events, .. } => events,
		DurableDesktopEvent::NonTerminal(_) => panic!("cancel was not terminal"),
		DurableDesktopEvent::Continuation { .. } => panic!("cancel produced a continuation"),
	};
	assert_eq!(events, vec![cancel]);
	Dto::<generated::ResponseAckV1>::decode(&ack.join().unwrap()).expect("cancel ACK is canonical");
	drop(desktop);
	drop(store);
	let reopened =
		HostOutboxStoreV1::open(root.path(), outbox_context(), outbox_keyring()).unwrap();
	assert_eq!(
		reopened.staged_upload_payload(&exact_request, operation_id, 0),
		Err(HostOutboxError::StateInvalid)
	);
}
