use anyhow::{anyhow, Result};
use oc::{
	demo,
	demo::{
		cli::{parse_common_cli, require_value, CommonCliOptions},
		packet::render_packet_snapshot_cli,
		spinner,
		util::{
			ensure_entity_token_verbose, fresh_authorization_with_client, init_logging,
			parse_identifier, resolve_token_target, signer_account_id, token_timeline, LogSink,
			RunMode, TokenTarget, TxExecutor, TxFlow,
		},
	},
	error::Error as SdkError,
	query::register::PacketSnapshotView,
	tx::{self, TxSubmitter},
	utils, ChainFlavor, Client, ConnectionConfig, RetryPolicy,
};
use origin_primitives::{
	registry::RegistryPermissions,
	view_api::{RegisterDetailsRequest, RegisterPacketSnapshotRequest},
};
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
		r#"Usage: packet-demo [OPTIONS]

Transaction mode (default):
  packet-demo
  packet-demo --flow relay

View mode (read-only):
  packet-demo --mode view --token <identifier>

Options:
  -m, --mode <tx|view>         Run mode (default: tx)
  -f, --flow <direct|relay>    Transaction flow when in transaction mode (default: direct)
  -t, --token <identifier>     Auto-detect target token for view mode
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
	let client = connect_client(cli.common.node.as_deref(), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	match cli.common.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client, chain_prefix).await,
		RunMode::View => run_view_flow(&cli, &client).await,
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
	let label = demo::random_label("packet-demo");
	let maintainer = tx::signer::dev_alice();
	let delegate_signer = tx::signer::dev_charlie();
	let maint_account = signer_account_id(&maintainer);
	let delegate_account = signer_account_id(&delegate_signer);
	let delegate_ss58 = utils::format_account(&delegate_account, chain_prefix);

	let mut direct_submitter = TxSubmitter::new(client, &maintainer);
	let relayer_signer =
		if cli.common.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut maint_executor = match cli.common.flow {
		TxFlow::Direct => TxExecutor::Direct { submitter: &mut direct_submitter },
		TxFlow::Relayed => TxExecutor::Relayed {
			relayer: relayer_submitter
				.as_mut()
				.expect("relayer submitter should exist in relayed mode"),
			meta_signer: &maintainer,
		},
	};
	let mut delegate_submitter = TxSubmitter::new(client, &delegate_signer);
	let mut delegate_executor = TxExecutor::Direct { submitter: &mut delegate_submitter };

	let profile = demo::entity_profile(&label);
	let (entity_token_id, created, entity_logs) = ensure_entity_token_verbose(
		client,
		&maintainer,
		&maint_account,
		&profile,
		&mut maint_executor,
	)
	.await?;
	let entity_token = demo::ss58_string(&entity_token_id);
	print_entity_setup(created, &entity_token, &entity_logs);

	let delegate_profile = demo::entity_profile(&format!("{label}-delegate"));
	let (delegate_token_id, delegate_created, delegate_entity_logs) = ensure_entity_token_verbose(
		client,
		&delegate_signer,
		&delegate_account,
		&delegate_profile,
		&mut delegate_executor,
	)
	.await?;
	let delegate_token = demo::ss58_string(&delegate_token_id);
	print_delegate_entity_setup(
		delegate_created,
		&delegate_token,
		&delegate_ss58,
		&delegate_entity_logs,
	);

	let mut registry_logs = Vec::new();
	let registry_spec = demo::registry_blueprint(&label);
	let mut registry_sink = LogSink::new(Some(&mut registry_logs));
	let registry_id = demo::create_registry_with_executor(
		client,
		&mut maint_executor,
		registry_spec,
		&mut registry_sink,
	)
	.await?;
	let registry_ss58 = demo::ss58_string(&registry_id);
	print_registry_header(&entity_token, &registry_ss58);
	for line in registry_logs {
		println!("{line}");
	}

	let mut delegate_logs = Vec::new();
	let mut delegate_sink = LogSink::new(Some(&mut delegate_logs));
	let delegate_call = client
		.tx()
		.register_set_delegate(&registry_ss58, &delegate_ss58, vec![RegistryPermissions::ENTRY])
		.await?;
	maint_executor
		.submit(client, delegate_call, "Set registry delegate", &mut delegate_sink)
		.await
		.map_err(|e| anyhow!(e))?;
	print_delegate_section(&delegate_ss58, &delegate_logs);

	let auth = fresh_authorization_with_client(client, &maintainer).await?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let registry_details = client.query().register().details(&details_req).await?;
	let packet_payload = demo::packet_attributes(&label, &entity_token);

	if cli.common.flow == TxFlow::Relayed {
		println!("ℹ️ Delegate packet submission currently uses direct signing.");
	}
	let mut packet_logs = Vec::new();
	let mut packet_sink = LogSink::new(Some(&mut packet_logs));
	let packet_id = demo::create_packet_with_executor(
		client,
		&mut delegate_executor,
		&registry_ss58,
		packet_payload,
		&registry_details,
		&mut packet_sink,
	)
	.await?;
	let packet_ss58 = demo::ss58_string(&packet_id);
	print_packet_header(&packet_ss58, &packet_logs);

	let maint_auth = fresh_authorization_with_client(client, &maintainer).await?;
	let snapshot_req = RegisterPacketSnapshotRequest {
		auth: maint_auth.clone(),
		registry: registry_id.clone(),
		packet: packet_id.clone(),
		version: None,
	};
	let snapshot = fetch_packet_snapshot_with_retry(client, &snapshot_req, "transaction").await?;
	let (timeline, next_cursor) = token_timeline(client, &maint_auth, &packet_id, Some(12)).await?;
	render_packet_snapshot_cli(
		&registry_ss58,
		&packet_ss58,
		Some(&delegate_ss58),
		&snapshot,
		&timeline,
		next_cursor,
		cli.common.view,
		cli.common.output_json,
	);
	Ok(())
}

