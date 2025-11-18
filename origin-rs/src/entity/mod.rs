use crate::{
	client::Client,
	dyn_helpers::DynHelpers,
	error::Error,
	error::{Error as SdkError, Result},
	params::config::OriginConfig,
	tx::submitter::{SubmitError, SubmitStage, TxSubmitter},
	types::{
		self,
		element::element_text_from_view,
		entity::{AttributeEntry, EntityInfoRecord, HistoryEntry},
		ElementJson,
	},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bs58;
use hex;
use log::debug;
use origin_primitives::{
	identifier::Ss58Identifier,
	view::{maybe_utf8, AttributeValueView},
	view_api::{
		AuthorizationRequest, EntityAttributeHistoryRequest, EntityLinkedAccountsRequest,
		TokenStateVersionRequest, TokenTimelineRequest,
	},
};
use serde::Serialize;
use std::collections::BTreeMap;
use subxt::utils::AccountId32;

use crate::types::token::StateEventRecord;
use scale_value::{Composite, Value, ValueDef};

#[derive(Default, Clone)]
pub struct EntityChainState {
	reserved: BTreeMap<String, String>,
	attributes: BTreeMap<String, String>,
}

impl EntityChainState {
	pub fn from_profile(profile: &serde_json::Value) -> Self {
		let mut state = EntityChainState::default();
		let reserved_fields = ["display", "web", "email"];
		for key in reserved_fields {
			let value = profile.get(key).and_then(|v| v.as_str()).map(|s| s.to_string());
			state.insert_reserved(key, value);
		}
		if let Some(attrs) = profile.get("attributes").and_then(|v| v.as_object()) {
			for (key, value) in attrs {
				let val = value.as_str().map(|s| s.to_string());
				state.insert_attribute(key.clone(), val);
			}
		}
		state
	}

	pub fn from_record(info: &EntityInfoRecord) -> Self {
		let mut state = EntityChainState::default();
		state.insert_reserved("display", element_text_from_view(&info.display));
		state.insert_reserved("web", element_text_from_view(&info.web));
		state.insert_reserved("email", element_text_from_view(&info.email));
		if let Some(attrs) = &info.attributes {
			for attr in attrs {
				let label = attribute_label(attr);
				let text = element_text_from_view(&attr.value);
				state.insert_attribute(label, text);
			}
		}
		state
	}

	pub fn get(&self, key: &str) -> Option<&str> {
		self.reserved
			.get(key)
			.map(|s| s.as_str())
			.or_else(|| self.attributes.get(key).map(|s| s.as_str()))
	}

	fn insert_reserved(&mut self, key: &str, value: Option<String>) {
		if let Some(val) = value {
			self.reserved.insert(key.to_string(), val);
		}
	}

	fn insert_attribute(&mut self, key: String, value: Option<String>) {
		if let Some(val) = value {
			self.attributes.insert(key, val);
		}
	}

	pub fn attributes_iter(&self) -> std::collections::btree_map::Iter<'_, String, String> {
		self.attributes.iter()
	}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AttributePlan {
	Skip,
	Add,
	Rotate,
}

pub fn plan_attribute_update(current: Option<&str>, desired: &str) -> AttributePlan {
	match current {
		Some(existing) if existing == desired => AttributePlan::Skip,
		Some(_) => AttributePlan::Rotate,
		None => AttributePlan::Add,
	}
}

pub async fn fetch_entity_chain_state<F>(
	client: &Client,
	token: &Ss58Identifier,
	auth_builder: &mut F,
) -> Result<EntityChainState>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let auth = auth_builder()?;
	if let Ok(Some(info)) = client.query().entity().details(&auth, token).await {
		return Ok(EntityChainState::from_record(&info));
	}
	// Fallback to dynamic decoding to remain forward-compatible with new element variants.
	let args = build_entity_details_args(&auth, token)?;
	if let Ok(value) = client.origin().call_view("Entity", "details", args).await {
		if let Ok(Some(state)) = parse_entity_info_value(&value, client.origin()).await {
			return Ok(state);
		}
	}
	Ok(EntityChainState::default())
}

