use crate::{
	demo::spinner,
	error::{Error, Result},
	params::config::OriginConfig,
	query::{auth::AuthorizationBuilder, register::PacketSnapshotView},
	tx::{self, MetaTxOptions, SubmitError, SubmitStage, TxSubmitter},
	types::token::StateEventRecord,
	Client,
};
use codec::{Decode, Encode};
use origin_primitives::{
	identifier::Ss58Identifier,
	registry::RegistryInfoView,
	view_api::{
		AuthorizationError, AuthorizationRequest, EntityAccountTokenRequest, EntityOverviewRequest,
		RegisterDetailsRequest, RegisterPacketSnapshotByTokenRequest, TokenTimelineRequest,
	},
};
use scale_value::{Composite, Value};
use serde::Serialize;
use serde_json::Value as JsonValue;
use sp_runtime::AccountId32 as RuntimeAccount;
use std::sync::Once;
use subxt::{
	blocks::ExtrinsicEvents,
	tx::{DynamicPayload, Payload},
	utils::{AccountId32, H256},
};

static LOG_INIT: Once = Once::new();

/// Ensures demo logging is configured once for the process.
pub fn init_logging() {
	LOG_INIT.call_once(|| {
		let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
			.format_timestamp_secs()
			.try_init();
	});
}

#[derive(Clone, Copy, Debug)]
pub enum ViewStyle {
	Compact,
	Full,
}

