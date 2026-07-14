use serde_json::{json, Map, Value};

use super::{
	contract::{NativeError, NativeErrorCode},
	domains::{
		attestation::{AttestationCommand, AttestationQuery},
		dotns::{DotnsCommand, DotnsQuery},
		drive::{DriveCommand, DriveQuery},
		s3::{S3Command, S3Query},
		storage::{StorageCommand, StorageQuery},
		storage_provider::{StorageProviderCommand, StorageProviderQuery},
		Validate,
	},
	transport::{
		prepare_attestation_command, prepare_dotns_command, prepare_drive_command,
		prepare_s3_command, prepare_storage_command, prepare_storage_provider_command,
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
			Self::StorageProviderCommand(value) => {
				prepare_storage_provider_command(value).map(drop)
			},
			Self::DriveQuery(value) => value.validate(),
			Self::DriveCommand(value) => prepare_drive_command(value).map(drop),
			Self::S3Query(value) => value.validate(),
			Self::S3Command(value) => prepare_s3_command(value).map(drop),
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

fn bytes(value: &mut Value) {
	if let Some(text) = value.as_str() {
		*value = Value::Array(text.as_bytes().iter().map(|byte| json!(byte)).collect());
	}
}

fn normalize_numbers(value: &mut Value) {
	match value {
		Value::Array(values) => values.iter_mut().for_each(normalize_numbers),
		Value::Object(fields) => {
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
			}
		},
		_ => {},
	}
}

fn normalize_accounts(value: &mut Value) {
	const ALICE: &str = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
	match value {
		Value::Array(values) => values.iter_mut().for_each(normalize_accounts),
		Value::Object(fields) => {
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
			}
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
		if (declaration == "AttestationCommand" && field == "definition")
			|| declaration.starts_with("Dotns")
				&& ((method == "register" && field == "salt")
					|| (["resolve_text", "set_text"].contains(&method) && field == "key")
					|| (method == "set_text" && field == "value")
					|| (method == "set_address" && field == "address"))
			|| declaration.starts_with("Drive") && method == "create_drive" && field == "name"
			|| declaration.starts_with("StorageProvider")
				&& ["endpoint", "service_key"].contains(&field)
			|| declaration.starts_with("S3") && field == "key"
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
		_ => Err(invalid("unsupported Rust route declaration")),
	}
}