async fn run_view_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let token_str = cli
		.token
		.as_deref()
		.ok_or_else(|| anyhow!("--token <identifier> is required in view mode"))?;
	let signer = tx::signer::dev_alice();
	let auth = fresh_authorization_with_client(client, &signer).await?;
	let token_id = parse_identifier(token_str)?;
	match resolve_token_target(client, &auth, &token_id).await? {
		TokenTarget::Packet { registry, packet, snapshot } => {
			let registry_ss58 = demo::ss58_string(&registry);
			let packet_ss58 = demo::ss58_string(&packet);
			let (timeline, next_cursor) =
				token_timeline(client, &auth, &packet, Some(12)).await?;
			render_packet_snapshot_cli(
				&registry_ss58,
				&packet_ss58,
				None,
				&snapshot,
				&timeline,
				next_cursor,
				cli.common.view,
				cli.common.output_json,
			);
			Ok(())
		},
		TokenTarget::Registry { .. } => Err(anyhow!(format!(
			"token {token_str} is a registry; run register-demo --mode view --token {token_str} to inspect it"
		))),
		TokenTarget::Entity { .. } => Err(anyhow!(format!(
			"token {token_str} is an entity; run entity-demo --mode view --token {token_str} to inspect it"
		))),
	}
}

fn print_entity_setup(created: bool, entity_token: &str, logs: &[String]) {
	println!("\n🚚 Packet Demo Prerequisites\n");
	println!("👤 Maintainer Entity");
	println!("  ↳ • Token : {entity_token}");
	println!(
		"  ↳ • Action: {}",
		if created { "Created new entity" } else { "Reused existing entity" }
	);
	if !logs.is_empty() {
		println!("\n  Setup logs:");
		for line in logs {
			println!("    {line}");
		}
	}
}

fn print_registry_header(entity_token: &str, registry_ss58: &str) {
	println!("\n📘 Registry Setup");
	println!("  ↳ • Maintainer : {entity_token}");
	println!("  ↳ • Registry   : {registry_ss58}");
}

fn print_delegate_section(delegate_ss58: &str, logs: &[String]) {
	println!("\n🧾 Delegate Assignment");
	println!("  ↳ • Delegate : {delegate_ss58}");
	if logs.is_empty() {
		println!("  ↳ • Action  : grant entry role");
	} else {
		for line in logs {
			println!("    {line}");
		}
	}
}

fn print_delegate_entity_setup(
	created: bool,
	delegate_token: &str,
	delegate_account: &str,
	logs: &[String],
) {
	println!("\n🧑‍🤝‍🧑 Delegate Entity");
	println!("  ↳ • Account : {delegate_account}");
	println!("  ↳ • Token   : {delegate_token}");
	println!(
		"  ↳ • Action  : {}",
		if created { "Created new entity" } else { "Reused existing entity" }
	);
	if !logs.is_empty() {
		println!("\n  Delegate logs:");
		for line in logs {
			println!("    {line}");
		}
	}
}

fn print_packet_header(packet_ss58: &str, logs: &[String]) {
	println!("\n📦 Packet Issue");
	println!("  ↳ • Packet : {packet_ss58}");
	if logs.is_empty() {
		println!("  ↳ • Action : submit attributes");
	} else {
		for line in logs {
			println!("    {line}");
		}
	}
}

async fn fetch_packet_snapshot_with_retry(
	client: &Client,
	req: &RegisterPacketSnapshotRequest,
	label: &str,
) -> Result<PacketSnapshotView> {
	const MAX_ATTEMPTS: usize = 5;
	for attempt in 0..MAX_ATTEMPTS {
		match client.query().register().packet_snapshot(req).await {
			Ok(snapshot) => return Ok(snapshot),
			Err(SdkError::NotFound(_msg)) if attempt + 1 < MAX_ATTEMPTS => {
				let wait_label = format!(
					"⏱️ waiting for packet snapshot ({} attempt {}/{})",
					label,
					attempt + 1,
					MAX_ATTEMPTS
				);
				spinner::with_spinner(wait_label, tokio::time::sleep(Duration::from_millis(800)))
					.await;
			},
			Err(SdkError::NotFound(msg)) => return Err(anyhow!(msg)),
			Err(other) => return Err(anyhow!(other)),
		}
	}
	Err(anyhow!("packet snapshot unavailable after retries"))
}
