use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cord_primitives::{identifier::Ss58Identifier, view_api::EntityNymRequest};
use oc::types::{
	self,
	entity::{AttributeEntry, HistoryEntry},
	ElementJson,
};
use oc::{
	demo,
	demo::{
		entity::{self, EntitySnapshot},
		util::{
			ensure_entity_token_verbose, fresh_authorization, init_logging, parse_identifier,
			signer_account_id, LogSink, RunMode, TxExecutor, TxFlow, ViewStyle,
		},
	},
	entity::{self as sdk_entity, AttributePlan, EntityChainState, TimelineRow},
	tx::{self, SubmitError, TxSubmitter},
	utils, ChainFlavor, Client,
};
use serde_json::json;
use sp_core::crypto::Ss58AddressFormat;
use std::time::Duration;
use subxt::utils::AccountId32;

struct CliOptions {
	view: ViewStyle,
	output_json: bool,
	node: Option<String>,
	mode: RunMode,
	flow: TxFlow,
	token: Option<String>,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	let mut style: Option<ViewStyle> = None;
	let mut json = false;
	let mut node: Option<String> = None;
	let mut mode = RunMode::Transaction;
	let mut flow = TxFlow::Direct;
	let mut token: Option<String> = None;
	let mut iter = args.iter().peekable();
	if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
		print_usage();
		std::process::exit(0);
	}

	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--json" | "-j" => json = true,
			"--display" | "-d" => {
				let value = require_value(&mut iter, arg.as_str())?;
				style = Some(parse_display_style(&value)?);
			},
			_ if arg.starts_with("--display=") => {
				let value = arg.trim_start_matches("--display=");
				style = Some(parse_display_style(value)?);
			},
			_ if arg.starts_with("-d=") => {
				style = Some(parse_display_style(arg.trim_start_matches("-d="))?);
			},
			"--node" | "-n" => {
				node = Some(require_value(&mut iter, arg.as_str())?);
			},
			_ if arg.starts_with("--node=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-n=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			"--mode" | "-m" => {
				let value = require_value(&mut iter, arg.as_str())?;
				mode = mode_from_value(&value)
					.ok_or_else(|| anyhow!("invalid --mode value: {value} (expected tx|view)"))?;
			},
			_ if arg.starts_with("--mode=") => {
				let value = arg.trim_start_matches("--mode=");
				mode = mode_from_value(value)
					.ok_or_else(|| anyhow!("invalid --mode value: {value} (expected tx|view)"))?;
			},
			"--flow" | "-f" => {
				let value = require_value(&mut iter, arg.as_str())?;
				flow = flow_from_value(&value).ok_or_else(|| {
					anyhow!("invalid --flow value: {value} (expected direct|relay)")
				})?;
			},
			_ if arg.starts_with("--flow=") => {
				let value = arg.trim_start_matches("--flow=");
				flow = flow_from_value(value).ok_or_else(|| {
					anyhow!("invalid --flow value: {value} (expected direct|relay)")
				})?;
			},
			"--token" | "-t" => {
				token = Some(require_value(&mut iter, arg.as_str())?);
			},
			_ if arg.starts_with("--token=") => {
				token = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg == "--" => break,
			_ if arg.starts_with('-') => {
				return Err(anyhow!(
					"unrecognized option '{arg}'. Use --help to view supported flags"
				));
			},
			_ => {
				return Err(anyhow!(
					"unexpected argument '{arg}'. Use --help to view supported flags"
				));
			},
		}
	}
	let resolved_style =
		style.unwrap_or_else(|| if json { ViewStyle::Full } else { ViewStyle::Compact });
	if mode == RunMode::View && token.is_none() {
		return Err(anyhow!("--token <identifier> is required in view mode"));
	}
	Ok(CliOptions { view: resolved_style, output_json: json, node, mode, flow, token })
}

fn require_value<'a>(
	iter: &mut std::iter::Peekable<std::slice::Iter<'a, String>>,
	flag: &str,
) -> Result<String> {
	iter.next()
		.map(|value| value.clone())
		.ok_or_else(|| anyhow!("{flag} expects a value"))
}

