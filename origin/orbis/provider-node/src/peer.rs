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

//! Private, canonical provider-to-provider replication wire contract.
//!
//! This module performs bounded decoding and authentication only. It owns no replay table,
//! capability authority, transport, filesystem, route or worker. Callers must supply one exact
//! finalized registry snapshot and persist the returned replay identity before any byte-plane I/O.

use codec::{Decode, Encode};
use pallet_orbis_storage_provider::MmrLeafV1;
use sp_core::{ed25519, Pair as _, H256};

use crate::{CanonicalCid, ContentError, CHUNK_BYTES, MAX_STORED_BYTES};

const VERSION: u8 = 1;
const DOMAIN: [u8; 33] = *b"cord/provider/peer-replication/v1";
const PAGE_REQUEST_TAG: &[u8] = b"cord/provider/peer-replication/page-request/v1";
const PAGE_RESPONSE_TAG: &[u8] = b"cord/provider/peer-replication/page-response/v1";
const CHUNK_REQUEST_TAG: &[u8] = b"cord/provider/peer-replication/chunk-request/v1";
const CHUNK_RESPONSE_TAG: &[u8] = b"cord/provider/peer-replication/chunk-response/v1";

/// Canonical source registry used to derive [`REGISTRY_SHA256`].
const REGISTRY: &[u8] = b"cord.provider.peer-replication.scale.v1|commitment=mmr_root:[u8;32],start_seq:u64,leaf_count:u64,predecessor_total_size:u64|context=domain:[u8;33],registry_hash:[u8;32],genesis_hash:[u8;32],finalized_hash:[u8;32],finalized_number:u32,bucket:[u8;32],source_provider:[u8;32],target_provider:[u8;32],source_key_version:u64,source_key:[u8;32],target_key_version:u64,target_key:[u8;32],source_endpoint_hash:[u8;32],target_endpoint_hash:[u8;32],commitment:commitment|identity=operation_id:[u8;16],request_nonce:[u8;16]|cursor=last_sequence:u64,cumulative_total:u64|page_request=v1,context,identity,cursor:Option<cursor>,limit:u16,signature:[u8;64]|page_item=cid:Vec<u8><=96,length:u64<=67108864,sequence:u64,total_size:u64,data_root:[u8;32],leaf_hash:[u8;32],chunk_manifest_hash:[u8;32],chunk_hashes:Vec<[u8;32]><=256|page_response=v1,context,identity,request_hash:[u8;32],requested_cursor:Option<cursor>,items:Vec<page_item><=128,next_cursor:Option<cursor>,response_hash:[u8;32],signature:[u8;64]|chunk_request=v1,context,identity,item,chunk_index:u16,chunk_hash:[u8;32],signature:[u8;64]|chunk_response=v1,context,identity,request_hash:[u8;32],item,chunk_index:u16,chunk_hash:[u8;32],chunk:Vec<u8><=262144,response_hash:[u8;32],signature:[u8;64]";

/// SHA-256 of the complete canonical registry declaration above.
const REGISTRY_SHA256: [u8; 32] = [
	58, 222, 156, 89, 28, 203, 225, 54, 189, 131, 179, 112, 132, 254, 92, 77, 127, 8, 255, 120,
	198, 189, 78, 95, 65, 134, 54, 246, 21, 75, 180, 240,
];

const MAX_PAGE_ITEMS: usize = 128;
const MAX_CID_BYTES: usize = 96;
const MAX_CHUNK_MANIFEST_BYTES: usize = crate::MAX_CHUNKS * 32;
// A maximum object contributes 256 fixed hashes (8 KiB); the remaining fixed context, object,
// signature and SCALE length prefixes stay below the explicit 16 KiB request ceiling.
pub(crate) const MAX_REQUEST_ENCODED: usize = 16 * 1024;
pub(crate) const MAX_PAGE_RESPONSE_ENCODED: usize = 2 * 1024 * 1024;
// A response carries both a full chunk and its full authenticated object manifest. Four KiB is a
// conservative fixed allowance for the context, CID, leaf, identities, hashes, signature and SCALE
// length prefixes after accounting for the manifest separately.
pub(crate) const MAX_CHUNK_RESPONSE_ENCODED: usize =
	CHUNK_BYTES + MAX_CHUNK_MANIFEST_BYTES + 4 * 1024;

/// Exact candidate MMR range that a replication operation must reconstruct.
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerMmrCommitmentV1 {
	mmr_root: [u8; 32],
	start_seq: u64,
	leaf_count: u64,
	predecessor_total_size: u64,
}

impl PeerMmrCommitmentV1 {
	/// Build a non-empty, overflow-safe candidate commitment.
	pub(crate) fn new(
		mmr_root: [u8; 32],
		start_seq: u64,
		leaf_count: u64,
		predecessor_total_size: u64,
	) -> Result<Self, ContentError> {
		let commitment = Self { mmr_root, start_seq, leaf_count, predecessor_total_size };
		commitment.validate()?;
		Ok(commitment)
	}

	/// Return the committed MMR root.
	pub(crate) fn mmr_root(&self) -> [u8; 32] {
		self.mmr_root
	}

	/// Return the half-open committed sequence range.
	pub(crate) fn sequence_range(&self) -> (u64, u64) {
		(self.start_seq, self.end_exclusive().expect("validated commitment cannot overflow"))
	}

	/// Return the cumulative total immediately before the first committed leaf.
	pub(crate) fn predecessor_total_size(&self) -> u64 {
		self.predecessor_total_size
	}

	/// Verify the reconstructed local MMR before a caller confirms replication.
	pub(crate) fn verify_local_root(&self, local_root: [u8; 32]) -> Result<(), ContentError> {
		if local_root != self.mmr_root {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn end_exclusive(&self) -> Result<u64, ContentError> {
		self.start_seq.checked_add(self.leaf_count).ok_or(ContentError::IntegrityFailed)
	}

	pub(crate) fn validate(&self) -> Result<(), ContentError> {
		if self.mmr_root == [0; 32]
			|| self.leaf_count == 0
			|| (self.start_seq == 0 && self.predecessor_total_size != 0)
		{
			return Err(ContentError::IntegrityFailed);
		}
		self.end_exclusive()?;
		Ok(())
	}
}

/// Source-authenticated continuation state for an exact MMR prefix.
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerPageCursorV1 {
	last_sequence: u64,
	cumulative_total: u64,
}

impl PeerPageCursorV1 {
	/// Build the continuation state emitted for the last item in a non-terminal page.
	pub(crate) fn new(last_sequence: u64, cumulative_total: u64) -> Self {
		Self { last_sequence, cumulative_total }
	}

	/// Return `(last sequence, cumulative total)`.
	pub(crate) fn position(&self) -> (u64, u64) {
		(self.last_sequence, self.cumulative_total)
	}
}

/// Exact finalized registry and peer audience bound into every message.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerContextV1 {
	domain: [u8; 33],
	registry_hash: [u8; 32],
	genesis_hash: [u8; 32],
	finalized_hash: [u8; 32],
	finalized_number: u32,
	bucket: [u8; 32],
	source_provider: [u8; 32],
	target_provider: [u8; 32],
	source_service_key_version: u64,
	source_service_key: [u8; 32],
	target_service_key_version: u64,
	target_service_key: [u8; 32],
	source_endpoint_hash: [u8; 32],
	target_endpoint_hash: [u8; 32],
	commitment: PeerMmrCommitmentV1,
}

impl PeerContextV1 {
	/// Build and validate one exact finalized source/target registry snapshot.
	#[allow(clippy::too_many_arguments)]
	pub(crate) fn new(
		genesis_hash: [u8; 32],
		finalized_hash: [u8; 32],
		finalized_number: u32,
		bucket: [u8; 32],
		source_provider: [u8; 32],
		target_provider: [u8; 32],
		source_service_key_version: u64,
		source_service_key: [u8; 32],
		target_service_key_version: u64,
		target_service_key: [u8; 32],
		source_endpoint_hash: [u8; 32],
		target_endpoint_hash: [u8; 32],
		commitment: PeerMmrCommitmentV1,
	) -> Result<Self, ContentError> {
		let context = Self {
			domain: DOMAIN,
			registry_hash: REGISTRY_SHA256,
			genesis_hash,
			finalized_hash,
			finalized_number,
			bucket,
			source_provider,
			target_provider,
			source_service_key_version,
			source_service_key,
			target_service_key_version,
			target_service_key,
			source_endpoint_hash,
			target_endpoint_hash,
			commitment,
		};
		context.validate()?;
		Ok(context)
	}