impl ViewStyle {
	pub fn is_full(&self) -> bool {
		matches!(self, ViewStyle::Full)
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode {
	Transaction,
	View,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TxFlow {
	Direct,
	Relayed,
}

pub enum TxExecutor<'a, 'b> {
	Direct {
		submitter: &'b mut TxSubmitter<'a, tx::signer::Keypair>,
	},
	Relayed {
		relayer: &'b mut TxSubmitter<'a, tx::signer::Keypair>,
		meta_signer: &'a tx::signer::Keypair,
	},
}

impl<'a, 'b> TxExecutor<'a, 'b> {
	pub async fn submit(
		&mut self,
		client: &Client,
		call: DynamicPayload,
		description: &str,
		sink: &mut LogSink<'_>,
	) -> Result<ExtrinsicEvents<OriginConfig>, SubmitError> {
		match self {
			TxExecutor::Direct { submitter } => {
				submit_with_logging(submitter, call, description, sink).await
			},
			TxExecutor::Relayed { relayer, meta_signer } => {
				let payload = client
					.tx()
					.meta_dispatch(call, meta_signer, MetaTxOptions::default())
					.await
					.map_err(SubmitError::from_origin_error)?;
				submit_with_logging(relayer, payload, &format!("Meta {description}"), sink).await
			},
		}
	}
}

pub struct LogSink<'a> {
	buffer: Option<&'a mut Vec<String>>,
}

impl<'a> LogSink<'a> {
	pub fn new(buffer: Option<&'a mut Vec<String>>) -> Self {
		Self { buffer }
	}

	pub fn stage(&mut self, stage: SubmitStage) {
		let message = match stage {
			SubmitStage::Validated => "  ↳ 🟡 validated and queued".to_string(),
			SubmitStage::Broadcasted => "  ↳ 📡 broadcast to peers".to_string(),
			SubmitStage::Retracted => {
				"  ↳ ⚠️ retracted from best block, waiting for re-inclusion".to_string()
			},
			SubmitStage::InBlock { hash, label } => {
				format!("  ↳ 📦 included in block {}", block_display(label, &hash))
			},
			SubmitStage::Finalized { description, .. } => {
				format!("  ↳ 🛡️ finalized {description}")
			},
			SubmitStage::Completed { description } => format!("  ↳ ✅ {description}"),
		};
		self.line(message);
	}

	pub fn line(&mut self, msg: impl Into<String>) {
		let text = msg.into();
		if let Some(buf) = self.buffer.as_deref_mut() {
			buf.push(text);
		} else {
			println!("{}", text);
		}
	}
}

fn block_display(label: Option<String>, hash: &H256) -> String {
	label.unwrap_or_else(|| format!("{hash:?}"))
}

pub fn signer_account_id(signer: &tx::signer::Keypair) -> AccountId32 {
	signer.account_id()
}

pub fn parse_identifier(value: &str) -> Result<Ss58Identifier> {
	Ss58Identifier::try_from(value.to_string())
		.map_err(|_| Error::Params(format!("invalid identifier: {value}")))
}

pub fn fresh_authorization(
	reference_block: u32,
	signer: &tx::signer::Keypair,
) -> Result<AuthorizationRequest> {
	let context = AuthorizationBuilder::default_context();
	AuthorizationBuilder::generate_view_authorization(signer, &context, reference_block, None)
		.map_err(|e| Error::Signer(e.to_string()))
}

pub async fn fresh_authorization_with_client(
	client: &Client,
	signer: &tx::signer::Keypair,
) -> Result<AuthorizationRequest> {
	let reference_block = client.view_auth_reference_block().await?;
	fresh_authorization(reference_block, signer)
}

pub fn log_view_payload<T: Serialize>(enabled: bool, label: &str, phase: &str, payload: &T) {
	if !enabled {
		return;
	}
	match serde_json::to_string_pretty(payload) {
		Ok(json) => println!("\n🔍 {label} {phase}:\n{json}"),
		Err(err) => println!("\n🔍 {label} {phase}: <serialization error: {err}>"),
	}
}

fn format_identifier(id: &Ss58Identifier) -> String {
	id.to_string_lossy()
}

pub async fn ensure_entity_token_verbose(
	client: &Client,
	signer: &tx::signer::Keypair,
	account_id: &AccountId32,
	profile: &JsonValue,
	tx_executor: &mut TxExecutor<'_, '_>,
	view_debug: bool,
) -> Result<(Ss58Identifier, bool, Vec<String>)> {
	let raw: [u8; 32] = *account_id.as_ref();
	let runtime_account = RuntimeAccount::from(raw);
	let request = EntityAccountTokenRequest {
		auth: fresh_authorization_with_client(client, signer).await?,
		account: runtime_account.clone(),
	};
	log_view_payload(view_debug, "Entity.account_token", "request", &request);
	let token_lookup = client.query().entity().account_token(&request).await?;
	if view_debug {
		let response = token_lookup
			.as_ref()
			.map(|id| format_identifier(id))
			.unwrap_or_else(|| "None".into());
		log_view_payload(view_debug, "Entity.account_token", "response", &response);
	}
	if let Some(token) = token_lookup {
		return Ok((token, false, Vec::new()));
	}

	let mut logs = Vec::new();
	let call = client.tx().entity_set_info_json(profile.clone()).await?;
	let mut sink = LogSink::new(Some(&mut logs));
	let events = tx_executor
		.submit(client, call, "Set entity info", &mut sink)
		.await
		.map_err(|e| Error::Signer(e.to_string()))?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: AccountId32 =
				Decode::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
			let token: Ss58Identifier =
				Decode::decode(&mut cursor).map_err(|e| Error::Codec(e.to_string()))?;
			logs.push("\nℹ️ Setting entity nym".to_string());
			return Ok((token, true, logs));
		}
	}
	Err(Error::NotFound("EntityInfoSet event not found".into()))
}

async fn submit_with_logging<S, P>(
	submitter: &mut TxSubmitter<'_, S>,
	call: P,
	description: &str,
	sink: &mut LogSink<'_>,
) -> Result<ExtrinsicEvents<OriginConfig>, SubmitError>
where
	S: subxt::tx::Signer<OriginConfig>,
	P: Payload,
{
	let spinner = spinner::Spinner::start(format!("Submitting {description}"));
	let events = submitter
		.submit_with_progress(call, description.to_string(), |stage| sink.stage(stage))
		.await;
	spinner.finish(None).await;
	events
}

