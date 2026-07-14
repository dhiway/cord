use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{
	contract::{NativeError, NativeErrorCode},
	domains::{
		attestation::{AttestationCommand, AttestationQuery},
		common::{AccountId, Hash32},
		dotns::{DotnsCommand, DotnsQuery},
		drive::{DriveCommand, DriveQuery},
		identity_personhood::{IdentityPersonhoodCommand, IdentityPersonhoodQuery},
		s3::{S3Command, S3Query},
		storage::{StorageCommand, StorageQuery},
		storage_provider::{StorageProviderCommand, StorageProviderQuery},
		Validate,
	},
	transport::{
		prepare_attestation_command, prepare_dotns_command, prepare_drive_command,
		prepare_identity_personhood_command, prepare_s3_command, prepare_storage_command,
		prepare_storage_provider_command,
	},
};

#[derive(Debug)]
pub enum NativeRouteBinding {
	AttestationQuery(AttestationQuery),
	AttestationCommand(AttestationCommand),
	DotnsQuery(DotnsQuery),
	DotnsCommand(DotnsCommand),
	StorageQuery(StorageQuery),
	StorageCommand(StorageCommand),
	StorageProviderQuery(StorageProviderQuery),
	StorageProviderCommand(StorageProviderCommand),
	DriveQuery(DriveQuery),
	DriveCommand(DriveCommand),
	S3Query(S3Query),
	S3Command(S3Command),
	IdentityPersonhoodQuery(IdentityPersonhoodQuery),
	IdentityPersonhoodCommand(IdentityPersonhoodCommand),
	PrepareSponsoredIntent(PrepareSponsoredIntentBinding),
	SubmitSponsoredIntent(SubmitSponsoredIntentBinding),
}

#[derive(Debug)]
pub struct PrepareSponsoredIntentBinding {
	participant: AccountId,
	nonce: CanonicalU32,
	mortality: SponsoredMortality,
	target: Box<NativeRouteBinding>,
}

#[derive(Debug)]
pub struct SubmitSponsoredIntentBinding {
	envelope: SponsoredIntentEnvelope,
	participant_signature: ParticipantSignature,
}

