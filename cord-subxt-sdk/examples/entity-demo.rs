use anyhow::{anyhow, Result};
use codec::Decode;
use cord_primitives::{
	identifier::Ss58Identifier,
	view_api::{AuthorizationRequest, EntityAccountTokenRequest, EntityNymRequest},
};
use oc::types::entity::HistoryEntry;
use oc::{
	demo,
	demo::entity::{self, EntitySnapshot},
	entity::{self as sdk_entity, AttributePlan, EntityChainState, TimelineRow},
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx::{self, SubmitError, SubmitStage, TxSubmitter},
	utils, ChainFlavor, Client, Error as SdkError,
};
use serde_json::json;
use sp_core::crypto::Ss58AddressFormat;
use sp_runtime::AccountId32 as RuntimeAccount;
use std::{collections::BTreeSet, time::Duration};
use subxt::{
	blocks::ExtrinsicEvents,
	utils::{AccountId32, H256},
};

fn fresh_authorization(
	signer: &tx::signer::sr25519::Keypair,
) -> oc::error::Result<AuthorizationRequest> {
	AuthorizationBuilder::from_signer(signer, SignatureScheme::Sr25519, None)
		.map_err(|e| SdkError::Signer(e.to_string()))?
		.as_request()
		.map_err(|e| SdkError::Signer(e.to_string()))
}

#[derive(Clone, Copy)]
enum ViewStyle {
	Compact,
	Full,
}

impl ViewStyle {
	fn is_full(&self) -> bool {
		matches!(self, ViewStyle::Full)
	}
}

struct CliOptions {
	view: ViewStyle,
	output_json: bool,
	node: Option<String>,
}

fn parse_args(args: &[String]) -> CliOptions {
	let mut style: Option<ViewStyle> = None;
	let mut json = false;
	let mut node: Option<String> = None;
	let mut iter = args.iter().peekable();
	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--json" | "-j" => json = true,
			"--display" | "-d" => {
				if let Some(value) = iter.next() {
					style = style_from_value(value);
				}
			},
			_ if arg.starts_with("--display=") => {
				let value = arg.trim_start_matches("--display=");
				style = style_from_value(value);
			},
			_ if arg.starts_with("-d=") => {
				style = style_from_value(arg.trim_start_matches("-d="));
			},
			"--node" | "-n" => {
				if let Some(value) = iter.next() {
					node = Some(value.clone());
				}
			},
			_ if arg.starts_with("--node=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-n=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ => {},
		}
	}
	let resolved_style =
		style.unwrap_or_else(|| if json { ViewStyle::Full } else { ViewStyle::Compact });
	CliOptions { view: resolved_style, output_json: json, node }
}

