use crate::{
	error::Error,
	tx::Transactions,
	types::{self, entity::AttributeEntry, ElementJson},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use std::collections::BTreeMap;
use subxt::{dynamic::Value, tx::DynamicPayload};

fn bytes_value(bytes: &[u8]) -> Value {
	Value::unnamed_composite(bytes.iter().copied().map(|b| Value::u128(b as u128)))
}

fn attribute_vec(entries: Vec<AttributeEntry>) -> crate::error::Result<Value> {
	let mut collected = Vec::new();
	for entry in entries.into_iter() {
		collected.push(entry.to_dynamic_pair()?);
	}
	Ok(Value::unnamed_composite(collected))
}

#[derive(Deserialize)]
struct EntityProfileJson {
	display: Option<String>,
	legal: Option<String>,
	web: Option<String>,
	email: Option<String>,
	twitter: Option<String>,
	#[serde(default)]
	attributes: BTreeMap<String, String>,
}

impl EntityProfileJson {
	fn into_value(self) -> crate::error::Result<Value> {
		let display = element_from_string(self.display);
		let legal = element_from_string(self.legal);
		let web = element_from_string(self.web);
		let email = element_from_string(self.email);
		let twitter = element_from_string(self.twitter);
		let attrs_value = option_attributes_value(self.attributes)?;
		Ok(Value::named_composite([
			("display", types::element_json_to_dynamic(&display)?),
			("legal", types::element_json_to_dynamic(&legal)?),
			("web", types::element_json_to_dynamic(&web)?),
			("email", types::element_json_to_dynamic(&email)?),
			("twitter", types::element_json_to_dynamic(&twitter)?),
			("attributes", attrs_value),
		]))
	}
}

fn element_from_string(input: Option<String>) -> ElementJson {
	input
		.map(|val| ElementJson::RawBase64(BASE64.encode(val)))
		.unwrap_or(ElementJson::None)
}

fn option_attributes_value(map: BTreeMap<String, String>) -> crate::error::Result<Value> {
	if map.is_empty() {
		return Ok(Value::variant("None", scale_value::Composite::unnamed(Vec::new())));
	}
	let entries = map
		.into_iter()
		.map(|(key, value)| AttributeEntry {
			key_hex: types::to_key_hex_from_utf8(&key),
			key_utf8: Some(key),
			value: ElementJson::RawBase64(BASE64.encode(value)),
		})
		.collect();
	let inner = attribute_vec(entries)?;
	Ok(Value::variant("Some", scale_value::Composite::unnamed(vec![inner])))
}

impl<'a> Transactions<'a> {
	pub async fn entity_set_info_json(
		&self,
		info_packet_json: serde_json::Value,
	) -> crate::error::Result<DynamicPayload> {
		let profile: EntityProfileJson = serde_json::from_value(info_packet_json)
			.map_err(|e| Error::Params(format!("invalid entity profile json: {e}")))?;
		let info_value = profile.into_value()?;
		let args = Value::named_composite([("info", info_value)]);
		self.build("Entity", "set_info", args).await
	}

	pub async fn entity_update_info(
		&self,
		entries: Vec<AttributeEntry>,
	) -> crate::error::Result<DynamicPayload> {
		let ops = attribute_vec(entries)?;
		let args = Value::named_composite([("ops", ops)]);
		self.build("Entity", "update_info", args).await
	}

	pub async fn entity_add_attributes(
		&self,
		entries: Vec<AttributeEntry>,
	) -> crate::error::Result<DynamicPayload> {
		let ops = attribute_vec(entries)?;
		let args = Value::named_composite([("ops", ops)]);
		self.build("Entity", "add_attributes", args).await
	}

	pub async fn entity_remove_attribute_hex(
		&self,
		key_hex: &str,
	) -> crate::error::Result<DynamicPayload> {
		let key = types::hex_to_bytes(key_hex)?;
		let args = Value::named_composite([("key", bytes_value(&key))]);
		self.build("Entity", "remove_attribute", args).await
	}

	pub async fn entity_remove_attribute_utf8(
		&self,
		key_utf8: &str,
	) -> crate::error::Result<DynamicPayload> {
		let key_hex = types::to_key_hex_from_utf8(key_utf8);
		self.entity_remove_attribute_hex(&key_hex).await
	}

	pub async fn entity_rotate_attribute(
		&self,
		entry: AttributeEntry,
	) -> crate::error::Result<DynamicPayload> {
		let key_bytes = entry.key_bytes()?;
		let element = types::element_json_to_dynamic(&entry.value)?;
		let args = Value::named_composite([("key", bytes_value(&key_bytes)), ("val", element)]);
		self.build("Entity", "rotate_attribute", args).await
	}

	pub async fn entity_set_sub_account(
		&self,
		sub_account_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let account = types::ss58_to_account32(sub_account_ss58)?;
		let args = Value::named_composite([("sub", types::account_id_value(&account))]);
		self.build("Entity", "set_sub_account", args).await
	}

	pub async fn entity_revoke_sub_account(
		&self,
		sub_account_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let account = types::ss58_to_account32(sub_account_ss58)?;
		let args = Value::named_composite([("sub", types::account_id_value(&account))]);
		self.build("Entity", "revoke_sub_account", args).await
	}

	pub async fn entity_rotate_controller(
		&self,
		token_ss58: &str,
		new_controller_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let token = types::identifier_value(token_ss58)?;
		let controller = types::ss58_to_account32(new_controller_ss58)?;
		let args = Value::named_composite([
			("token", token),
			("new", types::account_id_value(&controller)),
		]);
		self.build("Entity", "rotate_controller", args).await
	}

	pub async fn entity_clear_everything(
		&self,
		token_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let token = types::identifier_value(token_ss58)?;
		let args = Value::named_composite([("token", token)]);
		self.build("Entity", "clear", args).await
	}

	pub async fn entity_set_id_name_prefix(
		&self,
		prefix: &str,
	) -> crate::error::Result<DynamicPayload> {
		let args = Value::named_composite([("prefix", bytes_value(prefix.as_bytes()))]);
		self.build("Entity", "set_id_name_prefix", args).await
	}

	pub async fn entity_remove_id_name(
		&self,
		token_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let token = types::identifier_value(token_ss58)?;
		let args = Value::named_composite([("token", token)]);
		self.build("Entity", "remove_id_name", args).await
	}
}