#[derive(Debug)]
struct SponsoredIntentEnvelope {
	version: u8,
	signing_domain: String,
	genesis_hash: Hash32,
	spec_version: u32,
	transaction_version: u32,
	metadata_hash: Hash32,
	participant: AccountId,
	nonce: CanonicalU32,
	mortality: SponsoredMortality,
	target: Box<NativeRouteBinding>,
	signing_payload_hash: Hash32,
	intent_id: Hash32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalU32(String);

impl Validate for CanonicalU32 {
	fn validate(&self) -> Result<(), NativeError> {
		let parsed = self.0.parse::<u32>().map_err(|_| invalid("expected a decimal u32 string"))?;
		if parsed.to_string() != self.0 {
			return Err(invalid("expected a canonical decimal u32 string"));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SponsoredMortality {
	valid_from: CanonicalU32,
	valid_until: CanonicalU32,
}

impl SponsoredMortality {
	fn validate(&self) -> Result<(), NativeError> {
		self.valid_from.validate()?;
		self.valid_until.validate()?;
		let from = self.valid_from.0.parse::<u32>().expect("validated decimal u32");
		let until = self.valid_until.0.parse::<u32>().expect("validated decimal u32");
		let period = until
			.checked_sub(from)
			.ok_or_else(|| invalid("sponsored mortality must end after it starts"))?;
		if !(4..=65_536).contains(&period) || !period.is_power_of_two() {
			return Err(invalid(
				"sponsored mortality period must be a power of two from 4 through 65536 blocks",
			));
		}
		let quantize_factor = if period > 4_096 { period >> 12 } else { 1 };
		if from % quantize_factor != 0 {
			return Err(invalid(
				"sponsored mortality valid_from is not exactly representable by FRAME Era",
			));
		}
		Ok(())
	}
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ParticipantSignatureScheme {
	Sr25519,
	Ed25519,
	Ecdsa,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParticipantSignature {
	scheme: ParticipantSignatureScheme,
	value: String,
}

impl ParticipantSignature {
	fn validate(&self) -> Result<(), NativeError> {
		let bytes = self.value.as_bytes();
		let expected = match self.scheme {
			ParticipantSignatureScheme::Sr25519 | ParticipantSignatureScheme::Ed25519 => 64,
			ParticipantSignatureScheme::Ecdsa => 65,
		};
		if bytes.len() != 2 + expected * 2 ||
			!self.value.starts_with("0x") ||
			!bytes[2..]
				.iter()
				.all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
		{
			return Err(invalid(
				"participant signature must be lowercase 0x-prefixed hex with scheme-exact length",
			));
		}
		Ok(())
	}
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SponsoredTargetWire {
	capability: SponsorableCapability,
	method: String,
	payload: Value,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SponsorableCapability {
	Identity,
	Attestation,
	Dotns,
	Storage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareSponsoredIntentWire {
	participant: AccountId,
	nonce: CanonicalU32,
	mortality: SponsoredMortality,
	target: SponsoredTargetWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedSponsoredIntentWire {
	envelope: SponsoredIntentEnvelopeWire,
	participant_signature: ParticipantSignature,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitSponsoredIntentWire {
	signed_intent: SignedSponsoredIntentWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SponsoredIntentEnvelopeWire {
	version: u8,
	signing_domain: String,
	genesis_hash: Hash32,
	spec_version: u32,
	transaction_version: u32,
	metadata_hash: Hash32,
	participant: AccountId,
	nonce: CanonicalU32,
	mortality: SponsoredMortality,
	target: SponsoredTargetWire,
	signing_payload_hash: Hash32,
	intent_id: Hash32,
}

impl NativeRouteBinding {
	pub fn validate_and_prepare(&self) -> Result<(), NativeError> {
		match self {
			Self::AttestationQuery(value) => value.validate(),
			Self::AttestationCommand(value) => prepare_attestation_command(value).map(drop),
			Self::DotnsQuery(value) => value.validate(),
			Self::DotnsCommand(value) => prepare_dotns_command(value).map(drop),
			Self::StorageQuery(value) => value.validate(),
			Self::StorageCommand(value) => prepare_storage_command(value).map(drop),
			Self::StorageProviderQuery(value) => value.validate(),
			Self::StorageProviderCommand(value) =>
				prepare_storage_provider_command(value).map(drop),
			Self::DriveQuery(value) => value.validate(),
			Self::DriveCommand(value) => prepare_drive_command(value).map(drop),
			Self::S3Query(value) => value.validate(),
			Self::S3Command(value) => prepare_s3_command(value).map(drop),
			Self::IdentityPersonhoodQuery(value) => value.validate(),
			Self::IdentityPersonhoodCommand(value) =>
				prepare_identity_personhood_command(value).map(drop),
			Self::PrepareSponsoredIntent(value) => {
				value.participant.validate()?;
				value.nonce.validate()?;
				value.mortality.validate()?;
				value.target.validate_and_prepare()
			},
			Self::SubmitSponsoredIntent(value) => {
				let envelope = &value.envelope;
				if envelope.version != 1 || envelope.signing_domain != "orbis/meta-intent/v7" {
					return Err(invalid(
						"unsupported sponsored intent wire version or signing domain",
					));
				}
				envelope.genesis_hash.validate()?;
				envelope.metadata_hash.validate()?;
				envelope.signing_payload_hash.validate()?;
				envelope.intent_id.validate()?;
				envelope.participant.validate()?;
				envelope.nonce.validate()?;
				envelope.mortality.validate()?;
				if envelope.spec_version == 0 || envelope.transaction_version == 0 {
					return Err(invalid("sponsored intent runtime versions must be non-zero"));
				}
				envelope.target.validate_and_prepare()?;
				value.participant_signature.validate()
			},
		}
	}
}

fn invalid(message: impl Into<String>) -> NativeError {
	NativeError::new(NativeErrorCode::InvalidInput, message)
}

fn snake(value: &str) -> String {
	let mut output = String::new();
	for (index, character) in value.chars().enumerate() {
		if character.is_uppercase() && index > 0 {
			output.push('_');
		}
		output.extend(character.to_lowercase());
	}
	output
}

fn pascal(value: &str) -> String {
	value
		.split('_')
		.map(|word| {
			let mut characters = word.chars();
			characters
				.next()
				.map(|first| first.to_uppercase().chain(characters).collect::<String>())
				.unwrap_or_default()
		})
		.collect()
}

fn sponsored_target_declaration(
	capability: SponsorableCapability,
	method: &str,
) -> Result<(&'static str, String), NativeError> {
	let declaration = match capability {
		SponsorableCapability::Identity => {
			if ![
				"set_identity",
				"clear_identity",
				"request_judgement",
				"cancel_judgement_request",
				"provide_judgement",
				"attest_lite_person",
			]
			.contains(&method)
			{
				return Err(invalid("unsupported sponsored identity target"));
			}
			"IdentityPersonhoodCommand"
		},
		SponsorableCapability::Attestation => {
			if ![
				"create_schema",
				"set_schema_status",
				"issue",
				"issue_delegated",
				"issue_batch",
				"revoke",
				"set_emergency_pause",
				"force_schema_status",
				"force_revoke",
				"revoke_delegated",
				"issue_delegated_batch",
				"revoke_batch",
				"revoke_delegated_batch",
				"revoke_external_status",
				"revoke_external_status_batch",
			]
			.contains(&method)
			{
				return Err(invalid("unsupported sponsored attestation target"));
			}
			"AttestationCommand"
		},
		SponsorableCapability::Dotns => {
			if ![
				"commit",
				"cancel_commitment",
				"prune_expired_commitment",
				"register",
				"renew",
				"transfer",
				"add_controller",
				"remove_controller",
				"set_address",
				"set_subject",
				"set_attestation",
				"set_content",
				"set_text",
				"set_primary_name",
				"release",
				"remove_expired_name",
				"reserve_name",
				"clear_reservation",
				"set_label_protection",
				"set_paused",
				"force_transfer",
				"force_revoke",
				"set_registrar",
			]
			.contains(&method)
			{
				return Err(invalid("unsupported sponsored DotNS target"));
			}
			"DotnsCommand"
		},
		SponsorableCapability::Storage => {
			if [
				"store",
				"store_with_cid_config",
				"store_reserved",
				"renew_reserved",
				"attach_provider",
				"renew",
				"force_renew",
				"enable_auto_renew",
				"disable_auto_renew",
			]
			.contains(&method)
			{
				"StorageCommand"
			} else if [
				"register_provider",
				"update_provider",
				"set_provider_status",
				"remove_provider",
				"heartbeat",
				"propose_agreement",
				"accept_agreement",
				"cancel_agreement",
				"accept_renewal",
				"expire_agreement",
				"prune_agreement",
				"issue_challenge",
				"submit_checkpoint",
				"timeout_challenge",
				"request_renewal",
				"acknowledge_deletion",
				"commit_provider_root",
			]
			.contains(&method)
			{
				"StorageProviderCommand"
			} else if [
				"create_drive",
				"update_root",
				"drive.set_controller",
				"transfer_drive",
				"archive_drive",
			]
			.contains(&method)
			{
				"DriveCommand"
			} else if [
				"create_bucket",
				"s3.set_controller",
				"transfer_bucket",
				"set_archived",
				"set_versioning",
				"put_object",
				"delete_object",
				"delete_bucket",
			]
			.contains(&method)
			{
				"S3Command"
			} else {
				return Err(invalid("unsupported sponsored storage target"));
			}
		},
	};
	let variant = match method {
		"cancel_judgement_request" => "CancelJudgement".into(),
		"create_drive" => "Create".into(),
		"drive.set_controller" | "s3.set_controller" => "SetController".into(),
		"transfer_drive" => "Transfer".into(),
		"archive_drive" => "Archive".into(),
		_ => pascal(method),
	};
	Ok((declaration, variant))
}

fn sponsored_target_binding(
	target: SponsoredTargetWire,
) -> Result<NativeRouteBinding, NativeError> {
	if !target.payload.is_object() {
		return Err(invalid("sponsored target payload must be an object"));
	}
	let (declaration, variant) = sponsored_target_declaration(target.capability, &target.method)?;
	let route = json!({
		"method": target.method,
		"sample_payload": target.payload,
		"rust": { "declaration": declaration, "variant": variant },
	});
	instantiate_native_route(&route)
}

fn bytes(value: &mut Value) {
	if let Some(text) = value.as_str() {
		*value = Value::Array(text.as_bytes().iter().map(|byte| json!(byte)).collect());
	}
}

fn normalize_numbers(value: &mut Value) {
	match value {
		Value::Array(values) => values.iter_mut().for_each(normalize_numbers),
		Value::Object(fields) =>
			for (name, value) in fields {
				if [
					"spec_version",
					"nonce",
					"deadline",
					"expiry",
					"expires_at",
					"capacity_bytes",
					"additional_bytes",
					"bytes",
					"due_at",
					"sequence",
					"root_sequence",
					"leaf_index",
					"leaf_count",
					"additional_period",
					"additional_blocks",
					"block",
					"expected_version",
					"expected_bucket_version",
					"expected_object_version",
					"data_len",
				]
				.contains(&name.as_str())
				{
					if let Some(number) = value.as_str().and_then(|text| text.parse::<u64>().ok()) {
						*value = json!(number);
					}
				}
				normalize_numbers(value);
		},
		_ => {},
	}
}

fn normalize_accounts(value: &mut Value) {
	const ALICE: &str = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
	match value {
		Value::Array(values) => values.iter_mut().for_each(normalize_accounts),
		Value::Object(fields) =>
			for (name, value) in fields {
				if [
					"account",
					"creator",
					"issuer",
					"owner",
					"provider",
					"controller",
					"new_owner",
					"registrar",
					"beneficiary",
					"delegate",
					"revoker",
					"target",
					"candidate",
				]
				.contains(&name.as_str())
				{
					if value.is_string() {
						*value = json!(ALICE);
					}
				}
				if name == "authorized_issuers" {
					if let Some(values) = value.as_array_mut() {
						values.iter_mut().for_each(|value| *value = json!(ALICE));
					}
				}
				normalize_accounts(value);
		},
		_ => {},
	}
}

fn rust_arguments(route: &Value) -> Result<Map<String, Value>, NativeError> {
	let mut fields = route["sample_payload"]
		.as_object()
		.cloned()
		.ok_or_else(|| invalid("route sample payload is not an object"))?;
	let mut normalized = Value::Object(fields);
	normalize_numbers(&mut normalized);
	normalize_accounts(&mut normalized);
	fields = normalized.as_object().cloned().unwrap_or_default();
	if fields.contains_key("cursor") || fields.contains_key("limit") {
		let cursor = fields.remove("cursor").unwrap_or(Value::Null);
		let limit = fields.remove("limit").unwrap_or(json!(1));
		fields.insert("page".into(), json!({ "cursor": cursor, "limit": limit }));
	}
	for (from, to) in [
		("agreement_id", "agreement"),
		("challenge_id", "challenge"),
		("container_ref", "container"),
		("drive_id", "drive"),
		("additional_period", "additional_blocks"),
	] {
		if let Some(value) = fields.remove(from) {
			fields.insert(to.into(), value);
		}
	}
	let declaration = route["rust"]["declaration"].as_str().unwrap_or_default();
	let method = route["method"].as_str().unwrap_or_default();
	if declaration == "AttestationCommand" && method == "issue_batch" {
		if let Some(items) = fields.remove("items") {
			fields.insert("inputs".into(), items);
		}
	}
	if declaration == "S3Command" && method == "put_object" {
		if let Some(content) = fields.remove("content_hash") {
			fields.insert("content".into(), content);
		}
	}
	fn signatures(value: &mut Value) {
		match value {
			Value::Array(values) => values.iter_mut().for_each(signatures),
			Value::Object(fields) => {
				if let Some(signature) = fields.get_mut("signature") {
					if signature.is_string() {
						*signature = json!({ "scheme": "sr25519", "bytes": signature.clone() });
					}
				}
				fields.values_mut().for_each(signatures);
			},
			_ => {},
		}
	}
	let mut signature_value = Value::Object(fields);
	signatures(&mut signature_value);
	fields = signature_value.as_object().cloned().unwrap_or_default();
	if declaration == "AttestationCommand" && method == "issue" {
		return Ok(Map::from_iter([("input".into(), Value::Object(fields))]));
	}
	for field in ["definition", "label", "salt", "key", "value", "endpoint", "service_key", "name"]
	{
		if (declaration == "AttestationCommand" && field == "definition") ||
			declaration.starts_with("Dotns") &&
				((method == "register" && field == "salt") ||
					(["resolve_text", "set_text"].contains(&method) && field == "key") ||
					(method == "set_text" && field == "value") ||
					(method == "set_address" && field == "address")) ||
			declaration.starts_with("Drive") && method == "create_drive" && field == "name" ||
			declaration.starts_with("StorageProvider") &&
				["endpoint", "service_key"].contains(&field) ||
			declaration.starts_with("S3") && field == "key"
		{
			if let Some(value) = fields.get_mut(field) {
				bytes(value);
			}
		}
	}
	Ok(fields)
}

pub fn instantiate_native_route(route: &Value) -> Result<NativeRouteBinding, NativeError> {
	let declaration = route["rust"]["declaration"]
		.as_str()
		.ok_or_else(|| invalid("route Rust declaration is missing"))?;
	let variant = route["rust"]["variant"]
		.as_str()
		.ok_or_else(|| invalid("route Rust variant is missing"))?;
	if route["rust"]["binding_kind"].as_str() == Some("sdk-function") {
		if variant != "async-function" {
			return Err(invalid("unsupported Rust SDK function variant"));
		}
		return match declaration {
			"prepare_sponsored_intent" => {
				let wire: PrepareSponsoredIntentWire =
					serde_json::from_value(route["sample_payload"].clone()).map_err(|error| {
						invalid(format!("prepare_sponsored_intent canonical arguments: {error}"))
					})?;
				Ok(NativeRouteBinding::PrepareSponsoredIntent(PrepareSponsoredIntentBinding {
					participant: wire.participant,
					nonce: wire.nonce,
					mortality: wire.mortality,
					target: Box::new(sponsored_target_binding(wire.target)?),
				}))
			},
			"submit_sponsored_intent" => {
				let wire: SubmitSponsoredIntentWire =
					serde_json::from_value(route["sample_payload"].clone()).map_err(|error| {
						invalid(format!("submit_sponsored_intent canonical arguments: {error}"))
					})?;
				let envelope = wire.signed_intent.envelope;
				Ok(NativeRouteBinding::SubmitSponsoredIntent(SubmitSponsoredIntentBinding {
					envelope: SponsoredIntentEnvelope {
						version: envelope.version,
						signing_domain: envelope.signing_domain,
						genesis_hash: envelope.genesis_hash,
						spec_version: envelope.spec_version,
						transaction_version: envelope.transaction_version,
						metadata_hash: envelope.metadata_hash,
						participant: envelope.participant,
						nonce: envelope.nonce,
						mortality: envelope.mortality,
						target: Box::new(sponsored_target_binding(envelope.target)?),
						signing_payload_hash: envelope.signing_payload_hash,
						intent_id: envelope.intent_id,
					},
					participant_signature: wire.signed_intent.participant_signature,
				}))
			},
			_ => Err(invalid("unsupported Rust SDK function route")),
		};
	}
	let fields = rust_arguments(route)?;
	let tag = if declaration.ends_with("Query") { "query" } else { "command" };
	let mut encoded = Map::new();
	encoded.insert(tag.into(), Value::String(snake(variant)));
	if !fields.is_empty() {
		encoded.insert("arguments".into(), Value::Object(fields));
	}
	let value = Value::Object(encoded);
	macro_rules! decode {
		($kind:ident, $type:ty) => {
			serde_json::from_value::<$type>(value)
				.map(NativeRouteBinding::$kind)
				.map_err(|error| {
					invalid(format!("{declaration}::{variant} canonical arguments: {error}"))
				})
		};
	}
	match declaration {
		"AttestationQuery" => decode!(AttestationQuery, AttestationQuery),
		"AttestationCommand" => decode!(AttestationCommand, AttestationCommand),
		"DotnsQuery" => decode!(DotnsQuery, DotnsQuery),
		"DotnsCommand" => decode!(DotnsCommand, DotnsCommand),
		"StorageQuery" => decode!(StorageQuery, StorageQuery),
		"StorageCommand" => decode!(StorageCommand, StorageCommand),
		"StorageProviderQuery" => decode!(StorageProviderQuery, StorageProviderQuery),
		"StorageProviderCommand" => decode!(StorageProviderCommand, StorageProviderCommand),
		"DriveQuery" => decode!(DriveQuery, DriveQuery),
		"DriveCommand" => decode!(DriveCommand, DriveCommand),
		"S3Query" => decode!(S3Query, S3Query),
		"S3Command" => decode!(S3Command, S3Command),
		"IdentityPersonhoodQuery" => {
			decode!(IdentityPersonhoodQuery, IdentityPersonhoodQuery)
		},
		"IdentityPersonhoodCommand" => {
			decode!(IdentityPersonhoodCommand, IdentityPersonhoodCommand)
		},
		_ => Err(invalid("unsupported Rust route declaration")),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sponsored_wire_fields_fail_closed() {
		let valid_sr25519 = ParticipantSignature {
			scheme: ParticipantSignatureScheme::Sr25519,
			value: format!("0x{}", "11".repeat(64)),
		};
		assert!(valid_sr25519.validate().is_ok());
		assert!(ParticipantSignature {
			scheme: ParticipantSignatureScheme::Sr25519,
			value: "0x11".into(),
		}
		.validate()
		.is_err());
		assert!(ParticipantSignature {
			scheme: ParticipantSignatureScheme::Ecdsa,
			value: format!("0x{}", "AB".repeat(65)),
		}
		.validate()
		.is_err());

		let valid_mortality = SponsoredMortality {
			valid_from: CanonicalU32("1".into()),
			valid_until: CanonicalU32("65".into()),
		};
		assert!(valid_mortality.validate().is_ok());
		assert!(SponsoredMortality {
			valid_from: CanonicalU32("1".into()),
			valid_until: CanonicalU32("66".into()),
		}
		.validate()
		.is_err());
		assert!(CanonicalU32("4294967296".into()).validate().is_err());
	}
}
