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

//! Closed product operations for participant-authorized, sponsor-paid Orbis transactions.
//!
//! The participant signs the inner active MetaTx wire. A distinct sponsor signs and pays for the
//! outer `MetaTx::dispatch` extrinsic. This module accepts typed calls and signers only; callers
//! cannot replace runtime bindings or inject a pre-encoded extension tuple.

use scale_value::{Value, ValueDef};
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Payload};

use crate::{
	client::signer::{OriginSigner, SubxtSignerAdapter},
	config::{build_orbis_params, OrbisConfig},
	product_sdk::{
		domains::{
			attestation::AttestationCommand, drive::DriveCommand,
			identity_personhood::IdentityPersonhoodCommand, names::NamesCommand, s3::S3Command,
			storage::StorageCommand, storage_provider::StorageProviderCommand, BlockNumber,
			Validate,
		},
		prepare_attestation_command, prepare_drive_command, prepare_identity_personhood_command,
		prepare_names_command, prepare_s3_command, prepare_storage_command,
		prepare_storage_provider_command, OrbisNativeClient,
	},
	tx::meta::{
		meta_tx_value_from_signed, prepare_sponsored_intent as prepare_wire, SponsoredIntent,
		SponsoredIntentBindings, MAX_META_ENCODED_BYTES,
	},
	types::error::OriginSdkError,
};

/// Closed native write target accepted by sponsored product operations.
///
/// Call bytes are always derived from one of the public domain command types against live
/// metadata. Applications cannot inject an arbitrary pallet/call name or pre-encoded SCALE.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SponsoredNativeTarget {
	IdentityPersonhood(IdentityPersonhoodCommand),
	Attestation(AttestationCommand),
	Names(NamesCommand),
	Storage(StorageCommand),
	StorageProvider(StorageProviderCommand),
	Drive(DriveCommand),
	S3(S3Command),
}

impl SponsoredNativeTarget {
	fn validate_at(&self, current_block: BlockNumber) -> Result<(), OriginSdkError> {
		let result = match self {
			Self::IdentityPersonhood(command) => command.validate(),
			Self::Attestation(command) => command.validate_at(current_block),
			Self::Names(command) => command.validate_at(current_block),
			Self::Storage(command) => command.validate(),
			Self::StorageProvider(command) => command.validate_at(current_block),
			Self::Drive(command) => command.validate(),
			Self::S3(command) => command.validate(),
		};
		result.map_err(|error| OriginSdkError::InvalidInput(error.to_string()))
	}

	fn into_payload(
		self,
		current_block: BlockNumber,
	) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		// Several domain encoders only construct metadata values. Validate the closed command
		// before any payload is prepared so sponsored dispatch cannot bypass domain invariants.
		self.validate_at(current_block)?;
		let payload = match self {
			Self::IdentityPersonhood(command) => prepare_identity_personhood_command(&command),
			Self::Attestation(command) => prepare_attestation_command(&command),
			Self::Names(command) => prepare_names_command(&command),
			Self::Storage(command) => prepare_storage_command(&command),
			Self::StorageProvider(command) => prepare_storage_provider_command(&command),
			Self::Drive(command) => prepare_drive_command(&command),
			Self::S3(command) => prepare_s3_command(&command),
		};
		payload.map_err(|error| OriginSdkError::InvalidInput(error.to_string()))
	}
}

/// Exact finite validity window accepted by the sponsored product operation.
///
/// The window must be representable without SCALE `Era` quantization changing either endpoint.
/// `valid_from` is also required to be the finalized block used during preparation, which gives
/// the SDK the exact block hash required by `CheckMortality`'s signed implicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredMortality {
	pub valid_from: BlockNumber,
	pub valid_until: BlockNumber,
}

impl SponsoredMortality {
	fn era(self) -> Result<sp_runtime::generic::Era, OriginSdkError> {
		let period = self.valid_until.checked_sub(self.valid_from).ok_or_else(|| {
			OriginSdkError::InvalidInput(
				"sponsored mortality valid_until must be after valid_from".into(),
			)
		})?;
		if period == 0 {
			return Err(OriginSdkError::InvalidInput(
				"sponsored mortality valid_until must be after valid_from".into(),
			));
		}
		if !(4..=65_536).contains(&period) || !period.is_power_of_two() {
			return Err(OriginSdkError::InvalidInput(
				"sponsored mortality period must be a power of two from 4 through 65536 blocks"
					.into(),
			));
		}
		let era = sp_runtime::generic::Era::mortal(period.into(), self.valid_from.into());
		if era.birth(self.valid_from.into()) != u64::from(self.valid_from)
			|| era.death(self.valid_from.into()) != u64::from(self.valid_until)
		{
			return Err(OriginSdkError::InvalidInput(
				"sponsored mortality window is not exactly representable as a mortal era".into(),
			));
		}
		Ok(era)
	}
}