fn parse_display_style(value: &str) -> Result<ViewStyle> {
	style_from_value(value)
		.ok_or_else(|| anyhow!("invalid display style '{value}' (expected less|more/full)"))
}

fn print_usage() {
	println!(
		r#"Usage: entity-demo [OPTIONS]

Transaction mode (default):
  entity-demo
  entity-demo --flow relay

View mode (read-only):
  entity-demo --mode view --token <identifier>

Options:
  -m, --mode <tx|view>         Run mode (default: tx)
  -f, --flow <direct|relay>    Transaction flow when in transaction mode (default: direct)
  -t, --token <identifier>     Target identifier for view mode
  -d, --display <less|more>    Display style (default: less unless --json)
  -j, --json                   Emit JSON snapshot instead of CLI tables
  -n, --node <url>             WebSocket endpoint (default: ws://127.0.0.1:9944)
  -h, --help                   Show this help message
"#
	);
}

fn style_from_value(value: impl AsRef<str>) -> Option<ViewStyle> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"full" | "more" => Some(ViewStyle::Full),
		"compact" | "less" => Some(ViewStyle::Compact),
		_ => None,
	}
}

fn mode_from_value(value: impl AsRef<str>) -> Option<RunMode> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"tx" | "transaction" => Some(RunMode::Transaction),
		"view" => Some(RunMode::View),
		_ => None,
	}
}

fn flow_from_value(value: impl AsRef<str>) -> Option<TxFlow> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"direct" | "signer" => Some(TxFlow::Direct),
		"relayed" | "relay" | "meta" => Some(TxFlow::Relayed),
		_ => None,
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	init_logging();
	let args: Vec<String> = std::env::args().collect();
	let cli = parse_args(&args[1..])?;
	let client = utils::connect_or_default(cli.node.as_deref(), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	match cli.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client, chain_prefix).await,
		RunMode::View => run_view_flow(&cli, &client, chain_prefix).await,
	}
}

async fn run_transaction_flow(
	cli: &CliOptions,
	client: &Client,
	chain_prefix: Ss58AddressFormat,
) -> Result<()> {
	let label = demo::random_label("entity-demo");
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);
	let demo_value = format!("Demo attribute for {label}");
	let initial_public_key = utils::random_public_key_hex();
	let mut profile = demo::entity_profile(&label);
	seed_profile_attribute(&mut profile, "demo", &demo_value);
	seed_profile_attribute(&mut profile, "public_key", &initial_public_key);

	let mut direct_submitter = TxSubmitter::new(client, &signer);
	let mut _relayer_signer =
		if cli.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		_relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut tx_executor = match cli.flow {
		TxFlow::Direct => TxExecutor::Direct { submitter: &mut direct_submitter },
		TxFlow::Relayed => TxExecutor::Relayed {
			relayer: relayer_submitter
				.as_mut()
				.expect("relayer submitter should exist in relayed mode"),
			meta_signer: &signer,
		},
	};

	let (token_identifier, created, mut setup_logs) =
		ensure_entity_token_verbose(client, &signer, &account_id, &profile, &mut tx_executor)
			.await?;
	let entity_token = demo::ss58_string(&token_identifier);
	let mut snapshot = EntitySnapshot::from_profile(&profile, &entity_token);
	let mut state_auth = || fresh_authorization(&signer);
	let baseline_state_version: u32 =
		match sdk_entity::fetch_state_version(client, &token_identifier, &mut state_auth).await {
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
		match sdk_entity::fetch_entity_chain_state(client, &token_identifier, &mut auth_builder)
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
		let call = client.tx().entity_set_entity_nym(&entity_nym_prefix).await?;
		tx_executor.submit(client, call, "Set entity nym", &mut sink).await?;
		snapshot.set_entity_nym(format!("{entity_nym_prefix}.nym.org.in"));
		expected_state_events = expected_state_events.saturating_add(1);
	}
	print_transaction_header(created, &snapshot);
	if created {
		for line in &setup_logs {
			println!("{line}");
		}
	}

	let email_value = format!("{label}@cord.dev");
	if created {
		println!("\n✅ Entity initialized with info/nym/demo/public_key attributes.");
		utils::short_delay(Duration::from_secs(2)).await;
	} else {
		match sdk_entity::plan_attribute_update(chain_state.get("email"), &email_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'email' already set; skipping extrinsic");
				snapshot.set_email(email_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(client, &mut tx_executor, plan, "email", email_value.clone())
					.await?;
				snapshot.set_email(email_value.clone());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}

		match sdk_entity::plan_attribute_update(chain_state.get("demo"), &demo_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'demo' already set; skipping extrinsic");
				snapshot.set_attribute("demo", demo_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(client, &mut tx_executor, plan, "demo", demo_value.clone())
					.await?;
				snapshot.set_attribute("demo", demo_value.clone());
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
					client,
					&mut tx_executor,
					plan,
					"public_key",
					rotation_public_key.clone(),
				)
				.await?;
				snapshot.set_attribute("public_key", rotation_public_key.clone());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}
	}

	utils::short_delay(Duration::from_secs(6)).await;
	let target_version = baseline_state_version.saturating_add(expected_state_events);
	render_snapshot(
		client,
		&signer,
		&token_identifier,
		&mut snapshot,
		cli.view,
		cli.output_json,
		chain_prefix,
		target_version,
		false,
	)
	.await
}

