use crate::{
	client::Client,
	error::{Error as SdkError, Result},
	params::config::CordConfig,
	tx::submitter::{SubmitError, SubmitStage, TxSubmitter},
	types::{
		self,
		element::element_text_from_view,
		entity::{AttributeEntry, EntityInfoRecord, HistoryEntry},
		ElementJson,
	},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cord_primitives::{
	identifier::Ss58Identifier,
	view::{maybe_utf8, AttributeValueView},
	view_api::{
		AttributeKey, AuthorizationRequest, EntityAttributeHistoryForKeyRequest,
		EntityAttributeHistoryRequest, EntityLinkedAccountsRequest, TokenStateVersionRequest,
		TokenTimelineRequest,
	},
};
use hex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use subxt::utils::AccountId32;

use crate::types::token::StateEventRecord;

#[derive(Default, Clone)]
pub struct EntityChainState {
	reserved: BTreeMap<String, String>,
	attributes: BTreeMap<String, String>,
}

impl EntityChainState {
	pub fn from_profile(profile: &serde_json::Value) -> Self {
		let mut state = EntityChainState::default();
		let reserved_fields = ["display", "legal", "web", "email", "twitter"];
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
		state.insert_reserved("legal", element_text_from_view(&info.legal));
		state.insert_reserved("web", element_text_from_view(&info.web));
		state.insert_reserved("email", element_text_from_view(&info.email));
		state.insert_reserved("twitter", element_text_from_view(&info.twitter));
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
	match client.query().entity().details(&auth, token).await? {
		Some(info) => Ok(EntityChainState::from_record(&info)),
		None => Ok(EntityChainState::default()),
	}
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
	mutated_keys: &BTreeSet<Vec<u8>>,
	schema_keys: &BTreeSet<Vec<u8>>,
	auth_builder: &mut F,
) -> Result<Vec<HistoryEntry>>
where
	F: FnMut() -> Result<AuthorizationRequest>,
{
	let history_req =
		EntityAttributeHistoryRequest { auth: auth_builder()?, token: token_identifier.clone() };
	let baseline = match client.query().entity().attribute_history(&history_req).await {
		Ok(entries) => entries,
		Err(SdkError::NotFound(_)) => Vec::new(),
		Err(err) => return Err(err),
	};

	let mut keys_to_fetch: BTreeSet<Vec<u8>> = baseline
		.iter()
		.filter_map(|entry| types::hex_to_bytes(&entry.key_hex).ok())
		.filter(|bytes| !bytes.is_empty())
		.collect();
	keys_to_fetch.extend(mutated_keys.iter().cloned());
	keys_to_fetch.extend(schema_keys.iter().cloned());

	let target_hex: Vec<String> =
		mutated_keys.iter().map(|key| format!("0x{}", hex::encode(key))).collect();

	let mut combined = baseline.clone();
	let mut attempt = 0usize;
	loop {
		let mut extras = Vec::new();
		for key in &keys_to_fetch {
			let Ok(bounded_key) = AttributeKey::try_from(key.clone()) else {
				continue;
			};
			let key_req = EntityAttributeHistoryForKeyRequest {
				auth: auth_builder()?,
				token: token_identifier.clone(),
				key: bounded_key,
			};
			match client.query().entity().attribute_history_for_key(&key_req).await {
				Ok(mut extra) => extras.append(&mut extra),
				Err(SdkError::NotFound(_)) => {},
				Err(err) => return Err(err),
			}
		}
		combined.extend(extras.into_iter());
		combined
			.sort_by(|a, b| (b.block.height, b.block.index).cmp(&(a.block.height, a.block.index)));
		let complete = target_hex.is_empty()
			|| target_hex.iter().all(|hex_key| {
				combined.iter().any(|entry| entry.key_hex.eq_ignore_ascii_case(hex_key))
			});
		if complete || attempt >= ATTRIBUTE_HISTORY_RETRIES {
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
		match client.query().entity().linked_accounts(&req).await {
			Ok(accounts) => {
				if !accounts.is_empty() || attempt >= LINKED_ACCOUNTS_RETRIES {
					return Ok(accounts);
				}
			},
			Err(err) => {
				if attempt >= LINKED_ACCOUNTS_RETRIES {
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
	S: subxt::tx::Signer<CordConfig>,
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
	S: subxt::tx::Signer<CordConfig>,
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
	S: subxt::tx::Signer<CordConfig>,
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