/// Typed Rust preparation request matching the product SDK's explicit nonce, mortality, and
/// closed native target. The participant account is supplied by the participant signer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsoredIntentRequest {
	pub nonce: u32,
	pub mortality: SponsoredMortality,
	pub target: SponsoredNativeTarget,
}

/// Finalized evidence for a sponsored intent whose inner dispatch returned `Ok`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsoredIntentOutcome {
	pub block_hash: String,
	pub extrinsic_hash: String,
	pub participant: origin_primitives::AccountId,
	pub sponsor: origin_primitives::AccountId,
	pub intent_commitment: sp_core::H256,
}

/// Bind a dynamic product call to current Orbis state and sign it as `participant`.
///
/// `metadata_hash` is the RFC-78 hash for the connected runtime and is mandatory. The explicit
/// nonce and finite mortality window mirror the typed product request. Preparation is anchored to
/// the current finalized block so the mortal era's signed block-hash implicit is unambiguous.
pub async fn prepare_sponsored_intent(
	client: &OrbisNativeClient,
	request: SponsoredIntentRequest,
	participant: &OriginSigner,
	metadata_hash: [u8; 32],
) -> Result<SponsoredIntent, OriginSdkError> {
	let online = client.online();
	let SponsoredIntentRequest { nonce, mortality, target } = request;
	let finalized = online
		.blocks()
		.at_latest()
		.await
		.map_err(|error| OriginSdkError::Tx(error.to_string()))?;
	if finalized.number() != mortality.valid_from {
		return Err(OriginSdkError::InvalidInput(format!(
			"sponsored mortality valid_from {} is not the current finalized block {}",
			mortality.valid_from,
			finalized.number()
		)));
	}
	let era = mortality.era()?;
	let call = target.into_payload(mortality.valid_from)?;
	let call_bytes = call
		.encode_call_data(&online.metadata())
		.map_err(|error| OriginSdkError::Encode(error.to_string()))?;
	let live_nonce = finalized
		.account_nonce(&participant.account_id())
		.await
		.map_err(|error| OriginSdkError::Nonce(error.to_string()))?;
	let live_nonce = u32::try_from(live_nonce)
		.map_err(|_| OriginSdkError::InvalidInput("participant nonce exceeds u32".into()))?;
	if nonce != live_nonce {
		return Err(OriginSdkError::InvalidInput(format!(
			"sponsored intent nonce {nonce} does not match finalized participant nonce {live_nonce}"
		)));
	}
	let version = online.runtime_version();
	let bindings = SponsoredIntentBindings {
		nonce,
		era,
		spec_version: version.spec_version,
		transaction_version: version.transaction_version,
		genesis_hash: sp_core::H256(online.genesis_hash().0),
		mortality_hash: sp_core::H256(finalized.hash().0),
		metadata_hash: Some(metadata_hash),
	};
	prepare_wire(&online.metadata(), &call_bytes, participant, bindings).await
}

