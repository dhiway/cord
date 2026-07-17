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

//! Private durable primary-side checkpoint quorum collection.

use std::{
	collections::HashMap,
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::{Decode, Encode};
use orbis_storage_runtime_api::{
	CheckpointDutyInfo, CheckpointDutyPhase, ProviderDutyAuthority, ProviderDutyRole,
	RESPONSE_VERSION,
};
use pallet_orbis_storage_provider::{CheckpointContextV1, CommitmentPayloadV2, ReplicaSignature};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, ed25519, Pair as _, H256};
use sp_crypto_hashing::blake2_256;

use super::{
	checkpoint_outbox::CheckpointSubmissionInputV2,
	checkpoint_quorum::{
		checkpoint_context_digest, checkpoint_digest, ReplicaConfirmationRequestV1,
		ReplicaConfirmationResponseV1,
	},
	validate_proposal, PreparedCheckpointProposalV2, ServiceKeySigner,
};
use crate::ContentError;

const ROOT: &str = "checkpoint-primary-quorum-v1";
const VERSION: u8 = 1;
const RECORD_DOMAIN: &[u8] = b"cord/provider/checkpoint-primary-quorum-record/v1";
const MAX_RECORD_BYTES: usize = 256 * 1024;
const MAX_RECORDS: usize = 8_192;
// Atomic replacement creates at most one process-specific temporary artifact.
const MAX_TEMP_ARTIFACTS: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PrimaryQuorumState {
	Collecting,
	QuorumReady,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedConfirmationV1 {
	provider: String,
	service_key_version: u64,
	service_key: String,
	request: String,
	request_hash: String,
	response: Option<String>,
	response_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimaryQuorumRecordV1 {
	version: u8,
	state: PrimaryQuorumState,
	proposal: PreparedCheckpointProposalV2,
	selected: Vec<SelectedConfirmationV1>,
	confirmations_scale: Option<String>,
	record_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrimaryQuorumSnapshotV1 {
	pub state: PrimaryQuorumState,
	pub requests: Vec<Vec<u8>>,
	pub confirmations: Option<Vec<ReplicaSignature<AccountId32>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimaryQuorumFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
}

pub(crate) struct CheckpointPrimaryQuorumStore {
	root: PathBuf,
	records: RwLock<HashMap<String, PrimaryQuorumRecordV1>>,
	fault: RwLock<Option<PrimaryQuorumFault>>,
	poisoned: RwLock<bool>,
}

impl CheckpointPrimaryQuorumStore {
	pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let mut records = HashMap::new();
		let mut visited = 0usize;
		let mut temp_artifacts = Vec::new();
		for item in fs::read_dir(&root).map_err(io_error)? {
			visited = visited.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			if visited > MAX_RECORDS + MAX_TEMP_ARTIFACTS {
				return Err(ContentError::IntegrityFailed)
			}
			let item = item.map_err(io_error)?;
			let name = item.file_name().to_string_lossy().into_owned();
			if crate::bounded_io::is_json_temp_artifact(&name) {
				if temp_artifacts.len() >= MAX_TEMP_ARTIFACTS ||
					!item.file_type().map_err(io_error)?.is_file()
				{
					return Err(ContentError::IntegrityFailed)
				}
				temp_artifacts.push(item.path());
				continue
			}
			if !name.ends_with(".json") || !item.file_type().map_err(io_error)?.is_file() {
				return Err(ContentError::IntegrityFailed)
			}
			let bytes = crate::bounded_io::read_regular_file(
				item.path(),
				MAX_RECORD_BYTES as u64,
			)?;
			if records.len() >= MAX_RECORDS {
				return Err(ContentError::IntegrityFailed)
			}
			let record: PrimaryQuorumRecordV1 =
				serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
			let key = validate_record(&record)?;
			if name != format!("{key}.json") || records.insert(key, record).is_some() {
				return Err(ContentError::IntegrityFailed)
			}
		}
		crate::bounded_io::remove_validated_temp_artifacts(&root, &temp_artifacts)?;
		Ok(Self {
			root,
			records: RwLock::new(records),
			fault: RwLock::new(None),
			poisoned: RwLock::new(false),
		})
	}

	pub(crate) fn inject_fault_once(&self, fault: PrimaryQuorumFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	pub(crate) fn outstanding_proposals(
		&self,
	) -> Result<Vec<PreparedCheckpointProposalV2>, ContentError> {
		let records = self.records.read().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed);
		}
		let mut proposals = records
			.values()
			.filter(|record| {
				record.state == PrimaryQuorumState::Collecting
					&& record.selected.iter().any(|selected| selected.response.is_none())
			})
			.map(|record| record.proposal.clone())
			.collect::<Vec<_>>();
		proposals.sort_by(|left, right| {
			(&left.bucket_id, left.nonce, left.start_seq, &left.duty_id).cmp(&(
				&right.bucket_id,
				right.nonce,
				right.start_seq,
				&right.duty_id,
			))
		});
		Ok(proposals)
	}

	pub(crate) fn begin(
		&self,
		proposal: &PreparedCheckpointProposalV2,
		signer: &dyn ServiceKeySigner,
	) -> Result<PrimaryQuorumSnapshotV1, ContentError> {
		validate_proposal(proposal)?;
		let key = tuple_key(proposal)?;
		{
			let records = self.records.read().map_err(|_| lock_error())?;
			if *self.poisoned.read().map_err(|_| lock_error())? {
				return Err(ContentError::IntegrityFailed)
			}
			if let Some(record) = records.get(&key) {
				return if record.proposal == *proposal {
					snapshot(record)
				} else {
					Err(ContentError::IdempotencyConflict)
				}
			}
		}
		if signer.public_key() != decode_32(&proposal.service_key)? {
			return Err(ContentError::IntegrityFailed)
		}
		let duty = decode_duty(proposal)?;
		let targets = selected_targets(&duty)?;
		let payload = decode_exact::<CommitmentPayloadV2<H256, u32>>(&proposal.payload_scale)?;
		let context = decode_exact::<CheckpointContextV1<H256>>(&proposal.context_scale)?;
		let primary_provider = decode_account(&proposal.primary_provider)?;
		let primary_service_key = ed25519::Public::from_raw(signer.public_key());
		let primary_signature = ed25519::Signature::from_raw(decode_64(&proposal.signature)?);
		let primary_context_signature =
			ed25519::Signature::from_raw(decode_64(&proposal.context_signature)?);
		let mut selected = Vec::with_capacity(2);
		for target in targets {
			let mut request = ReplicaConfirmationRequestV1 {
				version: VERSION,
				proposal_record_hash: decode_32(&proposal.record_hash)?,
				target_provider: target.provider.clone(),
				target_service_key_version: target.active_service_key_version,
				target_service_key: ed25519::Public::from_raw(target.active_service_key),
				primary_provider: primary_provider.clone(),
				duty_id: duty.duty_id,
				payload,
				context,
				primary_service_key,
				primary_signature,
				primary_context_signature,
				auth_signature: ed25519::Signature::from_raw([0; 64]),
			};
			request.auth_signature =
				ed25519::Signature::from_raw(signer.sign_message(&request.auth_message()));
			let bytes = request.encode();
			selected.push(SelectedConfirmationV1 {
				provider: account_hex(&target.provider),
				service_key_version: target.active_service_key_version,
				service_key: hex::encode(target.active_service_key),
				request: hex::encode(&bytes),
				request_hash: hex::encode(blake2_256(&bytes)),
				response: None,
				response_hash: None,
			});
		}
		let mut record = PrimaryQuorumRecordV1 {
			version: VERSION,
			state: PrimaryQuorumState::Collecting,
			proposal: proposal.clone(),
			selected,
			confirmations_scale: None,
			record_hash: String::new(),
		};
		record.record_hash = record_hash(&record)?;
		let mut records = self.records.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(existing) = records.get(&key) {
			return if existing.proposal == *proposal {
				snapshot(existing)
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		if records.len() >= MAX_RECORDS {
			return Err(ContentError::ProviderRecoveryTableFull)
		}
		if let Err(error) = self.persist(&key, &record) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		records.insert(key, record.clone());
		snapshot(&record)
	}

	pub(crate) fn accept_response(
		&self,
		proposal: &PreparedCheckpointProposalV2,
		response_bytes: &[u8],
	) -> Result<PrimaryQuorumSnapshotV1, ContentError> {
		validate_proposal(proposal)?;
		let key = tuple_key(proposal)?;
		let response = ReplicaConfirmationResponseV1::decode_canonical(response_bytes)?;
		let mut records = self.records.write().map_err(|_| lock_error())?;
		if *self.poisoned.read().map_err(|_| lock_error())? {
			return Err(ContentError::IntegrityFailed)
		}
		let current = records.get(&key).cloned().ok_or(ContentError::NotFound)?;
		if current.proposal != *proposal {
			return Err(ContentError::IdempotencyConflict)
		}
		let target_hex = account_hex(&response.target_provider);
		let index = current
			.selected
			.iter()
			.position(|target| target.provider == target_hex)
			.ok_or(ContentError::IntegrityFailed)?;
		if let Some(existing) = current.selected[index].response.as_ref() {
			return if existing == &hex::encode(response_bytes) {
				snapshot(&current)
			} else {
				Err(ContentError::IdempotencyConflict)
			}
		}
		validate_response(&current, index, &response)?;
		let mut next = current;
		next.selected[index].response = Some(hex::encode(response_bytes));
		next.selected[index].response_hash = Some(hex::encode(blake2_256(response_bytes)));
		let confirmations = next
			.selected
			.iter()
			.filter_map(|item| item.response.as_ref())
			.map(|bytes| {
				ReplicaConfirmationResponseV1::decode_canonical(
					&hex::decode(bytes).map_err(|_| ContentError::IntegrityFailed)?,
				)
				.map(|response| response.confirmation)
			})
			.collect::<Result<Vec<_>, ContentError>>()?;
		if confirmations.len() == 2 {
			next.state = PrimaryQuorumState::QuorumReady;
			next.confirmations_scale = Some(hex::encode(confirmations.encode()));
		}
		next.record_hash = record_hash(&next)?;
		if let Err(error) = self.persist(&key, &next) {
			*self.poisoned.write().map_err(|_| lock_error())? = true;
			return Err(error)
		}
		records.insert(key, next.clone());
		snapshot(&next)
	}

	fn persist(&self, key: &str, record: &PrimaryQuorumRecordV1) -> Result<(), ContentError> {
		if validate_record(record)? != key {
			return Err(ContentError::IntegrityFailed)
		}
		let bytes = serde_json::to_vec(record).map_err(io_error)?;
		if bytes.len() > MAX_RECORD_BYTES {
			return Err(ContentError::IntegrityFailed)
		}
		let temp = self.root.join(format!("{key}.json.tmp-{}", std::process::id()));
		let mut file = File::create(&temp).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		self.trip(PrimaryQuorumFault::BeforeTempFsync)?;
		file.sync_all().map_err(io_error)?;
		self.trip(PrimaryQuorumFault::AfterTempFsync)?;
		fs::rename(&temp, self.root.join(format!("{key}.json"))).map_err(io_error)?;
		self.trip(PrimaryQuorumFault::AfterRename)?;
		File::open(&self.root)
			.and_then(|directory| directory.sync_all())
			.map_err(io_error)?;
		self.trip(PrimaryQuorumFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: PrimaryQuorumFault) -> Result<(), ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Err(ContentError::Io(format!("injected primary quorum fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

fn selected_targets(
	duty: &CheckpointDutyInfo<AccountId32, H256, u32>,
) -> Result<Vec<ProviderDutyAuthority<AccountId32, H256, u32>>, ContentError> {
	if duty.replicas.iter().any(|replica| replica == &duty.primary) ||
		duty.replicas
			.iter()
			.enumerate()
			.any(|(index, replica)| duty.replicas[..index].contains(replica)) ||
		duty.authorities.len() != duty.replicas.len().saturating_add(1)
	{
		return Err(ContentError::IntegrityFailed)
	}
	let expected = std::iter::once((&duty.primary, ProviderDutyRole::Primary, 0u8)).chain(
		duty.replicas.iter().enumerate().map(|(index, replica)| {
			(replica, ProviderDutyRole::Replica, index.saturating_add(1) as u8)
		}),
	);
	if expected.zip(&duty.authorities).any(|((provider, role, order), authority)| {
		authority.provider != *provider || authority.role != role || authority.order != order
	}) {
		return Err(ContentError::IntegrityFailed)
	}
	let initiator = duty.initiator.as_ref().ok_or(ContentError::IntegrityFailed)?;
	let candidates = match duty.phase {
		CheckpointDutyPhase::Primary if initiator == &duty.primary => duty.replicas.clone(),
		CheckpointDutyPhase::ReplicaFallback if duty.replicas.contains(initiator) => {
			let mut providers = vec![duty.primary.clone()];
			providers.extend(duty.replicas.iter().filter(|item| *item != initiator).cloned());
			providers
		},
		_ => return Err(ContentError::IntegrityFailed),
	};
	let mut targets = candidates
		.into_iter()
		.filter(|provider| provider != initiator)
		.map(|provider| {
			duty.authorities
				.iter()
				.find(|authority| authority.provider == provider)
				.cloned()
				.ok_or(ContentError::IntegrityFailed)
		})
		.collect::<Result<Vec<_>, _>>()?;
	targets.retain(|target| {
		target.active_service_key_version > 0 &&
			target.may_sign &&
			target.eligible &&
			target.organization_sla_eligible &&
			!target.overdue_challenge &&
			target.exclusion.is_none()
	});
	targets.sort_by_key(|target| target.provider.encode());
	if targets.len() < 2 || targets.windows(2).any(|pair| pair[0].provider == pair[1].provider) {
		return Err(ContentError::IntegrityFailed)
	}
	targets.truncate(2);
	Ok(targets)
}

fn validate_response(
	record: &PrimaryQuorumRecordV1,
	index: usize,
	response: &ReplicaConfirmationResponseV1,
) -> Result<(), ContentError> {
	let selected = record.selected.get(index).ok_or(ContentError::IntegrityFailed)?;
	let request = ReplicaConfirmationRequestV1::decode_canonical(
		&hex::decode(&selected.request).map_err(|_| ContentError::IntegrityFailed)?,
	)?;
	if response.proposal_record_hash != request.proposal_record_hash ||
		response.duty_id != request.duty_id ||
		response.target_provider != request.target_provider ||
		response.confirmation.provider != request.target_provider ||
		response.confirmation.service_key != request.target_service_key ||
		!ed25519::Pair::verify(
			&response.confirmation.signature,
			&checkpoint_digest(&request.payload),
			&response.confirmation.service_key,
		) || !ed25519::Pair::verify(
		&response.confirmation.context_signature,
		&checkpoint_context_digest(&request.context),
		&response.confirmation.service_key,
	) {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(())
}

fn validate_record(record: &PrimaryQuorumRecordV1) -> Result<String, ContentError> {
	if record.version != VERSION || record.record_hash != record_hash(record)? {
		return Err(ContentError::IntegrityFailed)
	}
	validate_proposal(&record.proposal)?;
	let duty = decode_duty(&record.proposal)?;
	let expected = selected_targets(&duty)?;
	if record.selected.len() != 2 {
		return Err(ContentError::IntegrityFailed)
	}
	for (index, selected) in record.selected.iter().enumerate() {
		let target = &expected[index];
		let request_bytes =
			hex::decode(&selected.request).map_err(|_| ContentError::IntegrityFailed)?;
		let request = ReplicaConfirmationRequestV1::decode_canonical(&request_bytes)?;
		if selected.provider != account_hex(&target.provider) ||
			selected.service_key_version != target.active_service_key_version ||
			selected.service_key != hex::encode(target.active_service_key) ||
			selected.request_hash != hex::encode(blake2_256(&request_bytes)) ||
			request.target_provider != target.provider ||
			request.target_service_key_version != target.active_service_key_version ||
			request.target_service_key.0 != target.active_service_key ||
			request.proposal_record_hash != decode_32(&record.proposal.record_hash)? ||
			request.duty_id != duty.duty_id ||
			request.primary_provider != decode_account(&record.proposal.primary_provider)? ||
			request.primary_service_key.0 != decode_32(&record.proposal.service_key)? ||
			hex::encode(request.payload.encode()) != record.proposal.payload_scale ||
			hex::encode(request.context.encode()) != record.proposal.context_scale ||
			hex::encode(request.primary_signature.0) != record.proposal.signature ||
			hex::encode(request.primary_context_signature.0) != record.proposal.context_signature ||
			!ed25519::Pair::verify(
				&request.auth_signature,
				&request.auth_message(),
				&request.primary_service_key,
			) {
			return Err(ContentError::IntegrityFailed)
		}
		match (&selected.response, &selected.response_hash) {
			(None, None) => {},
			(Some(encoded), Some(hash)) => {
				let bytes = hex::decode(encoded).map_err(|_| ContentError::IntegrityFailed)?;
				if hash != &hex::encode(blake2_256(&bytes)) {
					return Err(ContentError::IntegrityFailed)
				}
				let response = ReplicaConfirmationResponseV1::decode_canonical(&bytes)?;
				validate_response(record, index, &response)?;
			},
			_ => return Err(ContentError::IntegrityFailed),
		}
	}
	let responses = record.selected.iter().filter(|item| item.response.is_some()).count();
	match record.state {
		PrimaryQuorumState::Collecting if responses < 2 && record.confirmations_scale.is_none() => {
		},
		PrimaryQuorumState::QuorumReady if responses == 2 => {
			let confirmations = decode_exact::<Vec<ReplicaSignature<AccountId32>>>(
				record.confirmations_scale.as_deref().ok_or(ContentError::IntegrityFailed)?,
			)?;
			let expected_confirmations = record
				.selected
				.iter()
				.map(|item| {
					let bytes =
						hex::decode(item.response.as_deref().ok_or(ContentError::IntegrityFailed)?)
							.map_err(|_| ContentError::IntegrityFailed)?;
					Ok(ReplicaConfirmationResponseV1::decode_canonical(&bytes)?.confirmation)
				})
				.collect::<Result<Vec<_>, ContentError>>()?;
			if confirmations != expected_confirmations ||
				confirmations.len() != 2 ||
				confirmations
					.iter()
					.map(|item| account_hex(&item.provider))
					.ne(record.selected.iter().map(|item| item.provider.clone()))
			{
				return Err(ContentError::IntegrityFailed)
			}
		},
		_ => return Err(ContentError::IntegrityFailed),
	}
	tuple_key(&record.proposal)
}

fn snapshot(record: &PrimaryQuorumRecordV1) -> Result<PrimaryQuorumSnapshotV1, ContentError> {
	validate_record(record)?;
	let confirmations = record
		.confirmations_scale
		.as_deref()
		.map(decode_exact::<Vec<ReplicaSignature<AccountId32>>>)
		.transpose()?;
	Ok(PrimaryQuorumSnapshotV1 {
		state: record.state,
		requests: record
			.selected
			.iter()
			.filter(|item| item.response.is_none())
			.map(|item| hex::decode(&item.request).map_err(|_| ContentError::IntegrityFailed))
			.collect::<Result<_, _>>()?,
		confirmations,
	})
}

pub(crate) fn submission_input(
	proposal: &PreparedCheckpointProposalV2,
	snapshot: &PrimaryQuorumSnapshotV1,
) -> Result<CheckpointSubmissionInputV2, ContentError> {
	validate_proposal(proposal)?;
	if snapshot.state != PrimaryQuorumState::QuorumReady || !snapshot.requests.is_empty() {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(CheckpointSubmissionInputV2 {
		primary: decode_account(&proposal.primary_provider)?,
		domain: b"cord/storage/checkpoint/v2".to_vec(),
		payload: decode_exact(&proposal.payload_scale)?,
		context: decode_exact(&proposal.context_scale)?,
		window_start: proposal.window_start,
		window_end: proposal.window_end,
		service_key: ed25519::Public::from_raw(decode_32(&proposal.service_key)?),
		primary_signature: ed25519::Signature::from_raw(decode_64(&proposal.signature)?),
		primary_context_signature: ed25519::Signature::from_raw(decode_64(
			&proposal.context_signature,
		)?),
		confirmations: snapshot.confirmations.clone().ok_or(ContentError::IntegrityFailed)?,
	})
}

fn decode_duty(
	proposal: &PreparedCheckpointProposalV2,
) -> Result<CheckpointDutyInfo<AccountId32, H256, u32>, ContentError> {
	let duty = decode_exact::<CheckpointDutyInfo<AccountId32, H256, u32>>(&proposal.duty_scale)?;
	if duty.response_version != RESPONSE_VERSION {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(duty)
}

fn decode_exact<T: Decode + Encode>(encoded: &str) -> Result<T, ContentError> {
	let bytes = hex::decode(encoded).map_err(|_| ContentError::IntegrityFailed)?;
	if hex::encode(&bytes) != encoded {
		return Err(ContentError::IntegrityFailed)
	}
	let mut input = &bytes[..];
	let value = T::decode(&mut input).map_err(|_| ContentError::IntegrityFailed)?;
	if !input.is_empty() || value.encode() != bytes {
		return Err(ContentError::IntegrityFailed)
	}
	Ok(value)
}

fn tuple_key(proposal: &PreparedCheckpointProposalV2) -> Result<String, ContentError> {
	let mut input = b"cord/provider/checkpoint-primary-quorum-key/v1".to_vec();
	input.extend_from_slice(&decode_32(&proposal.bucket_id)?);
	input.extend_from_slice(&proposal.nonce.to_be_bytes());
	input.extend_from_slice(&proposal.start_seq.to_be_bytes());
	Ok(hex::encode(blake2_256(&input)))
}

fn record_hash(record: &PrimaryQuorumRecordV1) -> Result<String, ContentError> {
	let mut canonical = record.clone();
	canonical.record_hash.clear();
	let mut input = RECORD_DOMAIN.to_vec();
	input.extend_from_slice(&serde_json::to_vec(&canonical).map_err(io_error)?);
	Ok(hex::encode(blake2_256(&input)))
}

fn decode_32(value: &str) -> Result<[u8; 32], ContentError> {
	hex::decode(value)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn decode_64(value: &str) -> Result<[u8; 64], ContentError> {
	hex::decode(value)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

fn decode_account(value: &str) -> Result<AccountId32, ContentError> {
	Ok(AccountId32::new(decode_32(value)?))
}

fn account_hex(account: &AccountId32) -> String {
	hex::encode(<AccountId32 as AsRef<[u8]>>::as_ref(account))
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("checkpoint primary quorum lock poisoned".into())
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use orbis_storage_runtime_api::{CheckpointDutyMode, CommitmentInfo, ProviderDutyExclusion};
	use tempfile::TempDir;

	pub(crate) fn pair(id: u8) -> ed25519::Pair {
		ed25519::Pair::from_seed(&[id.saturating_add(10); 32])
	}

	fn authority(
		id: u8,
		role: ProviderDutyRole,
		order: u8,
		may_initiate: bool,
	) -> ProviderDutyAuthority<AccountId32, H256, u32> {
		ProviderDutyAuthority {
			provider: AccountId32::new([id; 32]),
			role,
			order,
			active_service_key_version: 5,
			active_service_key: pair(id).public().0,
			endpoint_hash: H256::repeat_byte(id),
			organization_sla_eligible: true,
			overdue_challenge: false,
			eligible: true,
			may_sign: true,
			may_initiate,
			exclusion: None,
			initiation_exclusion: None,
			confirmed_checkpoint: None,
		}
	}

	pub(crate) fn proposal(fallback: bool, replicas: u8) -> PreparedCheckpointProposalV2 {
		proposal_for_bucket(fallback, replicas, 4)
	}

	fn proposal_for_bucket(
		fallback: bool,
		replicas: u8,
		bucket: u8,
	) -> PreparedCheckpointProposalV2 {
		assert!(replicas >= 2);
		let primary = AccountId32::new([1; 32]);
		let replica_ids = (2..replicas.saturating_add(2)).collect::<Vec<_>>();
		let replica_accounts =
			replica_ids.iter().map(|id| AccountId32::new([*id; 32])).collect::<Vec<_>>();
		let initiator_id = if fallback { 2 } else { 1 };
		let initiator = AccountId32::new([initiator_id; 32]);
		let mut authorities = vec![authority(1, ProviderDutyRole::Primary, 0, !fallback)];
		authorities.extend(replica_ids.iter().enumerate().map(|(index, id)| {
			authority(
				*id,
				ProviderDutyRole::Replica,
				index.saturating_add(1) as u8,
				fallback && *id == 2,
			)
		}));
		let duty = CheckpointDutyInfo {
			response_version: RESPONSE_VERSION,
			commons_genesis_hash: H256::repeat_byte(10),
			commons_spec_version: 1,
			commons_transaction_version: 1,
			commons_metadata_hash: H256::repeat_byte(11),
			duty_id: H256::repeat_byte(5),
			bucket_id: H256::repeat_byte(bucket),
			primary,
			replicas: replica_accounts,
			authorities,
			initiator: Some(initiator.clone()),
			phase: if fallback {
				CheckpointDutyPhase::ReplicaFallback
			} else {
				CheckpointDutyPhase::Primary
			},
			mode: CheckpointDutyMode::Standard,
			snapshot_checkpoint: 100,
			snapshot_hash: H256::repeat_byte(12),
			due_at: 100,
			grace_until: 110,
			expected_nonce: 100,
			scheduled_at: 99,
			previous_commitment: None::<CommitmentInfo<H256>>,
			previous_checkpoint: None,
			expected_next_start_seq: 0,
			required_primary_confirmations: 1,
			required_replica_confirmations: 2,
		};
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id: duty.bucket_id,
			commitment: pallet_orbis_storage_provider::CommitmentV1 {
				mmr_root: H256::repeat_byte(22),
				start_seq: 0,
				leaf_count: 1,
			},
			nonce: 100,
		};
		let payload_scale = payload.encode();
		let digest = checkpoint_digest(&payload);
		let context = CheckpointContextV1 {
			version: 1,
			genesis_hash: duty.commons_genesis_hash,
			spec_version: duty.commons_spec_version,
			transaction_version: duty.commons_transaction_version,
			metadata_hash: duty.commons_metadata_hash,
			finalized_hash: duty.snapshot_hash,
			duty_id: duty.duty_id,
			v2_digest: digest,
		};
		let context_scale = context.encode();
		let context_digest = checkpoint_context_digest(&context);
		let signer = pair(initiator_id);
		let duty_scale = duty.encode();
		let mut proposal = PreparedCheckpointProposalV2 {
			version: 2,
			duty_id: hex::encode(duty.duty_id.as_bytes()),
			duty_fingerprint: hex::encode(blake2_256(&duty_scale)),
			duty_scale: hex::encode(duty_scale),
			finalized_number: 111,
			finalized_hash: "0d".repeat(32),
			snapshot_checkpoint: 100,
			snapshot_hash: hex::encode(duty.snapshot_hash.as_bytes()),
			service_key_version: 5,
			service_key: hex::encode(signer.public().0),
			primary_provider: account_hex(&initiator),
			bucket_id: hex::encode(duty.bucket_id.as_bytes()),
			window_start: 100,
			window_end: 110,
			nonce: 100,
			start_seq: 0,
			leaf_count: 1,
			mmr_root: hex::encode(payload.commitment.mmr_root.as_bytes()),
			payload_scale: hex::encode(payload_scale),
			digest: hex::encode(digest),
			signature: hex::encode(signer.sign(&digest).0),
			context_scale: hex::encode(context_scale),
			context_digest: hex::encode(context_digest),
			context_signature: hex::encode(signer.sign(&context_digest).0),
			state: "prepared".into(),
			record_hash: String::new(),
		};
		proposal.record_hash = super::super::proposal_record_hash(&proposal).unwrap();
		validate_proposal(&proposal).unwrap();
		proposal
	}

	pub(crate) fn response(request_bytes: &[u8]) -> Vec<u8> {
		let request = ReplicaConfirmationRequestV1::decode_canonical(request_bytes).unwrap();
		let provider_bytes: &[u8] = request.target_provider.as_ref();
		let signer = pair(provider_bytes[0]);
		ReplicaConfirmationResponseV1 {
			version: 1,
			proposal_record_hash: request.proposal_record_hash,
			target_provider: request.target_provider.clone(),
			duty_id: request.duty_id,
			confirmation: ReplicaSignature {
				provider: request.target_provider,
				service_key: signer.public(),
				signature: signer.sign(&checkpoint_digest(&request.payload)),
				context_signature: signer.sign(&checkpoint_context_digest(&request.context)),
			},
		}
		.encode()
	}

	#[test]
	fn primary_and_fallback_freeze_exact_sorted_pairs_across_replica_counts() {
		for (fallback, replicas, expected) in [
			(false, 2, vec![2, 3]),
			(false, 3, vec![2, 3]),
			(false, 4, vec![2, 3]),
			(true, 2, vec![1, 3]),
			(true, 3, vec![1, 3]),
			(true, 4, vec![1, 3]),
		] {
			let temp = TempDir::new().unwrap();
			let proposal = proposal(fallback, replicas);
			let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
			let signer = pair(if fallback { 2 } else { 1 });
			let snapshot = store.begin(&proposal, &signer).unwrap();
			assert_eq!(snapshot.state, PrimaryQuorumState::Collecting);
			assert_eq!(snapshot.requests.len(), 2);
			let selected = snapshot
				.requests
				.iter()
				.map(|bytes| {
					let request = ReplicaConfirmationRequestV1::decode_canonical(bytes).unwrap();
					assert!(ed25519::Pair::verify(
						&request.auth_signature,
						&request.auth_message(),
						&request.primary_service_key,
					));
					let provider: &[u8] = request.target_provider.as_ref();
					provider[0]
				})
				.collect::<Vec<_>>();
			assert_eq!(selected, expected);
			assert_eq!(store.begin(&proposal, &signer).unwrap(), snapshot);
		}
	}

	#[test]
	fn one_confirmation_reopens_then_exact_two_become_immutable_ready() {
		let temp = TempDir::new().unwrap();
		let proposal = proposal(true, 3);
		let signer = pair(2);
		let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		let started = store.begin(&proposal, &signer).unwrap();
		let first = response(&started.requests[0]);
		let collecting = store.accept_response(&proposal, &first).unwrap();
		assert_eq!(collecting.state, PrimaryQuorumState::Collecting);
		drop(store);
		let reopened = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		assert_eq!(reopened.begin(&proposal, &signer).unwrap(), collecting);
		let second = response(&started.requests[1]);
		let ready = reopened.accept_response(&proposal, &second).unwrap();
		assert_eq!(ready.state, PrimaryQuorumState::QuorumReady);
		let confirmations = ready.confirmations.as_ref().unwrap();
		assert_eq!(confirmations.len(), 2);
		assert!(confirmations[0].provider.encode() < confirmations[1].provider.encode());
		assert_eq!(reopened.accept_response(&proposal, &second).unwrap(), ready);
		assert_eq!(reopened.accept_response(&proposal, &first).unwrap(), ready);
		drop(reopened);
		assert_eq!(
			CheckpointPrimaryQuorumStore::open(temp.path())
				.unwrap()
				.begin(&proposal, &signer)
				.unwrap(),
			ready
		);
	}

	#[test]
	fn terminal_early_records_do_not_starve_later_outstanding_quorums_after_reopen() {
		let temp = TempDir::new().unwrap();
		let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		for bucket in 1..=70u8 {
			let proposal = proposal_for_bucket(false, 2, bucket);
			let started = store.begin(&proposal, &pair(1)).unwrap();
			if bucket <= 64 {
				store.accept_response(&proposal, &response(&started.requests[0])).unwrap();
				store.accept_response(&proposal, &response(&started.requests[1])).unwrap();
			}
		}
		assert_eq!(store.outstanding_proposals().unwrap().len(), 6);
		drop(store);
		let reopened = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		let outstanding = reopened.outstanding_proposals().unwrap();
		assert_eq!(outstanding.len(), 6);
		assert!(outstanding
			.iter()
			.all(|proposal| decode_32(&proposal.bucket_id).unwrap()[0] > 64));
	}

	#[test]
	fn quorum_emits_one_canonical_outbox_record_across_reopen() {
		let temp = TempDir::new().unwrap();
		let proposal = proposal(false, 2);
		let quorum = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		let started = quorum.begin(&proposal, &pair(1)).unwrap();
		quorum.accept_response(&proposal, &response(&started.requests[0])).unwrap();
		let ready = quorum.accept_response(&proposal, &response(&started.requests[1])).unwrap();
		let outbox =
			super::super::checkpoint_outbox::CheckpointOutboxV2::open(temp.path()).unwrap();
		let first = outbox.enqueue(&submission_input(&proposal, &ready).unwrap()).unwrap();
		let replay = outbox.enqueue(&submission_input(&proposal, &ready).unwrap()).unwrap();
		assert_eq!(first, replay);
		drop(outbox);
		let reopened =
			super::super::checkpoint_outbox::CheckpointOutboxV2::open(temp.path()).unwrap();
		assert_eq!(reopened.enqueue(&submission_input(&proposal, &ready).unwrap()).unwrap(), first);
		assert_eq!(reopened.pending_submissions().unwrap().len(), 1);
	}

	#[test]
	fn unselected_wrong_key_signature_proposal_and_changed_response_fail_closed() {
		let temp = TempDir::new().unwrap();
		let proposal = proposal(false, 4);
		let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		let started = store.begin(&proposal, &pair(1)).unwrap();
		let canonical = response(&started.requests[0]);
		let mut decoded = ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();

		decoded.target_provider = AccountId32::new([4; 32]);
		decoded.confirmation.provider = decoded.target_provider.clone();
		assert!(store.accept_response(&proposal, &decoded.encode()).is_err());

		let mut wrong_key = ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();
		wrong_key.confirmation.service_key = pair(9).public();
		assert!(store.accept_response(&proposal, &wrong_key.encode()).is_err());

		let mut wrong_signature =
			ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();
		wrong_signature.confirmation.signature = pair(9).sign(&[0; 32]);
		assert!(store.accept_response(&proposal, &wrong_signature.encode()).is_err());

		let mut wrong_proposal =
			ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();
		wrong_proposal.proposal_record_hash = [99; 32];
		assert!(store.accept_response(&proposal, &wrong_proposal.encode()).is_err());

		let mut wrong_duty = ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();
		wrong_duty.duty_id = H256::repeat_byte(99);
		assert!(store.accept_response(&proposal, &wrong_duty.encode()).is_err());

		store.accept_response(&proposal, &canonical).unwrap();
		let mut changed = ReplicaConfirmationResponseV1::decode_canonical(&canonical).unwrap();
		changed.confirmation.context_signature = pair(9).sign(&[1; 32]);
		assert_eq!(
			store.accept_response(&proposal, &changed.encode()),
			Err(ContentError::IdempotencyConflict)
		);

		let mut changed_proposal = proposal.clone();
		changed_proposal.finalized_number += 1;
		changed_proposal.record_hash =
			super::super::proposal_record_hash(&changed_proposal).unwrap();
		assert_eq!(
			store.begin(&changed_proposal, &pair(1)),
			Err(ContentError::IdempotencyConflict)
		);
	}

	#[test]
	fn duplicate_self_ineligible_and_recomputed_record_tamper_are_rejected() {
		for case in 0..4 {
			let temp = TempDir::new().unwrap();
			let mut proposal = proposal(false, if case < 2 { 3 } else { 2 });
			let mut duty = decode_duty(&proposal).unwrap();
			match case {
				0 => duty.replicas[1] = duty.replicas[0].clone(),
				1 => duty.replicas[0] = duty.primary.clone(),
				2 => duty.authorities[1].eligible = false,
				3 => duty.authorities[1].exclusion = Some(ProviderDutyExclusion::Inactive),
				_ => unreachable!(),
			}
			let bytes = duty.encode();
			proposal.duty_scale = hex::encode(&bytes);
			proposal.duty_fingerprint = hex::encode(blake2_256(&bytes));
			proposal.record_hash = super::super::proposal_record_hash(&proposal).unwrap();
			assert!(CheckpointPrimaryQuorumStore::open(temp.path())
				.unwrap()
				.begin(&proposal, &pair(1))
				.is_err());
		}

		let temp = TempDir::new().unwrap();
		let proposal = proposal(false, 2);
		let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
		store.begin(&proposal, &pair(1)).unwrap();
		drop(store);
		let path = fs::read_dir(temp.path().join(ROOT)).unwrap().next().unwrap().unwrap().path();
		let mut record: PrimaryQuorumRecordV1 =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		record.selected.swap(0, 1);
		record.record_hash = record_hash(&record).unwrap();
		fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
		assert!(CheckpointPrimaryQuorumStore::open(temp.path()).is_err());
	}

	#[test]
	fn capacity_and_crash_seams_preserve_exact_old_or_new_records() {
		let capacity = TempDir::new().unwrap();
		let proposal_value = proposal(false, 2);
		let store = CheckpointPrimaryQuorumStore::open(capacity.path()).unwrap();
		let first = store.begin(&proposal_value, &pair(1)).unwrap();
		let existing = store.records.read().unwrap().values().next().unwrap().clone();
		let mut records = store.records.write().unwrap();
		for index in 0..MAX_RECORDS - 1 {
			records.entry(format!("{index:064x}")).or_insert_with(|| existing.clone());
		}
		drop(records);
		assert_eq!(store.begin(&proposal_value, &pair(1)).unwrap(), first);
		assert_eq!(
			store.begin(&proposal_for_bucket(false, 2, 5), &pair(1)),
			Err(ContentError::ProviderRecoveryTableFull)
		);

		for fault in [
			PrimaryQuorumFault::BeforeTempFsync,
			PrimaryQuorumFault::AfterTempFsync,
			PrimaryQuorumFault::AfterRename,
			PrimaryQuorumFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let proposal = proposal(false, 2);
			let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(store.begin(&proposal, &pair(1)).is_err());
			assert!(store.begin(&proposal, &pair(1)).is_err());
			drop(store);
			let reopened = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
			assert!(reopened.begin(&proposal, &pair(1)).is_ok(), "fault {fault:?}");
		}

		for fault in [
			PrimaryQuorumFault::BeforeTempFsync,
			PrimaryQuorumFault::AfterTempFsync,
			PrimaryQuorumFault::AfterRename,
			PrimaryQuorumFault::AfterDirectoryFsync,
		] {
			let temp = TempDir::new().unwrap();
			let proposal = proposal(false, 2);
			let store = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
			let started = store.begin(&proposal, &pair(1)).unwrap();
			let confirmation = response(&started.requests[0]);
			store.inject_fault_once(fault).unwrap();
			assert!(store.accept_response(&proposal, &confirmation).is_err());
			drop(store);
			let reopened = CheckpointPrimaryQuorumStore::open(temp.path()).unwrap();
			assert!(reopened.accept_response(&proposal, &confirmation).is_ok(), "fault {fault:?}");
		}
	}

	#[test]
	fn recovery_rejects_temp_artifact_flood() {
		let temp = TempDir::new().unwrap();
		let root = temp.path().join(ROOT);
		fs::create_dir_all(&root).unwrap();
		fs::write(root.join("first.json.tmp-1"), b"partial").unwrap();
		fs::write(root.join("second.json.tmp-1"), b"partial").unwrap();
		assert!(matches!(
			CheckpointPrimaryQuorumStore::open(temp.path()),
			Err(ContentError::IntegrityFailed)
		));
		assert!(root.join("first.json.tmp-1").exists());
		assert!(root.join("second.json.tmp-1").exists());
	}
}