async fn run_view_flow(
	cli: &CliOptions,
	client: &Client,
	chain_prefix: Ss58AddressFormat,
) -> Result<()> {
	let token_str = cli
		.token
		.as_deref()
		.ok_or_else(|| anyhow!("--token is required in view mode"))?;
	let token_identifier = parse_identifier(token_str)?;
	let signer = tx::signer::dev_alice();

	let details_auth = || fresh_authorization(&signer);
	let auth = details_auth()?;
	let entity_info = client
		.query()
		.entity()
		.details(&auth, &token_identifier)
		.await?
		.ok_or_else(|| anyhow!("no entity info found for token {}", token_str))?;
	let chain_state = EntityChainState::from_record(&entity_info);
	let mut snapshot = EntitySnapshot::from_chain_state(&chain_state, token_str);

	let nym_req =
		EntityNymRequest { auth: fresh_authorization(&signer)?, token: token_identifier.clone() };
	if let Some(nym) = client.query().entity().entity_nym(&nym_req).await? {
		snapshot.set_entity_nym(nym);
	}

	render_snapshot(
		client,
		&signer,
		&token_identifier,
		&mut snapshot,
		cli.view,
		cli.output_json,
		chain_prefix,
		0,
		true,
	)
	.await
}

async fn render_snapshot(
	client: &Client,
	signer: &tx::signer::Keypair,
	token_identifier: &Ss58Identifier,
	snapshot: &mut EntitySnapshot,
	style: ViewStyle,
	output_json: bool,
	chain_prefix: Ss58AddressFormat,
	min_expected: u32,
	include_history: bool,
) -> Result<()> {
	let mut timeline = Vec::new();
	let mut history = Vec::new();

	if include_history {
		let mut timeline_auth = || fresh_authorization(&signer);
		timeline = sdk_entity::fetch_full_token_timeline(
			client,
			token_identifier,
			min_expected,
			&mut timeline_auth,
		)
		.await?;
		let mut history_auth = || fresh_authorization(&signer);
		history =
			sdk_entity::collect_attribute_history(client, token_identifier, &mut history_auth)
				.await?;
	}

	let mut links_auth = || fresh_authorization(&signer);
	let sub_accounts =
		sdk_entity::fetch_linked_accounts(client, token_identifier, &mut links_auth).await?;
	snapshot.set_active_accounts(&sub_accounts, chain_prefix);

	if output_json {
		let attr_json: Vec<_> = history
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
			"timeline": sdk_entity::build_token_activity(&timeline),
			"attributeHistory": attr_json,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
	} else if include_history {
		print_entity_sections(
			snapshot,
			&history,
			&sdk_entity::build_token_activity(&timeline),
			&sub_accounts,
			style,
			chain_prefix,
		);
	} else {
		print_entity_summary(snapshot, &sub_accounts, chain_prefix);
	}
	Ok(())
}

