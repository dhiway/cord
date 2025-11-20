use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use oc::{
	demo,
	demo::{
		cli::{parse_common_cli, require_value, CommonCliOptions},
		entity::{self, EntitySnapshot},
		spinner,
		util::{
			ensure_entity_token_verbose, fresh_authorization, fresh_authorization_with_client,
			init_logging, log_view_payload, parse_identifier, resolve_token_target,
			signer_account_id, LogSink, RunMode, TokenTarget, TxExecutor, TxFlow,
		},
	},
	entity::{self as sdk_entity, AttributePlan, EntityChainState, TokenTimelineEntry},
	error::Error as OcError,
	sdk::{self, OriginClient},
	tx::{self, SubmitError, TxSubmitter},
	types::{self, entity::AttributeEntry, token::StateEventRecord, ElementJson},
	utils, ChainFlavor, Client, ConnectionConfig, RetryPolicy,
};
use origin_primitives::{
	view::{AttributeValueView, EntityInfoView},
	view_api::EntityNymRequest,
};
use log::warn;
use sp_core::crypto::Ss58AddressFormat;
use std::time::Duration;

fn display_ss58(id: &origin_primitives::identifier::Ss58Identifier) -> String {
	id.to_string_lossy()
}

struct CliOptions {
	common: CommonCliOptions,
	token: Option<String>,
	view_debug: bool,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
		print_usage();
		std::process::exit(0);
	}
	let parsed = parse_common_cli(args)?;
	let mut token: Option<String> = None;
	let mut view_debug = false;
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
			"--view-debug" => view_debug = true,
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
	Ok(CliOptions { common: parsed.common, token, view_debug })
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
      --view-debug             Log request/response payloads for every runtime view call
  -h, --help                   Show this help message
"#
	);
}

#[tokio::main]
async fn main() -> Result<()> {
	init_logging();
	let args: Vec<String> = std::env::args().collect();
	let cli = parse_args(&args[1..])?;
	let node_url = cli.common.node.clone().unwrap_or_else(|| utils::DEFAULT_NODE_URL.to_string());
	let client = connect_client(Some(&node_url), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	match cli.common.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client, chain_prefix).await,
		RunMode::View => run_view_flow(&cli, &client, &node_url, chain_prefix).await,
	}
}