pub enum TokenTarget {
	Entity { token: Ss58Identifier },
	Registry { registry: Ss58Identifier, info: RegistryInfoView },
	Packet { registry: Ss58Identifier, packet: Ss58Identifier, snapshot: PacketSnapshotView },
}

pub async fn resolve_token_target(
	client: &Client,
	auth: &AuthorizationRequest,
	token: &Ss58Identifier,
) -> Result<TokenTarget> {
	let entity_req = EntityOverviewRequest {
		auth: auth.clone(),
		token: token.as_ref().to_vec(),
		history_limit: Some(1),
	};
	match client.query().entity().overview(&entity_req).await {
		Ok(Some(_)) => return Ok(TokenTarget::Entity { token: token.clone() }),
		Ok(None) => {},
		Err(Error::Codec(_) | Error::ViewDecode(_)) => {
			if try_entity_overview(client, &entity_req).await?.is_some() {
				return Ok(TokenTarget::Entity { token: token.clone() });
			}
		},
		Err(err) => return Err(err),
	}

	let reg_req = RegisterDetailsRequest { auth: auth.clone(), registry: token.clone() };
	match client.query().register().details(&reg_req).await {
		Ok(info) => return Ok(TokenTarget::Registry { registry: token.clone(), info }),
		Err(Error::NotFound(_)) => {},
		Err(err) => return Err(err),
	}

	let packet_req = RegisterPacketSnapshotByTokenRequest {
		auth: auth.clone(),
		token: token.clone(),
		version: None,
	};
	match client.query().register().packet_snapshot_by_token(&packet_req).await {
		Ok(Some(snapshot)) => {
			let registry =
				Ss58Identifier::try_from(snapshot.state.registry_ss58.clone()).map_err(|_| {
					Error::ViewDecode("packet snapshot returned invalid registry id".into())
				})?;
			return Ok(TokenTarget::Packet { registry, packet: token.clone(), snapshot });
		},
		Ok(None) => {},
		Err(Error::NotFound(_)) => {},
		Err(err) => return Err(err),
	}

	Err(Error::NotFound("token does not map to entity, registry, or packet".into()))
}

pub async fn token_timeline(
	client: &Client,
	auth: &AuthorizationRequest,
	token: &Ss58Identifier,
	limit: Option<u32>,
) -> Result<(Vec<StateEventRecord>, Option<u32>)> {
	let req = TokenTimelineRequest { auth: auth.clone(), token: token.clone(), start: None, limit };
	client.query().token().timeline(&req).await
}

async fn try_entity_overview(
	client: &Client,
	req: &EntityOverviewRequest,
) -> Result<Option<origin_primitives::view::EntityOverview>> {
	let mut entries = Vec::new();
	entries.push(("auth_bytes", Value::from_bytes(req.auth.encode())));
	entries.push(("token", Value::from_bytes(req.token.clone())));
	entries.push(("history_limit", option_u32_value(req.history_limit)));
	let args = Value::named_composite(entries);
	let raw = client.origin().call_view("Entity", "overview", args).await?;
	let plain = raw.remove_context();
	match scale_value::serde::from_value::<
		(),
		core::result::Result<origin_primitives::view::EntityOverview, AuthorizationError>,
	>(plain)
	{
		Ok(Ok(view)) => Ok(Some(view)),
		Ok(Err(AuthorizationError::NotFound)) => Ok(None),
		Ok(Err(err)) => Err(Error::ViewDecode(format!("entity.overview: {err:?}"))),
		Err(err) => Err(Error::Codec(err.to_string())),
	}
}

fn option_u32_value(value: Option<u32>) -> Value {
	match value {
		Some(v) => Value::variant("Some", Composite::unnamed(vec![Value::u128(v as u128)])),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
}
