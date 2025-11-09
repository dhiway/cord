use crate::{
	tx::Transactions,
	types::{self, entity::AttributeEntry},
};
use scale_value::Value;
use subxt::tx::DynamicPayload;

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

impl<'a> Transactions<'a> {
	pub async fn entity_set_info_json(
		&self,
		info_packet_json: serde_json::Value,
	) -> crate::error::Result<DynamicPayload> {
		self.build_json("Entity", "set_info", info_packet_json).await
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
		let args = Value::named_composite([("sub", bytes_value(account.as_ref()))]);
		self.build("Entity", "set_sub_account", args).await
	}

	pub async fn entity_revoke_sub_account(
		&self,
		sub_account_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let account = types::ss58_to_account32(sub_account_ss58)?;
		let args = Value::named_composite([("sub", bytes_value(account.as_ref()))]);
		self.build("Entity", "revoke_sub_account", args).await
	}

	pub async fn entity_rotate_controller(
		&self,
		token_ss58: &str,
		new_controller_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let token_bytes = token_ss58.as_bytes();
		let controller = types::ss58_to_account32(new_controller_ss58)?;
		let args = Value::named_composite([
			("token", bytes_value(token_bytes)),
			("new", bytes_value(controller.as_ref())),
		]);
		self.build("Entity", "rotate_controller", args).await
	}

	pub async fn entity_clear_everything(
		&self,
		token_ss58: &str,
	) -> crate::error::Result<DynamicPayload> {
		let token_bytes = token_ss58.as_bytes();
		let args = Value::named_composite([("token", bytes_value(token_bytes))]);
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
		let token_bytes = token_ss58.as_bytes();
		let args = Value::named_composite([("token", bytes_value(token_bytes))]);
		self.build("Entity", "remove_id_name", args).await
	}
}