async fn connect_client(node: Option<&str>, flavor: ChainFlavor) -> Result<Client> {
	let endpoint = node.unwrap_or(utils::DEFAULT_NODE_URL);
	let connection = ConnectionConfig::new(endpoint.to_string(), flavor).with_retry(RetryPolicy {
		initial_backoff: Duration::from_millis(100),
		max_backoff: Duration::from_secs(5),
		max_retries: Some(8),
	});
	let spinner = spinner::Spinner::start(format!("Connecting to {endpoint} ({flavor:?})"));
	let client = Client::connect_with(connection).await?;
	spinner.finish(Some("Connected")).await;
	Ok(client)
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

	let (token_identifier, created, mut setup_logs) = ensure_entity_token_verbose(
		client,
		&signer,
		&account_id,
		&profile,
		&mut tx_executor,
		cli.view_debug,
	)
	.await?;
	let entity_token = display_ss58(&token_identifier);
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
		token: token_identifier.as_ref().to_vec(),
	};
	log_view_payload(cli.view_debug, "Entity.entity_nym", "request", &nym_req);
	let existing_nym = sanitize_nym(
		client.query().entity().entity_nym(&nym_req).await?,
		cli.view_debug,
		"Entity.entity_nym",
	);
	log_view_payload(cli.view_debug, "Entity.entity_nym", "response", &existing_nym);
	if let Some(nym) = existing_nym {
		snapshot.set_entity_nym(nym);
	} else {
		let log_buffer = if created { Some(&mut setup_logs) } else { None };
		match set_entity_nym_on_chain(
			client,
			&mut tx_executor,
			&entity_nym_prefix,
			log_buffer,
		)
		.await
		{
			Ok(true) => {
				snapshot.set_entity_nym(format!("{entity_nym_prefix}.nym.org.in"));
				expected_state_events = expected_state_events.saturating_add(1);
			},
			Ok(false) => {},
			Err(err) => return Err(err.into()),
		};
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
	} else {
		match sdk_entity::plan_attribute_update(chain_state.get("email"), &email_value) {
			AttributePlan::Skip => {
				println!("ℹ️ attribute 'email' already set; skipping extrinsic");
				snapshot.set_email(email_value.clone());
			},
			plan @ (AttributePlan::Add | AttributePlan::Rotate) => {
				apply_attribute_plan(
					client,
					&mut tx_executor,
					plan,
					"email",
					email_value.clone(),
					created,
				)
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
				apply_attribute_plan(
					client,
					&mut tx_executor,
					plan,
					"demo",
					demo_value.clone(),
					created,
				)
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
					created,
				)
				.await?;
				snapshot.set_attribute("public_key", rotation_public_key.clone());
				expected_state_events = expected_state_events.saturating_add(1);
			},
		}
	}

	// Ensure nym populated before final render (post-mint or existing token).
	if snapshot.entity_nym.is_none() {
		let req = EntityNymRequest {
			auth: fresh_authorization_with_client(client, &signer).await?,
			token: token_identifier.as_ref().to_vec(),
		};
		log_view_payload(cli.view_debug, "Entity.entity_nym", "request", &req);
		let refreshed = sanitize_nym(
			client.query().entity().entity_nym(&req).await?,
			cli.view_debug,
			"Entity.entity_nym",
		);
		log_view_payload(cli.view_debug, "Entity.entity_nym", "response", &refreshed);
		if let Some(nym) = refreshed {
			snapshot.set_entity_nym(nym);
		} else {
			match set_entity_nym_on_chain(client, &mut tx_executor, &entity_nym_prefix, None).await {
				Ok(true) => {
					snapshot.set_entity_nym(format!("{entity_nym_prefix}.nym.org.in"));
					expected_state_events = expected_state_events.saturating_add(1);
				},
				Ok(false) => {},
				Err(err) => return Err(err.into()),
			};
		}
	}

	let target_version = baseline_state_version.saturating_add(expected_state_events);
	if let Err(err) = entity::render_entity_snapshot(
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
	{
		println!("⚠️ unable to render final snapshot: {err}");
	}
	Ok(())
}

async fn run_view_flow(
	cli: &CliOptions,
	client: &Client,
	node_url: &str,
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
		match resolve_token_target(client, &resolver_auth, &requested_token).await {
			Ok(TokenTarget::Entity { token }) => token,
			Ok(TokenTarget::Registry { .. }) => {
				return Err(anyhow!(format!(
				"token {token_str} is a registry; run register-demo --mode view --token {token_str}"
			)));
			},
			Ok(TokenTarget::Packet { .. }) => {
				return Err(anyhow!(format!(
				"token {token_str} is a packet; run packet-demo --mode view --token {token_str}"
			)));
			},
			Err(OcError::Codec(_) | OcError::ViewDecode(_)) => {
				println!(
					"⚠️ token resolver hit a decode error; assuming {token_str} is an entity token"
				);
				requested_token.clone()
			},
			Err(err) => return Err(err.into()),
		};

	let sdk_client = OriginClient::connect(node_url).await?;
	let overview_auth = fresh_authorization_with_client(client, &signer).await?;
	let overview_debug = serde_json::json!({
		"auth": overview_auth.clone(),
		"token": token_identifier.clone(),
		"history_limit": null,
	});
	log_view_payload(cli.view_debug, "Entity.overview", "request", &overview_debug);
	let overview = sdk_client.entities().overview(&overview_auth, &token_identifier).await?;
	log_view_payload(cli.view_debug, "Entity.overview", "response", &overview);

	if cli.common.output_json {
		println!("{}", serde_json::to_string_pretty(&overview).unwrap());
		return Ok(());
	}

	let info_view = info_view_from_sdk_entity(&overview.entity);
	let chain_state = EntityChainState::from_record(&info_view);
	let mut snapshot = EntitySnapshot::from_chain_state(&chain_state, token_str);
	if let Some(nym) = &overview.nym {
		snapshot.set_entity_nym(nym.clone());
	}
	snapshot.set_active_accounts(&overview.linked_accounts, chain_prefix);

	let timeline_rows = timeline_rows_from_events(&overview.timeline);
	entity::print_entity_sections(
		&snapshot,
		&overview.history,
		&timeline_rows,
		&overview.linked_accounts,
		cli.common.view,
		chain_prefix,
	);
	Ok(())
}