#[derive(Clone)]
pub struct TokenTimelineEntry {
	pub version: u32,
	pub event: StateEventRecord,
}

#[derive(Clone, Serialize)]
pub struct TimelineRow {
	pub version: u64,
	pub action: String,
	pub digest: String,
	pub block: u32,
	pub extrinsic: u32,
}

const TIMELINE_PAGE_SIZE: u32 = 32;
const TIMELINE_MAX_RETRIES: usize = 6;
const LINKED_ACCOUNTS_RETRIES: usize = 5;
const ATTRIBUTE_HISTORY_RETRIES: usize = 3;

pub async fn collect_attribute_history<F>(
	client: &Client,
	token_identifier: &Ss58Identifier,
	auth_builder: &mut F,
) -> Result<Vec<HistoryEntry>>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let mut attempt = 0usize;
	loop {
		let history_req = EntityAttributeHistoryRequest {
			auth: auth_builder()?,
			token: token_identifier.clone(),
		};
		let mut combined = match client.query().entity().attribute_history(&history_req).await {
			Ok(entries) => entries,
			Err(SdkError::NotFound(_)) => Vec::new(),
			Err(err) => return Err(err),
		};
		combined
			.sort_by(|a, b| (b.block.height, b.block.index).cmp(&(a.block.height, a.block.index)));
		debug!(
			target: "sdk::entity::history",
			"attribute_history attempt #{attempt} returned {} entries:\n{:#?}",
			combined.len(),
			combined
		);

		if attempt >= ATTRIBUTE_HISTORY_RETRIES || !combined.is_empty() {
			return Ok(combined);
		}
		attempt += 1;
	}
}

pub async fn fetch_linked_accounts<F>(
	client: &Client,
	token: &Ss58Identifier,
	auth_builder: &mut F,
) -> Result<Vec<AccountId32>>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let mut attempt = 0usize;
	loop {
		let req = EntityLinkedAccountsRequest { auth: auth_builder()?, token: token.clone() };
		let result = client.query().entity().linked_accounts(&req).await;
		match result {
			Ok(accounts) => {
				if !accounts.is_empty() || attempt >= LINKED_ACCOUNTS_RETRIES {
					return Ok(accounts);
				}
			},
			Err(err) => {
				let decode_error = matches!(err, Error::ViewDecode(_) | Error::Codec(_));
				if attempt >= LINKED_ACCOUNTS_RETRIES {
					// Fallback: return empty vector instead of erroring the flow.
					return Ok(Vec::new());
				}
				if decode_error {
					// If decode failed, back off and retry once more; keep loop going.
				} else {
					return Err(err);
				}
			},
		}
		attempt += 1;
	}
}

pub async fn fetch_state_version<F>(
	client: &Client,
	token: &Ss58Identifier,
	auth_builder: &mut F,
) -> Result<u32>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let req = TokenStateVersionRequest { auth: auth_builder()?, token: token.clone() };
	client.query().token().state_version(&req).await
}

pub async fn fetch_full_token_timeline<F>(
	client: &Client,
	token: &Ss58Identifier,
	min_expected: u32,
	auth_builder: &mut F,
) -> Result<Vec<TokenTimelineEntry>>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let mut attempt = 0usize;
	loop {
		let current_version = fetch_state_version(client, token, auth_builder).await.unwrap_or(0);
		let entries = collect_timeline_once(client, token, auth_builder).await?;
		let have = entries.len() as u32;
		let required = current_version.max(min_expected);
		if have >= required || attempt >= TIMELINE_MAX_RETRIES {
			return Ok(entries);
		}
		attempt += 1;
	}
}

pub fn build_token_activity(entries: &[TokenTimelineEntry]) -> Vec<TimelineRow> {
	let mut ordered = entries.to_vec();
	ordered.sort_by(|a, b| b.version.cmp(&a.version));
	ordered
		.into_iter()
		.map(|entry| TimelineRow {
			version: entry.version as u64,
			action: maybe_utf8(entry.event.action.as_slice())
				.unwrap_or_else(|| format!("0x{}", hex::encode(&entry.event.action))),
			digest: format!("0x{}", hex::encode(entry.event.digest)),
			block: entry.event.seal.height,
			extrinsic: entry.event.seal.index,
		})
		.collect()
}