fn style_from_value(value: impl AsRef<str>) -> Option<ViewStyle> {
	match value.as_ref() {
		"full" | "Full" => Some(ViewStyle::Full),
		"compact" | "Compact" => Some(ViewStyle::Compact),
		_ => None,
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("entity-demo");
	let args: Vec<String> = std::env::args().collect();
	let cli = parse_args(&args[1..]);
	let client = utils::connect_or_default(cli.node.as_deref(), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let mut submitter = TxSubmitter::new(&client, &signer);
	let demo_value = format!("Demo attribute for {label}");
	let initial_public_key = utils::random_public_key_hex();
	let mut profile = demo::entity_profile(&label);
	seed_profile_attribute(&mut profile, "demo", &demo_value);
	seed_profile_attribute(&mut profile, "public_key", &initial_public_key);
	let style = cli.view;
	let output_json = cli.output_json;
	let mut mutated_keys: BTreeSet<Vec<u8>> = BTreeSet::new();

	let (token_identifier, created, mut setup_logs) =
		ensure_entity_token_verbose(&client, &signer, &account_id, &profile, &mut submitter)
			.await?;
	let entity_token = demo::ss58_string(&token_identifier);
	let mut snapshot = EntitySnapshot::from_profile(&profile, &entity_token);
	let mut state_auth = || fresh_authorization(&signer);
	let baseline_state_version: u32 =
		match sdk_entity::fetch_state_version(&client, &token_identifier, &mut state_auth).await {
			Ok(value) => value,
			Err(err) => {
				println!("⚠️ unable to fetch token state version ({}); assuming 0", err);
				0
			},
		};
	let mut expected_state_events = 0u32;
	let chain_state = if created {
		EntityChainState::from_profile(&profile)
	} else {
		let mut auth_builder = || fresh_authorization(&signer);
		match sdk_entity::fetch_entity_chain_state(&client, &token_identifier, &mut auth_builder)
			.await
		{
			Ok(state) => state,
			Err(err) => {
				println!(
					"⚠️ unable to fetch on-chain entity snapshot ({}); proceeding with optimistic diff",
					err
				);
				EntityChainState::default()
			},
		}
	};
	let entity_nym_prefix = utils::sanitize_entity_nym(&label);
	let nym_req =
		EntityNymRequest { auth: fresh_authorization(&signer)?, token: token_identifier.clone() };
	let existing_nym = client.query().entity().entity_nym(&nym_req).await?;
	if let Some(nym) = existing_nym {
		snapshot.set_entity_nym(nym);
	} else {
		let mut sink = LogSink::new(if created { Some(&mut setup_logs) } else { None });
		if sdk_entity::submit_entity_nym(&mut submitter, &entity_nym_prefix, |stage| {
			sink.stage(stage)
		})
		.await?
		{
			snapshot.set_entity_nym(format!("{entity_nym_prefix}.nym.org.in"));
			expected_state_events = expected_state_events.saturating_add(1);
		}
	}
	print_transaction_header(created, &snapshot);
	if created {
		for line in &setup_logs {
			println!("{line}");
		}
	}

	let email_value = format!("{label}@cord.dev");
	if created {
		println!("\n✅ Entity initialized with nym/demo/public_key attributes.");
		utils::short_delay(Duration::from_secs(2)).await;
	} else {
		match sdk_entity::plan_attribute_update(chain_state.get("email"), &email_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'email' already set; skipping extrinsic");
				snapshot.set_email(email_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(plan, &mut submitter, "email", email_value.clone()).await?;
				snapshot.set_email(email_value.clone());
				mutated_keys.insert(b"email".to_vec());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}

		match sdk_entity::plan_attribute_update(chain_state.get("demo"), &demo_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'demo' already set; skipping extrinsic");
				snapshot.set_attribute("demo", demo_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(plan, &mut submitter, "demo", demo_value.clone()).await?;
				snapshot.set_attribute("demo", demo_value.clone());
				mutated_keys.insert(b"demo".to_vec());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}

		let rotation_public_key = utils::random_public_key_hex();
		match sdk_entity::plan_attribute_update(chain_state.get("public_key"), &rotation_public_key)
		{
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'public_key' already set; skipping extrinsic");
				snapshot.set_attribute("public_key", rotation_public_key.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(
					plan,
					&mut submitter,
					"public_key",
					rotation_public_key.clone(),
				)
				.await?;
				snapshot.set_attribute("public_key", rotation_public_key.clone());
				mutated_keys.insert(b"public_key".to_vec());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}
	}

	utils::short_delay(Duration::from_secs(6)).await;

	let target_version = baseline_state_version.saturating_add(expected_state_events);
	let mut timeline_auth = || fresh_authorization(&signer);
	let token_timeline_entries = sdk_entity::fetch_full_token_timeline(
		&client,
		&token_identifier,
		target_version,
		&mut timeline_auth,
	)
	.await?;
	let combined_timeline = sdk_entity::build_token_activity(&token_timeline_entries);

	let schema_keys: BTreeSet<Vec<u8>> =
		snapshot.attributes.keys().map(|k| k.as_bytes().to_vec()).collect();
	let mut history_auth = || fresh_authorization(&signer);
	let attribute_history = sdk_entity::collect_attribute_history(
		&client,
		&token_identifier,
		&mutated_keys,
		&schema_keys,
		&mut history_auth,
	)
	.await?;

	let mut links_auth = || fresh_authorization(&signer);
	let sub_accounts =
		sdk_entity::fetch_linked_accounts(&client, &token_identifier, &mut links_auth).await?;
	snapshot.set_active_accounts(&sub_accounts, chain_prefix);

	if output_json {
		let attr_json: Vec<_> = attribute_history
			.iter()
			.map(|entry| {
				json!({
					"version": entry.version,
					"key": entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone()),
					"oldValue": utils::decode_attr_value(&entry.old_value_base64),
					"block": entry.block.height,
					"extrinsic": entry.block.index,
				})
			})
			.collect();
		let json = json!({
			"snapshot": snapshot,
			"timeline": combined_timeline,
			"attributeHistory": attr_json,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	print_entity_sections(
		&snapshot,
		&attribute_history,
		&combined_timeline,
		&sub_accounts,
		style,
		chain_prefix,
	);
	Ok(())
}

async fn ensure_entity_token_verbose(
	client: &Client,
	signer: &tx::signer::sr25519::Keypair,
	account_id: &AccountId32,
	profile: &serde_json::Value,
	submitter: &mut TxSubmitter<'_, tx::signer::sr25519::Keypair>,
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
	let events = submit_with_logging(submitter, call, "Set entity info", &mut sink)
		.await
		.map_err(|e| anyhow!(e))?;
	for ev in events.iter() {
		let ev = ev?;
		if ev.pallet_name() == "Entity" && ev.variant_name() == "EntityInfoSet" {
			let mut cursor = ev.field_bytes();
			let _: AccountId32 = Decode::decode(&mut cursor)
				.map_err(|e| anyhow!("failed to decode account: {e}"))?;
			let token: Ss58Identifier =
				Decode::decode(&mut cursor).map_err(|e| anyhow!("failed to decode token: {e}"))?;
			logs.push("\nℹ️ Setting entity nym".to_string());
			return Ok((token, true, logs));
		}
	}
	Err(anyhow!("EntityInfoSet event not found"))
}

async fn apply_attribute_plan(
	plan: AttributePlan,
	submitter: &mut TxSubmitter<'_, tx::signer::sr25519::Keypair>,
	key: &str,
	value: String,
) -> Result<()> {
	match plan {
		AttributePlan::Skip => Ok(()),
		AttributePlan::Add => {
			println!("\n➕ attribute '{key}' missing on-chain; submitting add");
			let mut sink = LogSink::new(None);
			sdk_entity::submit_attribute_add(submitter, key, value, |stage| sink.stage(stage))
				.await
				.map_err(|e| anyhow!(e))?;
			utils::short_delay(Duration::from_secs(1)).await;
			Ok(())
		},
		AttributePlan::Rotate => {
			println!("\n🔁 attribute '{key}' exists with different value; rotating");
			let mut sink = LogSink::new(None);
			sdk_entity::submit_attribute_rotation(submitter, key, value, |stage| sink.stage(stage))
				.await
				.map_err(|e| anyhow!(e))?;
			utils::short_delay(Duration::from_secs(1)).await;
			Ok(())
		},
	}
}

struct LogSink<'a> {
	buffer: Option<&'a mut Vec<String>>,
}

impl<'a> LogSink<'a> {
	fn new(buffer: Option<&'a mut Vec<String>>) -> Self {
		Self { buffer }
	}

	fn stage(&mut self, stage: SubmitStage) {
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

	fn line(&mut self, msg: impl Into<String>) {
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

async fn submit_with_logging<S>(
	submitter: &mut TxSubmitter<'_, S>,
	call: subxt::tx::DynamicPayload,
	description: &str,
	sink: &mut LogSink<'_>,
) -> Result<ExtrinsicEvents<CordConfig>, SubmitError>
where
	S: subxt::tx::Signer<CordConfig>,
{
	let events = submitter
		.submit_with_progress(call, description.to_string(), |stage| sink.stage(stage))
		.await?;
	utils::short_delay(Duration::from_secs(1)).await;
	Ok(events)
}

fn signer_account_id(signer: &tx::signer::sr25519::Keypair) -> AccountId32 {
	<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(signer)
}

fn seed_profile_attribute(profile: &mut serde_json::Value, key: &str, value: &str) {
	let Some(attrs) = profile.get_mut("attributes").and_then(|val| val.as_object_mut()) else {
		return;
	};
	attrs.insert(key.to_string(), serde_json::Value::String(value.to_string()));
}

fn print_transaction_header(created: bool, snapshot: &EntitySnapshot) {
	println!("\n🌐 Origin Entity Demo\n");
	println!("🔄 State Updates\n");

	if created {
		println!("ℹ️ Setting entity info");
	} else {
		println!("ℹ️ Entity found");
		print_identifier_block(snapshot, "    ");
	}
}

fn print_identifier_block(snapshot: &EntitySnapshot, _indent: &str) {
	println!("  ↳ • Token  : {}", snapshot.token);
	if let Some(nym) = &snapshot.entity_nym {
		println!("  ↳ • Nym    : {}", nym);
	}
}

fn print_entity_sections(
	snapshot: &EntitySnapshot,
	attr_history: &[HistoryEntry],
	timeline: &[TimelineRow],
	accounts: &[AccountId32],
	style: ViewStyle,
	chain_prefix: Ss58AddressFormat,
) {
	println!("\n⏺️ Entity Snapshot (latest block)");
	println!("\nℹ️ Identifiers");
	print_identifier_block(snapshot, "    ");
	println!("\n🈁 Info");
	print_entity_info(snapshot);
	println!("\n🔢 Attributes");
	print_attribute_list(snapshot);
	entity::print_accounts_cli(accounts, chain_prefix);
	print_attribute_history(attr_history, style.is_full());
	print_combined_timeline(timeline, style.is_full());
	println!();
}

fn print_entity_info(snapshot: &EntitySnapshot) {
	println!("  ↳ • Display : {}", snapshot.display);
	println!("    • Legal   : {}", snapshot.legal);
	println!("    • Web     : {}", snapshot.web);
	println!("    • Email   : {}", snapshot.email);
	println!("    • Twitter : {}", snapshot.twitter);
}

fn print_attribute_list(snapshot: &EntitySnapshot) {
	if snapshot.attributes.is_empty() {
		println!("  ↳ • (no custom attributes)");
		return;
	}
	for (key, value) in &snapshot.attributes {
		println!("  ↳ • {:<10} : {}", key, utils::short_label(value, MAX_LABEL_LEN));
	}
}

fn truncate_label(value: &str, max_len: usize) -> String {
	if value.len() <= max_len {
		value.to_string()
	} else {
		let mut truncated = value.chars().take(max_len.saturating_sub(1)).collect::<String>();
		truncated.push('…');
		truncated
	}
}

const MAX_LABEL_LEN: usize = 32;

fn print_combined_timeline(entries: &[TimelineRow], full_view: bool) {
	println!("\n🔀 Activity (latest first)");
	if entries.is_empty() {
		println!("    • (no recorded activity)");
		return;
	}
	println!(
		"  ↳    {:>8}  {:>8}  {:>6}    {:<30} {:<34}",
		"Version", "Block", "Index", "Action", "Digest"
	);
	let total = entries.len();
	let mut shown = 0usize;
	let iter: Box<dyn Iterator<Item = &TimelineRow>> = if full_view {
		Box::new(entries.iter().rev())
	} else {
		Box::new(entries.iter().rev().take(10))
	};
	for entry in iter {
		println!(
			"       {:>8}  {:>8}  {:>6}    {:<30} {:<34}",
			entry.version,
			format!("#{}", entry.block),
			entry.extrinsic,
			truncate_label(&entry.action, 30),
			utils::short_label(&entry.digest, MAX_LABEL_LEN)
		);
		shown += 1;
	}
	if !full_view && total > shown {
		println!("    … {} older", total - shown);
	}
}

fn print_attribute_history(entries: &[HistoryEntry], full_view: bool) {
	println!("\n🔁 Rotations (latest first)");
	if entries.is_empty() {
		println!("  ↳ • (no attribute history)");
		return;
	}
	println!("  ↳    {:>8}  {:>6}    {:<18} {:<34}", "Block", "Index", "Key", "Rotated Value");
	let total = entries.len();
	let mut shown = 0usize;
	let iter: Box<dyn Iterator<Item = &HistoryEntry>> = if full_view {
		Box::new(entries.iter().rev())
	} else {
		Box::new(entries.iter().rev().take(10))
	};
	for entry in iter {
		let key = entry.key_utf8.clone().unwrap_or_else(|| entry.key_hex.clone());
		let value =
			utils::short_label(&utils::decode_attr_value(&entry.old_value_base64), MAX_LABEL_LEN);
		println!(
			"       {:>8}  {:>6}    {:<18} {:<34}",
			format!("#{}", entry.block.height),
			entry.block.index,
			truncate_label(&key, 18),
			value
		);
		shown += 1;
	}
	if !full_view && total > shown {
		println!("    … {} older", total - shown);
	}
}