async fn apply_attribute_plan(
	client: &Client,
	tx_executor: &mut TxExecutor<'_, '_>,
	plan: AttributePlan,
	key: &str,
	value: String,
) -> Result<(), SubmitError> {
	match plan {
		AttributePlan::Skip => return Ok(()),
		AttributePlan::Add => {
			println!("\n➕ attribute '{key}' missing on-chain; submitting add extrinsic");
			let entry = attribute_entry(key, value);
			let call = client
				.tx()
				.entity_add_attributes(vec![entry])
				.await
				.map_err(SubmitError::from_origin_error)?;
			let mut sink = LogSink::new(None);
			tx_executor
				.submit(client, call, &format!("Add attribute '{key}'"), &mut sink)
				.await?;
		},
		AttributePlan::Rotate => {
			println!("\n🔁 attribute '{key}' exists with different value; submitting rotation");
			let entry = attribute_entry(key, value);
			let call = client
				.tx()
				.entity_rotate_attribute(entry)
				.await
				.map_err(SubmitError::from_origin_error)?;
			let mut sink = LogSink::new(None);
			tx_executor
				.submit(client, call, &format!("Rotated attribute '{key}'"), &mut sink)
				.await?;
		},
	}
	Ok(())
}

fn attribute_entry(key: &str, value: String) -> AttributeEntry {
	AttributeEntry {
		key_hex: types::to_key_hex_from_utf8(key),
		key_utf8: Some(key.into()),
		value: base64_element(value.as_bytes()),
	}
}

fn base64_element(bytes: &[u8]) -> ElementJson {
	ElementJson::RawBase64(BASE64.encode(bytes))
}

fn seed_profile_attribute(profile: &mut serde_json::Value, key: &str, value: &str) {
	let Some(attrs) = profile.get_mut("attributes").and_then(|val| val.as_object_mut()) else {
		return;
	};
	attrs.insert(key.to_string(), serde_json::Value::String(value.to_string()));
}

fn print_transaction_header(created: bool, snapshot: &EntitySnapshot) {
	println!("\n🏷️ Origin Entity Demo\n");

	if created {
		println!("🔄 Create Entity\n");
		println!("ℹ️ Setting entity info");
	} else {
		println!("ℹ️ Entity found");
		print_identifier_block(snapshot, "    ");
		println!("\n🔄 State Updates");
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
	print_entity_core(snapshot, accounts, chain_prefix);
	print_attribute_history(attr_history, style.is_full());
	print_combined_timeline(timeline, style.is_full());
	println!();
}

fn print_entity_summary(
	snapshot: &EntitySnapshot,
	accounts: &[AccountId32],
	chain_prefix: Ss58AddressFormat,
) {
	print_entity_core(snapshot, accounts, chain_prefix);
	println!();
}

fn print_entity_core(
	snapshot: &EntitySnapshot,
	accounts: &[AccountId32],
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
	let iter: Box<dyn Iterator<Item = &TimelineRow>> =
		if full_view { Box::new(entries.iter()) } else { Box::new(entries.iter().take(10)) };
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
	let mut ordered: Vec<&HistoryEntry> = entries.iter().collect();
	ordered.sort_by(|a, b| (b.block.height, b.block.index).cmp(&(a.block.height, a.block.index)));
	let total = ordered.len();
	let mut shown = 0usize;
	let iter: Box<dyn Iterator<Item = &&HistoryEntry>> =
		if full_view { Box::new(ordered.iter()) } else { Box::new(ordered.iter().take(10)) };
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