pub async fn submit_attribute_add<'a, S, F>(
	submitter: &mut TxSubmitter<'a, S>,
	key: &str,
	value: String,
	mut handler: F,
) -> Result<(), SubmitError>
where
	S: subxt::tx::Signer<OriginConfig>,
	F: FnMut(SubmitStage),
{
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: base64_element(value.as_bytes()),
	};
	let call = submitter
		.client()
		.tx()
		.entity_add_attributes(vec![entry])
		.await
		.map_err(SubmitError::from_origin_error)?;
	submitter
		.submit_with_progress(call, format!("Added attribute '{key}'"), |stage| handler(stage))
		.await?;
	Ok(())
}

pub async fn submit_attribute_rotation<'a, S, F>(
	submitter: &mut TxSubmitter<'a, S>,
	key: &str,
	value: String,
	mut handler: F,
) -> Result<(), SubmitError>
where
	S: subxt::tx::Signer<OriginConfig>,
	F: FnMut(SubmitStage),
{
	let entry = AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: base64_element(value.as_bytes()),
	};
	let call = submitter
		.client()
		.tx()
		.entity_rotate_attribute(entry)
		.await
		.map_err(SubmitError::from_origin_error)?;
	submitter
		.submit_with_progress(call, format!("Rotated attribute '{key}'"), |stage| handler(stage))
		.await?;
	Ok(())
}

pub async fn submit_entity_nym<'a, S, F>(
	submitter: &mut TxSubmitter<'a, S>,
	prefix: &str,
	mut handler: F,
) -> Result<bool, SubmitError>
where
	S: subxt::tx::Signer<OriginConfig>,
	F: FnMut(SubmitStage),
{
	let call = submitter
		.client()
		.tx()
		.entity_set_entity_nym(prefix)
		.await
		.map_err(SubmitError::from_origin_error)?;
	match submitter
		.submit_with_progress(call, "Set entity nym", |stage| handler(stage))
		.await
	{
		Ok(_) => Ok(true),
		Err(SubmitError::Node(message)) if message.contains("Entity::EntityNymTaken") => Ok(false),
		Err(err) => Err(err),
	}
}

fn element_value_to_text(value: &Value<u32>) -> Option<String> {
	match &value.value {
		ValueDef::Variant(variant) => {
			let field = match &variant.values {
				Composite::Named(fields) => fields.get(0).map(|(_, v)| v),
				Composite::Unnamed(fields) => fields.get(0),
			};
			match variant.name.as_str() {
				"None" => None,
				"Raw" => field.and_then(|v| bytes_from_value(v).ok()).and_then(|b| {
					String::from_utf8(b.clone())
						.ok()
						.or_else(|| Some(format!("0x{}", hex::encode(b))))
				}),
				"Bool" => field?.as_u128().map(|b| (b != 0).to_string()),
				"U64" => field?.as_u128().map(|n| (n as u64).to_string()),
				"U128" => field?.as_u128().map(|n| n.to_string()),
				"Hash" => field
					.and_then(|v| bytes_from_value(v).ok())
					.map(|b| format!("0x{}", hex::encode(b))),
				"Token" => field.and_then(|v| bytes_from_value(v).ok()).map(|b| {
					String::from_utf8(b.clone())
						.ok()
						.unwrap_or_else(|| format!("0x{}", hex::encode(b)))
				}),
				"CID" => field
					.and_then(|v| bytes_from_value(v).ok())
					.map(|b| bs58::encode(b).into_string()),
				_ => None,
			}
		},
		_ => None,
	}
}

