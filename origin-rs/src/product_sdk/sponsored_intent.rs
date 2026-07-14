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
			attestation::AttestationCommand, dotns::DotnsCommand, drive::DriveCommand,
			identity_personhood::IdentityPersonhoodCommand, s3::S3Command, storage::StorageCommand,
			storage_provider::StorageProviderCommand,
		},
		prepare_attestation_command, prepare_dotns_command, prepare_drive_command,
		prepare_identity_personhood_command, prepare_s3_command, prepare_storage_command,
		prepare_storage_provider_command, OrbisNativeClient,
	},
	tx::meta::{
		meta_tx_value_from_signed, prepare_sponsored_intent as prepare_wire, SponsoredIntent,
		SponsoredIntentBindings,
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
	Dotns(DotnsCommand),
	Storage(StorageCommand),
	StorageProvider(StorageProviderCommand),
	Drive(DriveCommand),
	S3(S3Command),
}

impl SponsoredNativeTarget {
	fn into_payload(self) -> Result<subxt::tx::DynamicPayload, OriginSdkError> {
		let payload = match self {
			Self::IdentityPersonhood(command) => prepare_identity_personhood_command(&command),
			Self::Attestation(command) => prepare_attestation_command(&command),
			Self::Dotns(command) => prepare_dotns_command(&command),
			Self::Storage(command) => prepare_storage_command(&command),
			Self::StorageProvider(command) => prepare_storage_provider_command(&command),
			Self::Drive(command) => prepare_drive_command(&command),
			Self::S3(command) => prepare_s3_command(&command),
		};
		payload.map_err(|error| OriginSdkError::InvalidInput(error.to_string()))
	}
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
/// `metadata_hash` is the RFC-78 hash for the connected runtime and is mandatory. All other
/// bindings are captured from the connected client. The operation uses an immortal era, whose
/// mortality implicit is the genesis hash.
pub async fn prepare_sponsored_intent(
	client: &OrbisNativeClient,
	target: SponsoredNativeTarget,
	participant: &OriginSigner,
	metadata_hash: [u8; 32],
) -> Result<SponsoredIntent, OriginSdkError> {
	let online = client.online();
	let call = target.into_payload()?;
	let call_bytes = call
		.encode_call_data(&online.metadata())
		.map_err(|error| OriginSdkError::Encode(error.to_string()))?;
	let nonce = online
		.tx()
		.account_nonce(&participant.account_id())
		.await
		.map_err(|error| OriginSdkError::Nonce(error.to_string()))?;
	let nonce = u32::try_from(nonce)
		.map_err(|_| OriginSdkError::InvalidInput("participant nonce exceeds u32".into()))?;
	let version = online.runtime_version();
	let bindings = SponsoredIntentBindings {
		nonce,
		era: sp_runtime::generic::Era::Immortal,
		spec_version: version.spec_version,
		transaction_version: version.transaction_version,
		genesis_hash: sp_core::H256(online.genesis_hash().0),
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
		_ =>
			Err(OriginSdkError::MetaTx("MetaTx::Dispatched result has an unexpected shape".into())),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
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
}