async fn apply_attribute_plan(
	client: &Client,
	tx_executor: &mut TxExecutor<'_, '_>,
	mut plan: AttributePlan,
	key: &str,
	value: String,
	created: bool,
) -> Result<(), SubmitError> {
	if !created && matches!(plan, AttributePlan::Add) {
		// On existing entities, prefer rotation to avoid AttributeExists when we couldn't
		// accurately read the current value (e.g., new element variants).
		plan = AttributePlan::Rotate;
	}
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
			if let Err(err) = tx_executor
				.submit(client, call, &format!("Add attribute '{key}'"), &mut sink)
				.await
			{
				if matches_account_not_found(&err) {
					println!(
						"⚠️ controller not linked (AccountNotFound); skipping add for '{key}'"
					);
					return Ok(());
				}
				return Err(err);
			}
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
			if let Err(err) = tx_executor
				.submit(client, call, &format!("Rotated attribute '{key}'"), &mut sink)
				.await
			{
				if matches_account_not_found(&err) {
					println!(
						"⚠️ controller not linked (AccountNotFound); skipping rotation for '{key}'"
					);
					return Ok(());
				}
				return Err(err);
			}
		},
	}
	Ok(())
}

fn matches_account_not_found(err: &SubmitError) -> bool {
	match err {
		SubmitError::Runtime(msg) | SubmitError::Node(msg) | SubmitError::Invalid(msg) =>
			msg.contains("Entity::AccountNotFound"),
		_ => false,
	}
}

fn sanitize_nym(value: Option<String>, view_debug: bool, label: &str) -> Option<String> {
	if let Some(ref nym) = value {
		let trimmed = nym.trim();
		if trimmed.is_empty() || trimmed.chars().any(|ch| ch.is_control()) {
			log_nym_warning(
				view_debug,
				&format!("{label} returned non-printable nym; leaving original value untouched"),
			);
		}
	}
	value
}

fn log_nym_warning(view_debug: bool, message: &str) {
	if view_debug {
		println!("⚠️ {message}");
	} else {
		warn!("{message}");
	}
}

async fn set_entity_nym_on_chain(
	client: &Client,
	tx_executor: &mut TxExecutor<'_, '_>,
	entity_nym_prefix: &str,
	log_buffer: Option<&mut Vec<String>>,
) -> core::result::Result<bool, SubmitError> {
	let mut sink = match log_buffer {
		Some(buf) => LogSink::new(Some(buf)),
		None => LogSink::new(None),
	};
	let call = client
		.tx()
		.entity_set_entity_nym(entity_nym_prefix)
		.await
		.map_err(SubmitError::from_origin_error)?;
	match tx_executor.submit(client, call, "Set entity nym", &mut sink).await {
		Ok(_) => Ok(true),
		Err(SubmitError::Node(message)) if message.contains("Entity::EntityNymAlreadySet") => {
			println!("ℹ️ entity nym already set on-chain; skipping");
			Ok(false)
		},
		Err(err) => Err(err),
	}
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

fn info_view_from_sdk_entity(entity: &sdk::types::Entity) -> EntityInfoView {
	let attributes = if entity.attributes.is_empty() {
		None
	} else {
		Some(
			entity
				.attributes
				.iter()
				.map(|attr| AttributeValueView { key: attr.key.clone(), value: attr.value.clone() })
				.collect(),
		)
	};
	EntityInfoView {
		display: entity.display.clone(),
		web: entity.web.clone(),
		email: entity.email.clone(),
		attributes,
	}
}

fn timeline_rows_from_events(events: &[StateEventRecord]) -> Vec<sdk_entity::TimelineRow> {
	let entries: Vec<TokenTimelineEntry> = events
		.iter()
		.enumerate()
		.map(|(idx, event)| TokenTimelineEntry { version: idx as u32, event: event.clone() })
		.collect();
	sdk_entity::build_token_activity(&entries)
}