fn bytes_from_value(v: &Value<u32>) -> Result<Vec<u8>, Error> {
	match &v.value {
		ValueDef::Composite(comp) => {
			let mut acc = Vec::new();
			match comp {
				Composite::Named(fields) => {
					for (_, field) in fields {
						acc.extend(bytes_from_value(field)?);
					}
				},
				Composite::Unnamed(fields) => {
					for field in fields {
						acc.extend(bytes_from_value(field)?);
					}
				},
			}
			Ok(acc)
		},
		ValueDef::Primitive(p) => {
			if let Some(u) = p.as_u128() {
				Ok(vec![u as u8])
			} else {
				Err(Error::Codec("expected byte primitive".into()))
			}
		},
		_ => Err(Error::Codec("unsupported byte shape".into())),
	}
}

fn parse_attributes(attr_value: &Value<u32>, state: &mut EntityChainState) {
	let variant = match &attr_value.value {
		ValueDef::Variant(v) => v,
		_ => return,
	};
	if variant.name != "Some" {
		return;
	}
	match &variant.values {
		Composite::Named(fields) => {
			for (_, entry) in fields {
				if let ValueDef::Composite(comp_inner) = &entry.value {
					let mut iter = match comp_inner {
						Composite::Named(inner) => inner.iter().map(|(_, v)| v).collect::<Vec<_>>(),
						Composite::Unnamed(inner) => inner.iter().collect::<Vec<_>>(),
					}
					.into_iter();
					let Some(key_val) = iter.next() else { continue };
					let Some(elem_val) = iter.next() else { continue };
					let key_bytes = bytes_from_value(key_val);
					let key_label = key_bytes
						.as_ref()
						.ok()
						.and_then(|b| std::str::from_utf8(b).ok().map(|s| s.to_string()))
						.unwrap_or_else(|| {
							format!("0x{}", hex::encode(key_bytes.unwrap_or_default()))
						});
					if let Some(text) = element_value_to_text(elem_val) {
						state.insert_attribute(key_label, Some(text));
					}
				}
			}
		},
		Composite::Unnamed(fields) => {
			for entry in fields {
				if let ValueDef::Composite(comp_inner) = &entry.value {
					let mut iter = match comp_inner {
						Composite::Named(inner) => inner.iter().map(|(_, v)| v).collect::<Vec<_>>(),
						Composite::Unnamed(inner) => inner.iter().collect::<Vec<_>>(),
					}
					.into_iter();
					let Some(key_val) = iter.next() else { continue };
					let Some(elem_val) = iter.next() else { continue };
					let key_bytes = bytes_from_value(key_val);
					let key_label = key_bytes
						.as_ref()
						.ok()
						.and_then(|b| std::str::from_utf8(b).ok().map(|s| s.to_string()))
						.unwrap_or_else(|| {
							format!("0x{}", hex::encode(key_bytes.unwrap_or_default()))
						});
					if let Some(text) = element_value_to_text(elem_val) {
						state.insert_attribute(key_label, Some(text));
					}
				}
			}
		},
	}
}

async fn parse_entity_info_value(
	value: &Value<u32>,
	client: crate::origin_client::OriginClient,
) -> Result<Option<EntityChainState>> {
	let _helpers = DynHelpers::new(client.layout());
	let info = match &value.value {
		ValueDef::Variant(v) if v.name == "Ok" => match &v.values {
			Composite::Named(fields) => fields.get(0).map(|(_, v)| v),
			Composite::Unnamed(fields) => fields.get(0),
		},
		_ => return Ok(None),
	};
	let Some(Value { value: ValueDef::Composite(info_fields), .. }) = info else {
		return Ok(None);
	};

	let mut state = EntityChainState::default();
	match info_fields {
		Composite::Named(fields) => {
			for (name, field) in fields {
				match name.as_str() {
					"display" => {
						if let Some(text) = element_value_to_text(field) {
							state.insert_reserved("display", Some(text));
						}
					},
					"web" => {
						if let Some(text) = element_value_to_text(field) {
							state.insert_reserved("web", Some(text));
						}
					},
					"email" => {
						if let Some(text) = element_value_to_text(field) {
							state.insert_reserved("email", Some(text));
						}
					},
					"attributes" => parse_attributes(field, &mut state),
					_ => {},
				}
			}
		},
		Composite::Unnamed(fields) => {
			// Unexpected shape; best-effort scan for first four fields.
			for field in fields {
				if let ValueDef::Variant(_) = field.value {
					if let Some(text) = element_value_to_text(field) {
						state.insert_reserved("display", Some(text));
					}
				}
			}
		},
	}
	Ok(Some(state))
}