	/// Return the exact finalized snapshot identity.
	pub(crate) fn finalized(&self) -> ([u8; 32], u32) {
		(self.finalized_hash, self.finalized_number)
	}

	/// Return the bucket addressed by this peer exchange.
	pub(crate) fn bucket(&self) -> [u8; 32] {
		self.bucket
	}

	/// Return the data source provider audience.
	pub(crate) fn source_provider(&self) -> [u8; 32] {
		self.source_provider
	}

	/// Return the receiving target provider audience.
	pub(crate) fn target_provider(&self) -> [u8; 32] {
		self.target_provider
	}

	/// Return the exact candidate MMR that replication must reconstruct.
	pub(crate) fn candidate_commitment(&self) -> PeerMmrCommitmentV1 {
		self.commitment
	}

	fn validate(&self) -> Result<(), ContentError> {
		if self.domain != DOMAIN || self.registry_hash != REGISTRY_SHA256 {
			return Err(ContentError::IntegrityFailed);
		}
		if self.genesis_hash == [0; 32]
			|| self.finalized_hash == [0; 32]
			|| self.bucket == [0; 32]
			|| self.source_provider == [0; 32]
			|| self.target_provider == [0; 32]
			|| self.source_provider == self.target_provider
			|| self.source_service_key_version == 0
			|| self.target_service_key_version == 0
			|| self.source_service_key == [0; 32]
			|| self.target_service_key == [0; 32]
			|| self.source_service_key == self.target_service_key
			|| self.source_endpoint_hash == [0; 32]
			|| self.target_endpoint_hash == [0; 32]
		{
			return Err(ContentError::IntegrityFailed);
		}
		self.commitment.validate()?;
		Ok(())
	}