/// Submit a prepared intent with a distinct outer sponsor and require `MetaTx::Dispatched(Ok)`.
///
/// A successful outer extrinsic is insufficient: absence of the pallet event, a malformed event,
/// or an inner dispatch error all fail closed.
pub async fn submit_sponsored_intent(
	client: &OrbisNativeClient,
	intent: SponsoredIntent,
	sponsor: &OriginSigner,
) -> Result<SponsoredIntentOutcome, OriginSdkError> {
	if intent.participant() == &sponsor.account_id() {
		return Err(OriginSdkError::InvalidInput(
			"participant and sponsor must be distinct accounts".into(),
		));
	}

	let online = client.online();
	if intent.encoded_meta_tx().len() > MAX_META_ENCODED_BYTES {
		return Err(OriginSdkError::InvalidInput(format!(
			"sponsored MetaTx exceeds runtime limit of {MAX_META_ENCODED_BYTES} bytes"
		)));
	}
	let encoded_len = u32::try_from(intent.encoded_meta_tx().len())
		.map_err(|_| OriginSdkError::InvalidInput("sponsored MetaTx exceeds u32 length".into()))?;
	let dispatch = subxt::dynamic::tx(
		"MetaTx",
		"dispatch",
		vec![
			meta_tx_value_from_signed(&online.metadata(), intent.signed())?,
			Value::u128(encoded_len.into()),
		],
	);
	let sponsor_account = sponsor.account_id();
	let nonce = online
		.tx()
		.account_nonce(&sponsor_account)
		.await
		.map_err(|error| OriginSdkError::Nonce(error.to_string()))?;
	let params =
		build_orbis_params(DefaultExtrinsicParamsBuilder::<OrbisConfig>::new().nonce(nonce));
	let adapter = SubxtSignerAdapter::new(std::sync::Arc::new(sponsor.clone()));
	let progress = online
		.tx()
		.sign_and_submit_then_watch(&dispatch, &adapter, params)
		.await
		.map_err(|error| OriginSdkError::Tx(error.to_string()))?;
	let finalized = progress
		.wait_for_finalized()
		.await
		.map_err(|error| OriginSdkError::Tx(error.to_string()))?;
	let block_hash = finalized.block_hash();
	let events = finalized
		.wait_for_success()
		.await
		.map_err(|error| OriginSdkError::Tx(error.to_string()))?;
	ensure_meta_dispatched_ok(&events)?;

	Ok(SponsoredIntentOutcome {
		block_hash: format!("{block_hash:#x}"),
		extrinsic_hash: format!("{:#x}", events.extrinsic_hash()),
		participant: intent.participant().clone(),
		sponsor: sponsor_account,
		intent_commitment: intent.intent_commitment(),
	})
}

fn ensure_meta_dispatched_ok(
	events: &subxt::blocks::ExtrinsicEvents<OrbisConfig>,
) -> Result<(), OriginSdkError> {
	for event in events.iter() {
		let event = event.map_err(|error| OriginSdkError::Decode(error.to_string()))?;
		if event.pallet_name() != "MetaTx" || event.variant_name() != "Dispatched" {
			continue;
		}
		let fields = event
			.field_values()
			.map_err(|error| OriginSdkError::Decode(error.to_string()))?;
		let result = fields.values().next().ok_or_else(|| {
			OriginSdkError::MetaTx("MetaTx::Dispatched omitted its result".into())
		})?;
		return decode_dispatched_result(result);
	}
	Err(OriginSdkError::MetaTx("finalized extrinsic omitted MetaTx::Dispatched".into()))
}

fn decode_dispatched_result<T>(result: &Value<T>) -> Result<(), OriginSdkError> {
	match &result.value {
		ValueDef::Variant(variant) if variant.name == "Ok" => Ok(()),
		ValueDef::Variant(variant) if variant.name == "Err" => Err(OriginSdkError::MetaTx(
			"MetaTx::Dispatched reported an inner dispatch error".into(),
		)),
		_ => {
			Err(OriginSdkError::MetaTx("MetaTx::Dispatched result has an unexpected shape".into()))
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::product_sdk::domains::DriveId;
	use scale_value::Composite;

	fn dispatched(name: &str) -> Composite<()> {
		Composite::named([("result", Value::unnamed_variant(name, []))])
	}

	#[test]
	fn dispatch_result_shape_is_strict() {
		let ok = dispatched("Ok");
		assert!(decode_dispatched_result(ok.values().next().unwrap()).is_ok());
		let err = dispatched("Err");
		assert!(decode_dispatched_result(err.values().next().unwrap()).is_err());
		assert!(decode_dispatched_result(&Value::u128(0)).is_err());
	}

	#[test]
	fn mortality_window_must_survive_era_quantization_exactly() {
		let mortality = SponsoredMortality { valid_from: 100, valid_until: 164 };
		let era = mortality.era().expect("64-block mortal era");
		assert!(!era.is_immortal());
		assert_eq!(era.birth(100), 100);
		assert_eq!(era.death(100), 164);
		assert!(SponsoredMortality { valid_from: 100, valid_until: 100 }.era().is_err());
		assert!(SponsoredMortality { valid_from: 100, valid_until: 165 }.era().is_err());
	}

	#[test]
	fn sponsored_target_validation_precedes_payload_preparation() {
		let drive = DriveId::new(format!("0x{}", "11".repeat(32))).expect("drive id");
		let target = SponsoredNativeTarget::Drive(DriveCommand::UpdateRoot {
			drive,
			expected_version: 0,
			root_storage_ref: None,
		});
		assert!(target.into_payload(1).is_err());
	}
}
