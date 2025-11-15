use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use oc::types::{self, entity::AttributeEntry, ElementJson};
use oc::{
	demo,
	demo::{
		cli::{parse_common_cli, require_value, CommonCliOptions},
		entity::{self, EntitySnapshot},
		util::{
			ensure_entity_token_verbose, fresh_authorization, fresh_authorization_with_client,
			init_logging, parse_identifier, resolve_token_target, signer_account_id, LogSink,
			RunMode, TokenTarget, TxExecutor, TxFlow,
		},
	},
	entity::{self as sdk_entity, AttributePlan, EntityChainState},
	tx::{self, SubmitError, TxSubmitter},
	utils, ChainFlavor, Client,
};
use origin_primitives::view_api::EntityNymRequest;
use sp_core::crypto::Ss58AddressFormat;
use std::time::Duration;

struct CliOptions {
	common: CommonCliOptions,
	token: Option<String>,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
		print_usage();
		std::process::exit(0);
	}
	let parsed = parse_common_cli(args)?;
	let mut token: Option<String> = None;
	let mut iter = parsed.rest.iter().peekable();
	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--token" | "-t" => token = Some(require_value(&mut iter, arg)?),
			_ if arg.starts_with("--token=") => {
				token = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-t=") => {
				token = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			"--" => break,
			_ => {
				return Err(anyhow!(
					"unrecognized option {arg}. Use --help to view supported flags"
				));
			},
		}
	}
	if parsed.common.mode == RunMode::View && token.is_none() {
		return Err(anyhow!("--token <identifier> is required in view mode"));
	}
	Ok(CliOptions { common: parsed.common, token })
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

#[tokio::main]
async fn main() -> Result<()> {
	init_logging();
	let args: Vec<String> = std::env::args().collect();
	let cli = parse_args(&args[1..])?;
	let client = utils::connect_or_default(cli.common.node.as_deref(), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	match cli.common.mode {
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
		if cli.common.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		_relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut tx_executor = match cli.common.flow {
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
	let state_reference_block = client.view_auth_reference_block().await?;
	let mut state_auth = || fresh_authorization(state_reference_block, &signer);
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
		let chain_state_reference_block = client.view_auth_reference_block().await?;
		let mut auth_builder = || fresh_authorization(chain_state_reference_block, &signer);
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
	let nym_req = EntityNymRequest {
		auth: fresh_authorization_with_client(client, &signer).await?,
		token: token_identifier.clone(),
	};
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
	entity::render_entity_snapshot(
		client,
		&signer,
		&token_identifier,
		&mut snapshot,
		cli.common.view,
		cli.common.output_json,
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
		.ok_or_else(|| anyhow!("--token <identifier> is required in view mode"))?;
	let requested_token = parse_identifier(token_str)?;
	let signer = tx::signer::dev_alice();
	let resolver_auth = fresh_authorization_with_client(client, &signer).await?;
	let token_identifier =
		match resolve_token_target(client, &resolver_auth, &requested_token).await? {
			TokenTarget::Entity { token } => token,
			TokenTarget::Registry { .. } => {
				return Err(anyhow!(format!(
				"token {token_str} is a registry; run register-demo --mode view --token {token_str}"
			)));
			},
			TokenTarget::Packet { .. } => {
				return Err(anyhow!(format!(
				"token {token_str} is a packet; run packet-demo --mode view --token {token_str}"
			)));
			},
		};

	let auth = fresh_authorization_with_client(client, &signer).await?;
	let entity_info = client
		.query()
		.entity()
		.details(&auth, &token_identifier)
		.await?
		.ok_or_else(|| anyhow!(format!("no entity info found for token {token_str}")))?;
	let chain_state = EntityChainState::from_record(&entity_info);
	let mut snapshot = EntitySnapshot::from_chain_state(&chain_state, token_str);

	let nym_req = EntityNymRequest {
		auth: fresh_authorization_with_client(client, &signer).await?,
		token: token_identifier.clone(),
	};
	if let Some(nym) = client.query().entity().entity_nym(&nym_req).await? {
		snapshot.set_entity_nym(nym);
	}

	entity::render_entity_snapshot(
		client,
		&signer,
		&token_identifier,
		&mut snapshot,
		cli.common.view,
		cli.common.output_json,
		chain_prefix,
		0,
		true,
	)
	.await
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