	fn validate_expected(&self, expected: &Self) -> Result<(), ContentError> {
		self.validate()?;
		expected.validate()?;
		if self != expected {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

/// Stable operation and request nonce used by the caller's singular replay table.
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerRequestIdentityV1 {
	operation_id: [u8; 16],
	request_nonce: [u8; 16],
}

impl PeerRequestIdentityV1 {
	/// Build a non-zero stable operation/nonce identity.
	pub(crate) fn new(
		operation_id: [u8; 16],
		request_nonce: [u8; 16],
	) -> Result<Self, ContentError> {
		let identity = Self { operation_id, request_nonce };
		identity.validate()?;
		Ok(identity)
	}

	fn validate(&self) -> Result<(), ContentError> {
		if self.operation_id == [0; 16] || self.request_nonce == [0; 16] {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

/// Stable replay key returned only after canonical authentication succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PeerReplayIdentityV1 {
	/// Stable idempotent replication operation.
	pub operation_id: [u8; 16],
	/// Exact caller nonce consumed by the singular replay table.
	pub request_nonce: [u8; 16],
	/// Canonical authenticated request hash.
	pub request_hash: [u8; 32],
}

/// Compact source-authenticated response proof retained without response payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PeerResponseProofV1 {
	response_hash: [u8; 32],
	signature: [u8; 64],
}

/// Compact target-authenticated request proof retained without repeated manifest bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PeerRequestProofV1 {
	request_nonce: [u8; 16],
	request_hash: [u8; 32],
	signature: [u8; 64],
}

impl PeerRequestProofV1 {
	/// Return the stable request nonce.
	pub(crate) fn request_nonce(&self) -> [u8; 16] {
		self.request_nonce
	}

	/// Return the canonical authenticated request digest.
	pub(crate) fn request_hash(&self) -> [u8; 32] {
		self.request_hash
	}

	/// Return the target service-key signature.
	pub(crate) fn signature(&self) -> [u8; 64] {
		self.signature
	}

	/// Reconstruct one persisted compact target proof.
	pub(crate) fn from_parts(
		request_nonce: [u8; 16],
		request_hash: [u8; 32],
		signature: [u8; 64],
	) -> Self {
		Self { request_nonce, request_hash, signature }
	}
}

impl PeerResponseProofV1 {
	/// Return the exact response digest signed by the source service key.
	pub(crate) fn response_hash(&self) -> [u8; 32] {
		self.response_hash
	}

	/// Return the source service-key signature over the response digest.
	pub(crate) fn signature(&self) -> [u8; 64] {
		self.signature
	}

	/// Reconstruct one persisted compact proof for fail-closed validation.
	pub(crate) fn from_parts(response_hash: [u8; 32], signature: [u8; 64]) -> Self {
		Self { response_hash, signature }
	}
}

/// One canonical object descriptor in the pinned bucket sequence.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerObjectV1 {
	cid: Vec<u8>,
	length: u64,
	sequence: u64,
	total_size: u64,
	data_root: [u8; 32],
	leaf_hash: [u8; 32],
	chunk_manifest_hash: [u8; 32],
	chunk_hashes: Vec<[u8; 32]>,
}

impl PeerObjectV1 {
	/// Build one manifest-bound MMR leaf descriptor for a pinned page.
	pub(crate) fn new(
		cid: &CanonicalCid,
		length: u64,
		sequence: u64,
		total_size: u64,
		chunk_hashes: Vec<[u8; 32]>,
	) -> Result<Self, ContentError> {
		let data_root = cid.digest();
		let leaf = MmrLeafV1 { data_root: H256::from(data_root), data_size: length, total_size };
		let object = Self {
			cid: cid.as_str().as_bytes().to_vec(),
			length,
			sequence,
			total_size,
			data_root,
			leaf_hash: sp_crypto_hashing::blake2_256(&leaf.encode()),
			chunk_manifest_hash: chunk_manifest_hash(&chunk_hashes),
			chunk_hashes,
		};
		object.validate()?;
		Ok(object)
	}

	/// Return the canonical CID text.
	pub(crate) fn cid(&self) -> &str {
		std::str::from_utf8(&self.cid).expect("validated peer CID is UTF-8")
	}

	/// Return `(length, sequence, cumulative total size)`.
	pub(crate) fn position(&self) -> (u64, u64, u64) {
		(self.length, self.sequence, self.total_size)
	}

	/// Return the authenticated per-chunk manifest.
	pub(crate) fn chunk_hashes(&self) -> &[[u8; 32]] {
		&self.chunk_hashes
	}

	/// Return the authenticated digest of the ordered per-chunk manifest.
	pub(crate) fn chunk_manifest_hash(&self) -> [u8; 32] {
		self.chunk_manifest_hash
	}

	fn validate(&self) -> Result<(), ContentError> {
		if self.cid.is_empty() || self.cid.len() > MAX_CID_BYTES {
			return Err(ContentError::SchemaInvalid);
		}
		let cid = std::str::from_utf8(&self.cid).map_err(|_| ContentError::SchemaInvalid)?;
		let cid = CanonicalCid::parse(cid)?;
		if self.length > MAX_STORED_BYTES {
			return Err(ContentError::ObjectTooLarge);
		}
		if self.length == 0 && self.data_root != sp_crypto_hashing::blake2_256(&[]) {
			return Err(ContentError::IntegrityFailed);
		}
		let chunk_count =
			if self.length == 0 { 0 } else { self.length.div_ceil(CHUNK_BYTES as u64) as usize };
		if self.chunk_hashes.len() != chunk_count
			|| self.chunk_hashes.len() > crate::MAX_CHUNKS
			|| self.chunk_hashes.iter().any(|hash| *hash == [0; 32])
			|| self.chunk_manifest_hash != chunk_manifest_hash(&self.chunk_hashes)
			|| self.data_root != cid.digest()
			|| self.total_size < self.length
		{
			return Err(ContentError::IntegrityFailed);
		}
		let leaf = MmrLeafV1 {
			data_root: H256::from(self.data_root),
			data_size: self.length,
			total_size: self.total_size,
		};
		if self.leaf_hash != sp_crypto_hashing::blake2_256(&leaf.encode()) {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

/// Complete expected page request, obtained from one finalized registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerPageExpectationV1 {
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	cursor: Option<PeerPageCursorV1>,
	limit: u16,
}

impl PeerPageExpectationV1 {
	/// Freeze the exact first or resumed page request expected by the source.
	pub(crate) fn new(
		context: PeerContextV1,
		identity: PeerRequestIdentityV1,
		cursor: Option<PeerPageCursorV1>,
		limit: u16,
	) -> Result<Self, ContentError> {
		context.validate()?;
		identity.validate()?;
		if limit == 0 || usize::from(limit) > MAX_PAGE_ITEMS {
			return Err(ContentError::SchemaInvalid);
		}
		validate_cursor(&context.commitment, cursor)?;
		Ok(Self { context, identity, cursor, limit })
	}
}

/// Target-authenticated request for one pinned object page.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerSyncPageRequestV1 {
	version: u8,
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	cursor: Option<PeerPageCursorV1>,
	limit: u16,
	signature: [u8; 64],
}

impl PeerSyncPageRequestV1 {
	/// Construct and sign one target-authenticated page request.
	pub(crate) fn new_signed(
		expected: &PeerPageExpectationV1,
		target: &ed25519::Pair,
	) -> Result<Self, ContentError> {
		let mut request = Self {
			version: VERSION,
			context: expected.context.clone(),
			identity: expected.identity,
			cursor: expected.cursor,
			limit: expected.limit,
			signature: [0; 64],
		};
		request.sign(target)?;
		request.validate_expected(expected)?;
		Ok(request)
	}

	/// Encode the already validated canonical request.
	pub(crate) fn encode_wire(&self) -> Vec<u8> {
		self.encode()
	}

	/// Return the requested cursor and page bound.
	pub(crate) fn page(&self) -> (Option<PeerPageCursorV1>, u16) {
		(self.cursor, self.limit)
	}

	/// Return the pinned registry context.
	pub(crate) fn context(&self) -> &PeerContextV1 {
		&self.context
	}

	fn request_hash(&self) -> [u8; 32] {
		digest(
			PAGE_REQUEST_TAG,
			&(self.version, &self.context, self.identity, self.cursor, self.limit).encode(),
		)
	}

	fn sign(&mut self, target: &ed25519::Pair) -> Result<(), ContentError> {
		self.validate_structure()?;
		if target.public().0 != self.context.target_service_key {
			return Err(ContentError::IntegrityFailed);
		}
		self.signature = target.sign(&self.request_hash()).0;
		Ok(())
	}

	fn validate_structure(&self) -> Result<(), ContentError> {
		if self.version != VERSION || self.limit == 0 || usize::from(self.limit) > MAX_PAGE_ITEMS {
			return Err(ContentError::SchemaInvalid);
		}
		self.context.validate()?;
		self.identity.validate()?;
		validate_cursor(&self.context.commitment, self.cursor)
	}

	fn validate_expected(&self, expected: &PeerPageExpectationV1) -> Result<(), ContentError> {
		self.validate_structure()?;
		self.context.validate_expected(&expected.context)?;
		expected.identity.validate()?;
		if self.identity != expected.identity
			|| self.cursor != expected.cursor
			|| self.limit != expected.limit
			|| !verify(self.context.target_service_key, self.request_hash(), self.signature)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	/// Decode, re-encode and authenticate before any caller performs I/O.
	pub(crate) fn decode_canonical(
		bytes: &[u8],
		expected: &PeerPageExpectationV1,
	) -> Result<(Self, PeerReplayIdentityV1), ContentError> {
		let request: Self = decode_canonical(bytes, MAX_REQUEST_ENCODED)?;
		request.validate_expected(expected)?;
		let replay = request.replay_identity();
		Ok((request, replay))
	}

	/// Decode and authenticate a canonical request without an external expectation.
	pub(crate) fn decode_authenticated(bytes: &[u8]) -> Result<Self, ContentError> {
		let request: Self = decode_canonical(bytes, MAX_REQUEST_ENCODED)?;
		request.validate_authentication()?;
		Ok(request)
	}

	/// Return the replay identity only after authenticating this exact request.
	pub(crate) fn authenticated_replay_identity(
		&self,
	) -> Result<PeerReplayIdentityV1, ContentError> {
		self.validate_authentication()?;
		Ok(self.replay_identity())
	}

	fn replay_identity(&self) -> PeerReplayIdentityV1 {
		PeerReplayIdentityV1 {
			operation_id: self.identity.operation_id,
			request_nonce: self.identity.request_nonce,
			request_hash: self.request_hash(),
		}
	}
}

/// Source-authenticated response to one exact page request.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerSyncPageResponseV1 {
	version: u8,
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	request_hash: [u8; 32],
	requested_cursor: Option<PeerPageCursorV1>,
	items: Vec<PeerObjectV1>,
	next_cursor: Option<PeerPageCursorV1>,
	response_hash: [u8; 32],
	signature: [u8; 64],
}

impl PeerSyncPageResponseV1 {
	/// Construct and sign one source-authenticated pinned page.
	pub(crate) fn new_signed(
		request: &PeerSyncPageRequestV1,
		items: Vec<PeerObjectV1>,
		next_cursor: Option<PeerPageCursorV1>,
		source: &ed25519::Pair,
	) -> Result<Self, ContentError> {
		request.validate_authentication()?;
		let mut response = Self {
			version: VERSION,
			context: request.context.clone(),
			identity: request.identity,
			request_hash: request.request_hash(),
			requested_cursor: request.cursor,
			items,
			next_cursor,
			response_hash: [0; 32],
			signature: [0; 64],
		};
		response.sign(source)?;
		response.validate_request(request)?;
		Ok(response)
	}

	/// Encode the already validated canonical response.
	pub(crate) fn encode_wire(&self) -> Vec<u8> {
		self.encode()
	}

	/// Return the verified page items.
	pub(crate) fn items(&self) -> &[PeerObjectV1] {
		&self.items
	}

	/// Return the authenticated continuation cursor.
	pub(crate) fn next_cursor(&self) -> Option<PeerPageCursorV1> {
		self.next_cursor
	}

	/// Return the compact source signature proof after full response verification.
	pub(crate) fn compact_proof(&self) -> PeerResponseProofV1 {
		PeerResponseProofV1 { response_hash: self.response_hash, signature: self.signature }
	}

	/// Revalidate persisted page provenance without retaining the original response bytes.
	pub(crate) fn verify_compact_proof(
		request: &PeerSyncPageRequestV1,
		items: Vec<PeerObjectV1>,
		next_cursor: Option<PeerPageCursorV1>,
		proof: PeerResponseProofV1,
	) -> Result<(), ContentError> {
		let response = Self {
			version: VERSION,
			context: request.context.clone(),
			identity: request.identity,
			request_hash: request.request_hash(),
			requested_cursor: request.cursor,
			items,
			next_cursor,
			response_hash: proof.response_hash,
			signature: proof.signature,
		};
		response.validate_request(request)
	}

	fn expected_response_hash(&self) -> [u8; 32] {
		digest(
			PAGE_RESPONSE_TAG,
			&(
				self.version,
				&self.context,
				self.identity,
				self.request_hash,
				self.requested_cursor,
				&self.items,
				self.next_cursor,
			)
				.encode(),
		)
	}

	fn sign(&mut self, source: &ed25519::Pair) -> Result<(), ContentError> {
		self.validate_structure()?;
		if source.public().0 != self.context.source_service_key {
			return Err(ContentError::IntegrityFailed);
		}
		self.response_hash = self.expected_response_hash();
		self.signature = source.sign(&self.response_hash).0;
		Ok(())
	}

	fn validate_structure(&self) -> Result<(), ContentError> {
		if self.version != VERSION || self.items.len() > MAX_PAGE_ITEMS {
			return Err(ContentError::SchemaInvalid);
		}
		self.context.validate()?;
		self.identity.validate()?;
		validate_cursor(&self.context.commitment, self.requested_cursor)?;
		for item in &self.items {
			item.validate()?;
		}
		let commitment = self.context.commitment;
		let end = commitment.end_exclusive()?;
		let (mut expected_sequence, mut prior_total) = match self.requested_cursor {
			Some(cursor) => (
				cursor.last_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?,
				cursor.cumulative_total,
			),
			None => (commitment.start_seq, commitment.predecessor_total_size),
		};
		if self.items.is_empty() {
			return Err(ContentError::IntegrityFailed);
		}
		for item in &self.items {
			if item.sequence != expected_sequence || item.sequence >= end {
				return Err(ContentError::IntegrityFailed);
			}
			let expected_total =
				prior_total.checked_add(item.length).ok_or(ContentError::IntegrityFailed)?;
			if item.total_size != expected_total {
				return Err(ContentError::IntegrityFailed);
			}
			prior_total = item.total_size;
			expected_sequence =
				expected_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		}
		let last = self.items.last().expect("non-empty page checked above");
		let continuation = PeerPageCursorV1::new(last.sequence, last.total_size);
		if last.sequence.checked_add(1) == Some(end) {
			if self.next_cursor.is_some() {
				return Err(ContentError::IntegrityFailed);
			}
		} else if self.next_cursor != Some(continuation) {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn validate_request(&self, request: &PeerSyncPageRequestV1) -> Result<(), ContentError> {
		request.validate_authentication()?;
		self.validate_structure()?;
		if self.context != request.context
			|| self.identity != request.identity
			|| self.request_hash != request.request_hash()
			|| self.requested_cursor != request.cursor
			|| self.items.len() > usize::from(request.limit)
			|| self.response_hash != self.expected_response_hash()
			|| !verify(self.context.source_service_key, self.response_hash, self.signature)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	/// Decode, re-encode and authenticate one response against its exact request.
	pub(crate) fn decode_canonical(
		bytes: &[u8],
		request: &PeerSyncPageRequestV1,
	) -> Result<Self, ContentError> {
		let response: Self = decode_canonical(bytes, MAX_PAGE_RESPONSE_ENCODED)?;
		response.validate_request(request)?;
		Ok(response)
	}
}

impl PeerSyncPageRequestV1 {
	fn validate_authentication(&self) -> Result<(), ContentError> {
		self.validate_structure()?;
		if !verify(self.context.target_service_key, self.request_hash(), self.signature) {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

/// Complete expected chunk request, obtained from the same pinned page and registry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerChunkExpectationV1 {
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	object: PeerObjectV1,
	chunk_index: u16,
	chunk_hash: [u8; 32],
}

impl PeerChunkExpectationV1 {
	/// Freeze one exact chunk request from an authenticated page manifest.
	pub(crate) fn new(
		context: PeerContextV1,
		identity: PeerRequestIdentityV1,
		object: PeerObjectV1,
		chunk_index: u16,
	) -> Result<Self, ContentError> {
		context.validate()?;
		identity.validate()?;
		object.validate()?;
		expected_chunk_length(object.length, chunk_index)?;
		let chunk_hash = object.chunk_hashes[usize::from(chunk_index)];
		Ok(Self { context, identity, object, chunk_index, chunk_hash })
	}
}

/// Target-authenticated request for one exact content chunk.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerChunkRequestV1 {
	version: u8,
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	object: PeerObjectV1,
	chunk_index: u16,
	chunk_hash: [u8; 32],
	signature: [u8; 64],
}

impl PeerChunkRequestV1 {
	/// Construct and sign a target-authenticated request for one manifest chunk.
	pub(crate) fn new_signed(
		expected: &PeerChunkExpectationV1,
		target: &ed25519::Pair,
	) -> Result<Self, ContentError> {
		let mut request = Self {
			version: VERSION,
			context: expected.context.clone(),
			identity: expected.identity,
			object: expected.object.clone(),
			chunk_index: expected.chunk_index,
			chunk_hash: expected.chunk_hash,
			signature: [0; 64],
		};
		request.sign(target)?;
		request.validate_expected(expected)?;
		Ok(request)
	}

	/// Encode the already validated canonical request.
	pub(crate) fn encode_wire(&self) -> Vec<u8> {
		self.encode()
	}

	/// Return the requested object and chunk identity.
	pub(crate) fn chunk(&self) -> (&PeerObjectV1, u16, [u8; 32]) {
		(&self.object, self.chunk_index, self.chunk_hash)
	}

	/// Return the pinned registry context.
	pub(crate) fn context(&self) -> &PeerContextV1 {
		&self.context
	}

	/// Return the compact target signature proof for durable provenance.
	pub(crate) fn compact_proof(&self) -> PeerRequestProofV1 {
		PeerRequestProofV1 {
			request_nonce: self.identity.request_nonce,
			request_hash: self.request_hash(),
			signature: self.signature,
		}
	}

	/// Revalidate compact persisted chunk-request provenance.
	pub(crate) fn verify_compact_proof(
		context: PeerContextV1,
		operation_id: [u8; 16],
		object: PeerObjectV1,
		chunk_index: u16,
		proof: PeerRequestProofV1,
	) -> Result<Self, ContentError> {
		object.validate()?;
		let chunk_hash = *object
			.chunk_hashes
			.get(usize::from(chunk_index))
			.ok_or(ContentError::ChunkOutOfOrder)?;
		let request = Self {
			version: VERSION,
			context,
			identity: PeerRequestIdentityV1::new(operation_id, proof.request_nonce)?,
			object,
			chunk_index,
			chunk_hash,
			signature: proof.signature,
		};
		request.validate_authentication()?;
		if request.request_hash() != proof.request_hash {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(request)
	}

	fn request_hash(&self) -> [u8; 32] {
		digest(
			CHUNK_REQUEST_TAG,
			&(
				self.version,
				&self.context,
				self.identity,
				&self.object,
				self.chunk_index,
				self.chunk_hash,
			)
				.encode(),
		)
	}

	fn sign(&mut self, target: &ed25519::Pair) -> Result<(), ContentError> {
		self.validate_structure()?;
		if target.public().0 != self.context.target_service_key {
			return Err(ContentError::IntegrityFailed);
		}
		self.signature = target.sign(&self.request_hash()).0;
		Ok(())
	}

	fn validate_structure(&self) -> Result<(), ContentError> {
		if self.version != VERSION || self.chunk_hash == [0; 32] {
			return Err(ContentError::SchemaInvalid);
		}
		self.context.validate()?;
		self.identity.validate()?;
		self.object.validate()?;
		expected_chunk_length(self.object.length, self.chunk_index)?;
		if self.object.chunk_hashes[usize::from(self.chunk_index)] != self.chunk_hash {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn validate_expected(&self, expected: &PeerChunkExpectationV1) -> Result<(), ContentError> {
		self.validate_structure()?;
		self.context.validate_expected(&expected.context)?;
		expected.identity.validate()?;
		expected.object.validate()?;
		if self.identity != expected.identity
			|| self.object != expected.object
			|| self.chunk_index != expected.chunk_index
			|| self.chunk_hash != expected.chunk_hash
			|| !verify(self.context.target_service_key, self.request_hash(), self.signature)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	/// Decode, re-encode and authenticate before any caller reads content bytes.
	pub(crate) fn decode_canonical(
		bytes: &[u8],
		expected: &PeerChunkExpectationV1,
	) -> Result<(Self, PeerReplayIdentityV1), ContentError> {
		let request: Self = decode_canonical(bytes, MAX_REQUEST_ENCODED)?;
		request.validate_expected(expected)?;
		let replay = request.replay_identity();
		Ok((request, replay))
	}

	/// Decode and authenticate a canonical request without an external expectation.
	pub(crate) fn decode_authenticated(bytes: &[u8]) -> Result<Self, ContentError> {
		let request: Self = decode_canonical(bytes, MAX_REQUEST_ENCODED)?;
		request.validate_authentication()?;
		Ok(request)
	}

	/// Return the replay identity only after authenticating this exact request.
	pub(crate) fn authenticated_replay_identity(
		&self,
	) -> Result<PeerReplayIdentityV1, ContentError> {
		self.validate_authentication()?;
		Ok(self.replay_identity())
	}

	fn replay_identity(&self) -> PeerReplayIdentityV1 {
		PeerReplayIdentityV1 {
			operation_id: self.identity.operation_id,
			request_nonce: self.identity.request_nonce,
			request_hash: self.request_hash(),
		}
	}

	fn validate_authentication(&self) -> Result<(), ContentError> {
		self.validate_structure()?;
		if !verify(self.context.target_service_key, self.request_hash(), self.signature) {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}
}

/// Source-authenticated response containing one bounded verified chunk.
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub(crate) struct PeerChunkResponseV1 {
	version: u8,
	context: PeerContextV1,
	identity: PeerRequestIdentityV1,
	request_hash: [u8; 32],
	object: PeerObjectV1,
	chunk_index: u16,
	chunk_hash: [u8; 32],
	chunk: Vec<u8>,
	response_hash: [u8; 32],
	signature: [u8; 64],
}

impl PeerChunkResponseV1 {
	/// Construct and sign one source-authenticated bounded chunk response.
	pub(crate) fn new_signed(
		request: &PeerChunkRequestV1,
		chunk: Vec<u8>,
		source: &ed25519::Pair,
	) -> Result<Self, ContentError> {
		request.validate_authentication()?;
		let mut response = Self {
			version: VERSION,
			context: request.context.clone(),
			identity: request.identity,
			request_hash: request.request_hash(),
			object: request.object.clone(),
			chunk_index: request.chunk_index,
			chunk_hash: request.chunk_hash,
			chunk,
			response_hash: [0; 32],
			signature: [0; 64],
		};
		response.sign(source)?;
		response.validate_request(request)?;
		Ok(response)
	}

	/// Encode the already validated canonical response.
	pub(crate) fn encode_wire(&self) -> Vec<u8> {
		self.encode()
	}

	/// Return the verified object, chunk index and bytes.
	pub(crate) fn verified_chunk(&self) -> (&PeerObjectV1, u16, &[u8]) {
		(&self.object, self.chunk_index, &self.chunk)
	}

	/// Return the compact source signature proof after full chunk verification.
	pub(crate) fn compact_proof(&self) -> PeerResponseProofV1 {
		PeerResponseProofV1 { response_hash: self.response_hash, signature: self.signature }
	}

	/// Revalidate persisted chunk provenance without retaining duplicate chunk bytes.
	pub(crate) fn verify_compact_proof(
		request: &PeerChunkRequestV1,
		proof: PeerResponseProofV1,
	) -> Result<(), ContentError> {
		request.validate_authentication()?;
		let expected = digest(
			CHUNK_RESPONSE_TAG,
			&(
				VERSION,
				&request.context,
				request.identity,
				request.request_hash(),
				&request.object,
				request.chunk_index,
				request.chunk_hash,
			)
				.encode(),
		);
		if proof.response_hash != expected
			|| !verify(request.context.source_service_key, expected, proof.signature)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn expected_response_hash(&self) -> [u8; 32] {
		digest(
			CHUNK_RESPONSE_TAG,
			&(
				self.version,
				&self.context,
				self.identity,
				self.request_hash,
				&self.object,
				self.chunk_index,
				self.chunk_hash,
			)
				.encode(),
		)
	}

	fn sign(&mut self, source: &ed25519::Pair) -> Result<(), ContentError> {
		self.validate_structure()?;
		if source.public().0 != self.context.source_service_key {
			return Err(ContentError::IntegrityFailed);
		}
		self.response_hash = self.expected_response_hash();
		self.signature = source.sign(&self.response_hash).0;
		Ok(())
	}

	fn validate_structure(&self) -> Result<(), ContentError> {
		if self.version != VERSION {
			return Err(ContentError::SchemaInvalid);
		}
		if self.chunk.len() > CHUNK_BYTES {
			return Err(ContentError::ChunkTooLarge);
		}
		self.context.validate()?;
		self.identity.validate()?;
		self.object.validate()?;
		let expected = expected_chunk_length(self.object.length, self.chunk_index)?;
		if self.chunk.len() != expected {
			return Err(ContentError::LengthMismatch);
		}
		if sp_crypto_hashing::blake2_256(&self.chunk) != self.chunk_hash {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn validate_request(&self, request: &PeerChunkRequestV1) -> Result<(), ContentError> {
		request.validate_authentication()?;
		self.validate_structure()?;
		if self.context != request.context
			|| self.identity != request.identity
			|| self.request_hash != request.request_hash()
			|| self.object != request.object
			|| self.chunk_index != request.chunk_index
			|| self.chunk_hash != request.chunk_hash
			|| self.response_hash != self.expected_response_hash()
			|| !verify(self.context.source_service_key, self.response_hash, self.signature)
		{
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	/// Decode, re-encode and authenticate one bounded response before persistence.
	pub(crate) fn decode_canonical(
		bytes: &[u8],
		request: &PeerChunkRequestV1,
	) -> Result<Self, ContentError> {
		if bytes.len() > MAX_CHUNK_RESPONSE_ENCODED {
			return Err(ContentError::ObjectTooLarge);
		}
		let response: Self = decode_canonical(bytes, MAX_CHUNK_RESPONSE_ENCODED)?;
		response.validate_request(request)?;
		Ok(response)
	}
}

fn validate_cursor(
	commitment: &PeerMmrCommitmentV1,
	cursor: Option<PeerPageCursorV1>,
) -> Result<(), ContentError> {
	commitment.validate()?;
	let Some(cursor) = cursor else { return Ok(()) };
	let end = commitment.end_exclusive()?;
	let next = cursor.last_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
	if cursor.last_sequence < commitment.start_seq
		|| next >= end
		|| cursor.cumulative_total < commitment.predecessor_total_size
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn expected_chunk_length(length: u64, index: u16) -> Result<usize, ContentError> {
	if length > MAX_STORED_BYTES {
		return Err(ContentError::ObjectTooLarge);
	}
	let start = u64::from(index)
		.checked_mul(CHUNK_BYTES as u64)
		.ok_or(ContentError::ChunkOutOfOrder)?;
	if start >= length {
		return Err(ContentError::ChunkOutOfOrder);
	}
	Ok((length - start).min(CHUNK_BYTES as u64) as usize)
}

fn chunk_manifest_hash(chunk_hashes: &[[u8; 32]]) -> [u8; 32] {
	digest(b"cord/provider/peer-replication/chunk-manifest/v1", &chunk_hashes.encode())
}

fn digest(tag: &[u8], encoded: &[u8]) -> [u8; 32] {
	let mut input = Vec::with_capacity(tag.len() + encoded.len());
	input.extend_from_slice(tag);
	input.extend_from_slice(encoded);
	sp_crypto_hashing::blake2_256(&input)
}

fn verify(key: [u8; 32], message: [u8; 32], signature: [u8; 64]) -> bool {
	ed25519::Pair::verify(
		&ed25519::Signature::from_raw(signature),
		&message,
		&ed25519::Public::from_raw(key),
	)
}

fn decode_canonical<T: Decode + Encode>(bytes: &[u8], max: usize) -> Result<T, ContentError> {
	if bytes.len() > max {
		return Err(ContentError::SchemaInvalid);
	}
	let mut input = bytes;
	let value = T::decode(&mut input).map_err(|_| ContentError::SchemaInvalid)?;
	if !input.is_empty() || value.encode() != bytes {
		return Err(ContentError::SchemaInvalid);
	}
	Ok(value)
}

#[cfg(test)]
mod tests {
	use sha2::{Digest as _, Sha256};

	use super::*;

	fn pair(seed: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[seed; 32])
	}

	fn cid(seed: u8) -> Vec<u8> {
		CanonicalCid::from_digest([seed; 32]).to_string().into_bytes()
	}

	fn context() -> PeerContextV1 {
		context_with_commitment(PeerMmrCommitmentV1::new([17; 32], 0, 128, 0).unwrap())
	}

	fn context_with_commitment(commitment: PeerMmrCommitmentV1) -> PeerContextV1 {
		PeerContextV1::new(
			[1; 32],
			[2; 32],
			500,
			[3; 32],
			[4; 32],
			[5; 32],
			7,
			pair(11).public().0,
			9,
			pair(12).public().0,
			[13; 32],
			[14; 32],
			commitment,
		)
		.unwrap()
	}

	fn identity() -> PeerRequestIdentityV1 {
		PeerRequestIdentityV1::new([15; 16], [16; 16]).unwrap()
	}

	fn object(sequence: u64) -> PeerObjectV1 {
		let length = CHUNK_BYTES as u64;
		let total_size = (sequence + 1) * length;
		let chunk_hashes = vec![[sequence as u8 | 1; 32]];
		PeerObjectV1::new(
			&CanonicalCid::from_digest([sequence as u8; 32]),
			length,
			sequence,
			total_size,
			chunk_hashes,
		)
		.unwrap()
	}

	fn sized_object(sequence: u64, length: u64, total_size: u64, seed: u8) -> PeerObjectV1 {
		let digest = if length == 0 { sp_crypto_hashing::blake2_256(&[]) } else { [seed; 32] };
		let chunk_hashes = if length == 0 { Vec::new() } else { vec![[seed | 1; 32]] };
		PeerObjectV1::new(
			&CanonicalCid::from_digest(digest),
			length,
			sequence,
			total_size,
			chunk_hashes,
		)
		.unwrap()
	}

	fn page_request() -> (PeerSyncPageRequestV1, PeerPageExpectationV1) {
		let expected =
			PeerPageExpectationV1::new(context(), identity(), None, MAX_PAGE_ITEMS as u16).unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap();
		(request, expected)
	}

	fn chunk_request(bytes: &[u8]) -> (PeerChunkRequestV1, PeerChunkExpectationV1) {
		let data_root = sp_crypto_hashing::blake2_256(bytes);
		let length = bytes.len() as u64;
		let chunk_hashes = vec![data_root];
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest(data_root),
			length,
			0,
			length,
			chunk_hashes,
		)
		.unwrap();
		let expected = PeerChunkExpectationV1::new(context(), identity(), object, 0).unwrap();
		let request = PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap();
		(request, expected)
	}

	#[test]
	fn registry_hash_and_page_request_identity_are_canonical() {
		assert_eq!(Sha256::digest(REGISTRY).as_slice(), REGISTRY_SHA256);
		assert!(PeerMmrCommitmentV1::new([1; 32], 0, 1, 1).is_err());
		assert!(PeerMmrCommitmentV1::new([1; 32], 1, 1, 0).is_ok());
		let (request, expected) = page_request();
		let bytes = request.encode();
		let (decoded, replay) = PeerSyncPageRequestV1::decode_canonical(&bytes, &expected).unwrap();
		assert_eq!(decoded, request);
		assert_eq!(replay.operation_id, expected.identity.operation_id);
		assert_eq!(replay.request_nonce, expected.identity.request_nonce);
		assert_eq!(replay.request_hash, request.request_hash());

		let mut changed_nonce = request.clone();
		changed_nonce.identity.request_nonce[0] ^= 1;
		changed_nonce.sign(&pair(12)).unwrap();
		assert_ne!(changed_nonce.request_hash(), request.request_hash());
		assert!(
			PeerSyncPageRequestV1::decode_canonical(&changed_nonce.encode(), &expected).is_err()
		);

		let mut trailing = bytes;
		trailing.push(0);
		assert!(PeerSyncPageRequestV1::decode_canonical(&trailing, &expected).is_err());
	}

	#[test]
	fn page_request_rejects_every_snapshot_audience_key_endpoint_and_bound_change() {
		let (request, expected) = page_request();
		let mut cases = Vec::new();
		for change in 0..15 {
			let mut changed = request.clone();
			match change {
				0 => changed.context.domain[0] ^= 1,
				1 => changed.context.registry_hash[0] ^= 1,
				2 => changed.context.genesis_hash[0] ^= 1,
				3 => changed.context.finalized_hash[0] ^= 1,
				4 => changed.context.finalized_number += 1,
				5 => changed.context.bucket[0] ^= 1,
				6 => changed.context.source_provider[0] ^= 1,
				7 => changed.context.target_provider[0] ^= 1,
				8 => changed.context.source_service_key_version += 1,
				9 => changed.context.target_service_key_version += 1,
				10 => changed.context.source_service_key[0] ^= 1,
				11 => changed.context.target_service_key[0] ^= 1,
				12 => changed.context.source_endpoint_hash[0] ^= 1,
				13 => changed.context.target_endpoint_hash[0] ^= 1,
				14 => changed.context.commitment.mmr_root[0] ^= 1,
				_ => unreachable!(),
			}
			cases.push(changed);
		}
		for changed in cases {
			assert!(PeerSyncPageRequestV1::decode_canonical(&changed.encode(), &expected).is_err());
		}

		let mut wrong_operation = request.clone();
		wrong_operation.identity.operation_id[0] ^= 1;
		wrong_operation.sign(&pair(12)).unwrap();
		assert!(
			PeerSyncPageRequestV1::decode_canonical(&wrong_operation.encode(), &expected).is_err()
		);
		let mut wrong_cursor = request.clone();
		wrong_cursor.cursor = Some(PeerPageCursorV1::new(1, 2 * CHUNK_BYTES as u64));
		wrong_cursor.sign(&pair(12)).unwrap();
		assert!(PeerSyncPageRequestV1::decode_canonical(&wrong_cursor.encode(), &expected).is_err());
		let mut wrong_signature = request.clone();
		wrong_signature.signature[0] ^= 1;
		assert!(
			PeerSyncPageRequestV1::decode_canonical(&wrong_signature.encode(), &expected).is_err()
		);
		for limit in [0, 129] {
			let mut invalid = request.clone();
			invalid.limit = limit;
			assert!(PeerSyncPageRequestV1::decode_canonical(&invalid.encode(), &expected).is_err());
		}

		let mut shared_gateway = context();
		shared_gateway.target_endpoint_hash = shared_gateway.source_endpoint_hash;
		let shared_expected =
			PeerPageExpectationV1::new(shared_gateway, identity(), None, 1).unwrap();
		let shared_request =
			PeerSyncPageRequestV1::new_signed(&shared_expected, &pair(12)).unwrap();
		assert!(PeerSyncPageRequestV1::decode_canonical(
			&shared_request.encode_wire(),
			&shared_expected
		)
		.is_ok());
	}

	#[test]
	fn pages_are_exact_prefixes_of_the_committed_mmr_range() {
		let commitment = PeerMmrCommitmentV1::new([40; 32], 10, 3, 100).unwrap();
		let context = context_with_commitment(commitment);
		assert_eq!(context.candidate_commitment(), commitment);
		assert_eq!(commitment.mmr_root(), [40; 32]);
		assert_eq!(commitment.sequence_range(), (10, 13));
		assert_eq!(commitment.predecessor_total_size(), 100);
		assert!(commitment.verify_local_root([40; 32]).is_ok());
		assert!(commitment.verify_local_root([41; 32]).is_err());

		let first_expected =
			PeerPageExpectationV1::new(context.clone(), identity(), None, 2).unwrap();
		let first_request = PeerSyncPageRequestV1::new_signed(&first_expected, &pair(12)).unwrap();
		let mut wrong_commitment = first_request.clone();
		wrong_commitment.context.commitment.mmr_root[0] ^= 1;
		wrong_commitment.sign(&pair(12)).unwrap();
		assert!(PeerSyncPageRequestV1::decode_canonical(
			&wrong_commitment.encode_wire(),
			&first_expected,
		)
		.is_err());
		let cursor = PeerPageCursorV1::new(11, 112);
		let first = PeerSyncPageResponseV1::new_signed(
			&first_request,
			vec![sized_object(10, 5, 105, 50), sized_object(11, 7, 112, 51)],
			Some(cursor),
			&pair(11),
		)
		.unwrap();
		let first =
			PeerSyncPageResponseV1::decode_canonical(&first.encode_wire(), &first_request).unwrap();
		assert_eq!(first.next_cursor().unwrap().position(), (11, 112));

		let next_identity = PeerRequestIdentityV1::new([15; 16], [18; 16]).unwrap();
		let second_expected =
			PeerPageExpectationV1::new(context, next_identity, first.next_cursor(), 2).unwrap();
		let second_request =
			PeerSyncPageRequestV1::new_signed(&second_expected, &pair(12)).unwrap();
		let second = PeerSyncPageResponseV1::new_signed(
			&second_request,
			vec![sized_object(12, 9, 121, 52)],
			None,
			&pair(11),
		)
		.unwrap();
		assert!(PeerSyncPageResponseV1::decode_canonical(&second.encode_wire(), &second_request)
			.is_ok());
		assert_eq!(second.next_cursor(), None);
	}

	#[test]
	fn pages_reject_early_terminal_gaps_overrun_and_invalid_totals() {
		let commitment = PeerMmrCommitmentV1::new([60; 32], 10, 3, 100).unwrap();
		let context = context_with_commitment(commitment);
		let expected = PeerPageExpectationV1::new(context.clone(), identity(), None, 2).unwrap();
		let request = PeerSyncPageRequestV1::new_signed(&expected, &pair(12)).unwrap();

		assert!(PeerSyncPageResponseV1::new_signed(
			&request,
			vec![sized_object(10, 5, 105, 70)],
			None,
			&pair(11),
		)
		.is_err());
		assert!(PeerSyncPageResponseV1::new_signed(&request, Vec::new(), None, &pair(11)).is_err());
		assert!(PeerSyncPageResponseV1::new_signed(
			&request,
			vec![sized_object(10, 5, 105, 70), sized_object(12, 7, 112, 71)],
			Some(PeerPageCursorV1::new(12, 112)),
			&pair(11),
		)
		.is_err());
		assert!(PeerSyncPageResponseV1::new_signed(
			&request,
			vec![sized_object(10, 5, 105, 70), sized_object(11, 7, 105, 71)],
			Some(PeerPageCursorV1::new(11, 105)),
			&pair(11),
		)
		.is_err());

		let resumed = PeerPageExpectationV1::new(
			context,
			identity(),
			Some(PeerPageCursorV1::new(11, 112)),
			2,
		)
		.unwrap();
		let resumed = PeerSyncPageRequestV1::new_signed(&resumed, &pair(12)).unwrap();
		assert!(PeerSyncPageResponseV1::new_signed(
			&resumed,
			vec![sized_object(13, 1, 113, 72)],
			None,
			&pair(11),
		)
		.is_err());
		assert!(PeerPageExpectationV1::new(
			resumed.context.clone(),
			identity(),
			Some(PeerPageCursorV1::new(12, 121)),
			1,
		)
		.is_err());
	}

	#[test]
	fn empty_objects_require_the_canonical_empty_raw_cid() {
		let empty_digest = sp_crypto_hashing::blake2_256(&[]);
		assert!(PeerObjectV1::new(&CanonicalCid::from_digest(empty_digest), 0, 0, 0, Vec::new(),)
			.is_ok());
		assert!(
			PeerObjectV1::new(&CanonicalCid::from_digest([99; 32]), 0, 0, 0, Vec::new(),).is_err()
		);
	}

	#[test]
	fn pinned_page_response_enforces_128_cursor_hash_and_source_authentication() {
		let (request, expected) = page_request();
		let (request, _) =
			PeerSyncPageRequestV1::decode_canonical(&request.encode(), &expected).unwrap();
		let items = (0..MAX_PAGE_ITEMS as u64).map(object).collect::<Vec<_>>();
		let response =
			PeerSyncPageResponseV1::new_signed(&request, items, None, &pair(11)).unwrap();
		assert_eq!(
			PeerSyncPageResponseV1::decode_canonical(&response.encode_wire(), &request).unwrap(),
			response
		);
		assert_eq!(response.items().len(), MAX_PAGE_ITEMS);
		assert_eq!(response.next_cursor(), None);

		let mut oversized = response.clone();
		oversized.items.push(object(MAX_PAGE_ITEMS as u64));
		oversized.next_cursor = Some(PeerPageCursorV1::new(
			MAX_PAGE_ITEMS as u64,
			(MAX_PAGE_ITEMS as u64 + 1) * CHUNK_BYTES as u64,
		));
		assert!(PeerSyncPageResponseV1::decode_canonical(&oversized.encode(), &request).is_err());
		let mut regressed = response.clone();
		regressed.items[1].sequence = regressed.items[0].sequence;
		assert!(PeerSyncPageResponseV1::decode_canonical(&regressed.encode(), &request).is_err());
		let mut omitted = response.clone();
		omitted.items.remove(1);
		omitted.response_hash = omitted.expected_response_hash();
		omitted.signature = pair(11).sign(&omitted.response_hash).0;
		assert!(PeerSyncPageResponseV1::decode_canonical(&omitted.encode(), &request).is_err());
		let mut changed_manifest = response.clone();
		changed_manifest.items[0].chunk_hashes[0][0] ^= 1;
		changed_manifest.response_hash = changed_manifest.expected_response_hash();
		changed_manifest.signature = pair(11).sign(&changed_manifest.response_hash).0;
		assert!(
			PeerSyncPageResponseV1::decode_canonical(&changed_manifest.encode(), &request).is_err()
		);
		let mut wrong_next = response.clone();
		wrong_next.next_cursor = Some(PeerPageCursorV1::new(
			MAX_PAGE_ITEMS as u64 - 1,
			MAX_PAGE_ITEMS as u64 * CHUNK_BYTES as u64,
		));
		assert!(PeerSyncPageResponseV1::decode_canonical(&wrong_next.encode(), &request).is_err());
		let mut wrong_request = response.clone();
		wrong_request.request_hash[0] ^= 1;
		assert!(
			PeerSyncPageResponseV1::decode_canonical(&wrong_request.encode(), &request).is_err()
		);
		let mut wrong_source = response;
		wrong_source.signature = pair(12).sign(&wrong_source.response_hash).0;
		assert!(PeerSyncPageResponseV1::decode_canonical(&wrong_source.encode(), &request).is_err());
	}

	#[test]
	fn chunk_request_binds_object_sequence_index_hash_and_replay_identity() {
		let bytes = vec![17; CHUNK_BYTES];
		let (request, expected) = chunk_request(&bytes);
		let encoded = request.encode();
		let (_, replay) = PeerChunkRequestV1::decode_canonical(&encoded, &expected).unwrap();
		assert_eq!(replay.request_hash, request.request_hash());

		for change in 0..5 {
			let mut changed = request.clone();
			match change {
				0 => changed.object.cid = cid(99),
				1 => changed.object.length -= 1,
				2 => changed.object.sequence += 1,
				3 => changed.chunk_index = 1,
				4 => changed.chunk_hash[0] ^= 1,
				_ => unreachable!(),
			}
			assert!(PeerChunkRequestV1::decode_canonical(&changed.encode(), &expected).is_err());
		}
		let mut wrong_nonce = request;
		wrong_nonce.identity.request_nonce[0] ^= 1;
		wrong_nonce.sign(&pair(12)).unwrap();
		assert!(PeerChunkRequestV1::decode_canonical(&wrong_nonce.encode(), &expected).is_err());

		let (mut invented_hash, expected) = chunk_request(&bytes);
		invented_hash.chunk_hash[0] ^= 1;
		invented_hash.signature = pair(12).sign(&invented_hash.request_hash()).0;
		assert!(PeerChunkRequestV1::decode_canonical(&invented_hash.encode(), &expected).is_err());

		let hashes = vec![[23; 32]; crate::MAX_CHUNKS];
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest([24; 32]),
			MAX_STORED_BYTES,
			0,
			MAX_STORED_BYTES,
			hashes.clone(),
		)
		.unwrap();
		let expected = PeerChunkExpectationV1::new(
			context(),
			identity(),
			object,
			(crate::MAX_CHUNKS - 1) as u16,
		)
		.unwrap();
		let max_request = PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap();
		assert!(max_request.encode_wire().len() > 4 * 1024);
		assert!(max_request.encode_wire().len() <= MAX_REQUEST_ENCODED);
		assert!(PeerChunkRequestV1::decode_canonical(&max_request.encode_wire(), &expected).is_ok());

		let mut too_many = hashes;
		too_many.push([25; 32]);
		assert!(PeerObjectV1::new(
			&CanonicalCid::from_digest([24; 32]),
			MAX_STORED_BYTES,
			0,
			MAX_STORED_BYTES,
			too_many,
		)
		.is_err());
	}

	#[test]
	fn chunk_response_accepts_262144_and_rejects_corruption_trailing_and_oversize() {
		let bytes = vec![21; CHUNK_BYTES];
		let (request, expected) = chunk_request(&bytes);
		let (request, _) =
			PeerChunkRequestV1::decode_canonical(&request.encode(), &expected).unwrap();
		let response = PeerChunkResponseV1::new_signed(&request, bytes, &pair(11)).unwrap();
		let encoded = response.encode_wire();
		assert_eq!(PeerChunkResponseV1::decode_canonical(&encoded, &request).unwrap(), response);
		assert_eq!(response.verified_chunk().1, 0);
		assert_eq!(response.verified_chunk().2.len(), CHUNK_BYTES);
		let request_proof = request.compact_proof();
		let compact_request = PeerChunkRequestV1::verify_compact_proof(
			request.context.clone(),
			request.identity.operation_id,
			request.object.clone(),
			request.chunk_index,
			request_proof,
		)
		.unwrap();
		let response_proof = response.compact_proof();
		PeerChunkResponseV1::verify_compact_proof(&compact_request, response_proof).unwrap();
		let mut bad_target_signature = request_proof.signature();
		bad_target_signature[0] ^= 1;
		assert!(PeerChunkRequestV1::verify_compact_proof(
			request.context.clone(),
			request.identity.operation_id,
			request.object.clone(),
			request.chunk_index,
			PeerRequestProofV1::from_parts(
				request_proof.request_nonce(),
				request_proof.request_hash(),
				bad_target_signature,
			),
		)
		.is_err());
		let mut bad_source_signature = response_proof.signature();
		bad_source_signature[0] ^= 1;
		assert!(PeerChunkResponseV1::verify_compact_proof(
			&compact_request,
			PeerResponseProofV1::from_parts(response_proof.response_hash(), bad_source_signature,),
		)
		.is_err());
		let mut bad_response_hash = response_proof.response_hash();
		bad_response_hash[0] ^= 1;
		assert!(PeerChunkResponseV1::verify_compact_proof(
			&compact_request,
			PeerResponseProofV1::from_parts(bad_response_hash, response_proof.signature(),),
		)
		.is_err());

		let mut corrupt = response.clone();
		corrupt.chunk[0] ^= 1;
		assert!(PeerChunkResponseV1::decode_canonical(&corrupt.encode(), &request).is_err());
		let mut wrong_response_hash = response.clone();
		wrong_response_hash.response_hash[0] ^= 1;
		assert!(
			PeerChunkResponseV1::decode_canonical(&wrong_response_hash.encode(), &request).is_err()
		);
		let mut trailing = encoded;
		trailing.push(0);
		assert!(PeerChunkResponseV1::decode_canonical(&trailing, &request).is_err());
		let mut oversized = response;
		oversized.chunk.push(0);
		assert!(PeerChunkResponseV1::decode_canonical(&oversized.encode(), &request).is_err());
	}

	#[test]
	fn maximum_manifest_and_chunk_response_round_trips_with_derived_bound() {
		let chunk = vec![31; CHUNK_BYTES];
		let chunk_hash = sp_crypto_hashing::blake2_256(&chunk);
		let object = PeerObjectV1::new(
			&CanonicalCid::from_digest([32; 32]),
			MAX_STORED_BYTES,
			0,
			MAX_STORED_BYTES,
			vec![chunk_hash; crate::MAX_CHUNKS],
		)
		.unwrap();
		let expected = PeerChunkExpectationV1::new(
			context(),
			identity(),
			object,
			(crate::MAX_CHUNKS - 1) as u16,
		)
		.unwrap();
		let request = PeerChunkRequestV1::new_signed(&expected, &pair(12)).unwrap();
		let response = PeerChunkResponseV1::new_signed(&request, chunk, &pair(11)).unwrap();
		let encoded = response.encode_wire();

		assert!(encoded.len() > CHUNK_BYTES + 4 * 1024);
		assert!(encoded.len() <= MAX_CHUNK_RESPONSE_ENCODED);
		assert_eq!(PeerChunkResponseV1::decode_canonical(&encoded, &request).unwrap(), response);
		assert!(PeerChunkResponseV1::new_signed(&request, vec![31; CHUNK_BYTES + 1], &pair(11),)
			.is_err());
	}
}
