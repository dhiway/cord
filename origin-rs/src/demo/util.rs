use crate::{
	error::{Error, Result},
	params::config::CordConfig,
	query::{auth::AuthorizationBuilder, register::PacketSnapshotView},
	tx::{self, MetaTxOptions, SubmitError, SubmitStage, TxSubmitter},
	types::token::StateEventRecord,
	utils, Client,
};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	registry::RegistryInfoView,
	view_api::{
		AuthorizationRequest, EntityAccountTokenRequest, RegisterDetailsRequest,
		RegisterPacketSnapshotByTokenRequest, TokenTimelineRequest,
	},
};
use serde_json::Value as JsonValue;
use sp_runtime::AccountId32 as RuntimeAccount;
use std::{sync::Once, time::Duration};
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
	) -> Result<ExtrinsicEvents<CordConfig>, SubmitError> {
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

pub fn fresh_authorization(valid_until: u32, signer: &tx::signer::Keypair) -> Result<AuthorizationRequest> {
	AuthorizationBuilder::from_signer(signer, valid_until, None)
		.map_err(|e| Error::Signer(e.to_string()))?
		.as_request()
		.map_err(|e| Error::Signer(e.to_string()))
}

pub async fn fresh_authorization_with_client(
	client: &Client,
	signer: &tx::signer::Keypair,
) -> Result<AuthorizationRequest> {
	let valid_until = client.view_auth_valid_until().await?;
	fresh_authorization(valid_until, signer)
}

pub async fn ensure_entity_token_verbose(
	client: &Client,
	signer: &tx::signer::Keypair,
	account_id: &AccountId32,
	profile: &JsonValue,
	tx_executor: &mut TxExecutor<'_, '_>,
) -> Result<(Ss58Identifier, bool, Vec<String>)> {
	let raw: [u8; 32] = *account_id.as_ref();
	let runtime_account = RuntimeAccount::from(raw);
	let request =
		EntityAccountTokenRequest { auth: fresh_authorization(signer)?, account: runtime_account };
	if let Some(token) = client.query().entity().account_token(&request).await? {
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
) -> Result<ExtrinsicEvents<CordConfig>, SubmitError>
where
	S: subxt::tx::Signer<CordConfig>,
	P: Payload,
{
	let events = submitter
		.submit_with_progress(call, description.to_string(), |stage| sink.stage(stage))
		.await?;
	utils::short_delay(Duration::from_secs(1)).await;
	Ok(events)
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
	if let Some(_) = client.query().entity().details(auth, token).await? {
		return Ok(TokenTarget::Entity { token: token.clone() });
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