fn build_entity_details_args(auth: &AuthorizationRequest, token: &Ss58Identifier) -> Result<Value> {
	let raw: [u8; 32] = *auth.account.as_ref();
	let account = types::account_id_value(&subxt::utils::AccountId32::from(raw));
	let payload = types::bytes_value(auth.payload.as_slice());
	let signature = match &auth.signature {
		origin_primitives::Signature::Sr25519(sig) => {
			Value::unnamed_variant("Sr25519", [types::bytes_value(sig.as_ref())])
		},
		origin_primitives::Signature::Ed25519(sig) => {
			Value::unnamed_variant("Ed25519", [types::bytes_value(sig.as_ref())])
		},
		origin_primitives::Signature::Ecdsa(sig) => {
			Value::unnamed_variant("Ecdsa", [types::bytes_value(sig.as_ref())])
		},
	};
	let auth_value = Value::named_composite([
		("account", account),
		("payload", payload),
		("signature", signature),
	]);
	let token_value = types::identifier_struct(token);
	Ok(Value::named_composite([("auth", auth_value), ("token", token_value)]))
}

pub fn build_entity_nym_args(req: &origin_primitives::view_api::EntityNymRequest) -> Result<Value> {
	let raw: [u8; 32] = *req.auth.account.as_ref();
	let account = types::account_id_value(&subxt::utils::AccountId32::from(raw));
	let payload = types::bytes_value(req.auth.payload.as_slice());
	let signature = match &req.auth.signature {
		origin_primitives::Signature::Sr25519(sig) => {
			Value::unnamed_variant("Sr25519", [types::bytes_value(sig.as_ref())])
		},
		origin_primitives::Signature::Ed25519(sig) => {
			Value::unnamed_variant("Ed25519", [types::bytes_value(sig.as_ref())])
		},
		origin_primitives::Signature::Ecdsa(sig) => {
			Value::unnamed_variant("Ecdsa", [types::bytes_value(sig.as_ref())])
		},
	};
	let auth_value = Value::named_composite([
		("account", account),
		("payload", payload),
		("signature", signature),
	]);
	let token_value = types::identifier_struct(&req.token);
	Ok(Value::named_composite([("auth", auth_value), ("token", token_value)]))
}

fn base64_element(bytes: &[u8]) -> ElementJson {
	ElementJson::RawBase64(BASE64.encode(bytes))
}

fn attribute_label(attr: &AttributeValueView) -> String {
	match core::str::from_utf8(attr.key.as_slice()) {
		Ok(text) => text.to_string(),
		Err(_) => format!("0x{}", hex::encode(attr.key.as_slice())),
	}
}

async fn collect_timeline_once<F>(
	client: &Client,
	token: &Ss58Identifier,
	auth_builder: &mut F,
) -> Result<Vec<TokenTimelineEntry>>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let mut cursor = Some(0u32);
	let mut version_cursor = 0u32;
	let mut rows = Vec::new();
	loop {
		let req = TokenTimelineRequest {
			auth: auth_builder()?,
			token: token.clone(),
			start: cursor,
			limit: Some(TIMELINE_PAGE_SIZE),
		};
		let (batch, next_cursor) = client.query().token().timeline(&req).await?;
		if batch.is_empty() {
			break;
		}
		for entry in batch {
			rows.push(TokenTimelineEntry { version: version_cursor, event: entry });
			version_cursor = version_cursor.saturating_add(1);
		}
		match next_cursor {
			Some(next) => {
				cursor = Some(next);
				version_cursor = next;
			},
			None => break,
		}
	}
	Ok(rows)
}
