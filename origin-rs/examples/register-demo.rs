use anyhow::{anyhow, Result};
use oc::{
	demo,
	demo::{
		cli::{parse_common_cli, require_value, CommonCliOptions},
		packet::render_packet_snapshot_cli,
		register::render_registry_snapshot_cli,
		spinner,
		util::{
			ensure_entity_token_verbose, fresh_authorization_with_client, init_logging,
			parse_identifier, resolve_token_target, signer_account_id, token_timeline, LogSink,
			RunMode, TokenTarget, TxExecutor, TxFlow,
		},
	},
	error::Error as SdkError,
	tx::{self, TxSubmitter},
	utils, ChainFlavor, Client, ConnectionConfig, RetryPolicy,
};
use origin_primitives::{
	identifier::Ss58Identifier,
	registry::{LookupSpecView, RegistryInfoView},
	view_api::{RegisterDetailsRequest, RegisterLookupSpecsRequest},
};
use std::time::Duration;

struct CliOptions {
	common: CommonCliOptions,
	registry: Option<String>,
	token: Option<String>,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
		print_usage();
		std::process::exit(0);
	}
	let parsed = parse_common_cli(args)?;
	let mut registry: Option<String> = None;
	let mut token: Option<String> = None;
	let mut iter = parsed.rest.iter().peekable();
	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--registry" | "-r" => registry = Some(require_value(&mut iter, arg)?),
			_ if arg.starts_with("--registry=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-r=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
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
	if parsed.common.mode == RunMode::View && registry.is_none() && token.is_none() {
		return Err(anyhow!(
			"--token <identifier> (or --registry for legacy mode) is required in view mode"
		));
	}
	Ok(CliOptions { common: parsed.common, registry, token })
}

fn print_usage() {
	println!(
		r#"Usage: register-demo [OPTIONS]

Transaction mode (default):
  register-demo
  register-demo --flow relay

View mode (read-only):
  register-demo --mode view --token <identifier>
  register-demo --mode view --registry <identifier>  # legacy fallback

Options:
  -m, --mode <tx|view>         Run mode (default: tx)
  -f, --flow <direct|relay>    Transaction flow when in transaction mode (default: direct)
  -r, --registry <identifier>  Target registry token for transaction/view mode
      --token <identifier>     Auto-detect target token (preferred for view mode)
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
	match cli.common.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client).await,
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

async fn run_transaction_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let label = demo::random_label("register-demo");
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);

	let mut direct_submitter = TxSubmitter::new(client, &signer);
	let relayer_signer =
		if cli.common.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut tx_executor = match cli.common.flow {
		TxFlow::Direct => TxExecutor::Direct { submitter: &mut direct_submitter },
		TxFlow::Relayed => TxExecutor::Relayed {
			relayer: relayer_submitter
				.as_mut()
				.expect("relayer submitter should exist in relayed mode"),
			meta_signer: &signer,
		},
	};

	let profile = demo::entity_profile(&label);
	let (entity_token_id, created, entity_logs) =
		ensure_entity_token_verbose(client, &signer, &account_id, &profile, &mut tx_executor)
			.await?;
	let entity_token = demo::ss58_string(&entity_token_id);
	print_entity_setup(created, &entity_token, &entity_logs);

	let mut registry_logs = Vec::new();
	let registry_spec = demo::registry_blueprint(&label);
	let mut sink = LogSink::new(Some(&mut registry_logs));
	let registry_id =
		demo::create_registry_with_executor(client, &mut tx_executor, registry_spec, &mut sink)
			.await?;
	let registry_ss58 = demo::ss58_string(&registry_id);
	print_registry_header(&entity_token, &registry_ss58);
	for line in registry_logs {
		println!("{line}");
	}

	let (details, lookups) =
		fetch_registry_snapshot_with_retry(client, &signer, &registry_id, "transaction").await?;
	let auth = fresh_authorization_with_client(client, &signer).await?;
	let (timeline, next_cursor) = token_timeline(client, &auth, &registry_id, Some(12)).await?;

	render_registry_snapshot_cli(
		&registry_ss58,
		&details,
		&lookups,
		&timeline,
		next_cursor,
		cli.common.view,
		cli.common.output_json,
	)
}

async fn fetch_registry_snapshot_with_retry(
	client: &Client,
	signer: &tx::signer::Keypair,
	registry: &Ss58Identifier,
	label: &str,
) -> Result<(RegistryInfoView, Vec<LookupSpecView>)> {
	const MAX_ATTEMPTS: usize = 5;
	for attempt in 0..MAX_ATTEMPTS {
		let auth = fresh_authorization_with_client(client, signer).await?;
		let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry.clone() };
		match client.query().register().details(&details_req).await {
			Ok(details) => {
				let lookup_req = RegisterLookupSpecsRequest { auth, registry: registry.clone() };
				let lookups = client.query().register().lookup_specs(&lookup_req).await?;
				return Ok((details, lookups));
			},
			Err(SdkError::NotFound(_)) if attempt + 1 < MAX_ATTEMPTS => {
				let wait_label = format!(
					"⏱️ waiting for registry details ({} attempt {}/{})",
					label,
					attempt + 1,
					MAX_ATTEMPTS
				);
				spinner::with_spinner(wait_label, tokio::time::sleep(Duration::from_millis(800)))
					.await;
			},
			Err(err) => return Err(err.into()),
		}
	}
	Err(anyhow!("registry details unavailable after retries"))
}

async fn run_view_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let signer = tx::signer::dev_alice();
	let auth = fresh_authorization_with_client(client, &signer).await?;
	if let Some(token_str) = cli.token.as_deref() {
		let token_id = parse_identifier(token_str)?;
		match resolve_token_target(client, &auth, &token_id).await? {
			TokenTarget::Registry { registry, info } => {
				let registry_ss58 = demo::ss58_string(&registry);
				let lookup_req =
					RegisterLookupSpecsRequest { auth: auth.clone(), registry: registry.clone() };
				let lookups = client.query().register().lookup_specs(&lookup_req).await?;
				let (timeline, next_cursor) =
					token_timeline(client, &auth, &registry, Some(12)).await?;
				return render_registry_snapshot_cli(
					&registry_ss58,
					&info,
					&lookups,
					&timeline,
					next_cursor,
					cli.common.view,
					cli.common.output_json,
				);
			},
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
				return Ok(());
			},
			TokenTarget::Entity { .. } => {
				return Err(anyhow!(format!(
					"token {token_str} is an entity profile; run entity-demo --mode view --token {token_str} to inspect it"
				)));
			},
		}
	}

	let registry_str = cli
		.registry
		.as_deref()
		.ok_or_else(|| anyhow!("--token or --registry is required in view mode"))?;
	let registry_id = parse_identifier(registry_str)?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let details = client.query().register().details(&details_req).await?;
	let lookup_req =
		RegisterLookupSpecsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	let (timeline, next_cursor) = token_timeline(client, &auth, &registry_id, Some(12)).await?;
	render_registry_snapshot_cli(
		registry_str,
		&details,
		&lookups,
		&timeline,
		next_cursor,
		cli.common.view,
		cli.common.output_json,
	)
}

fn print_entity_setup(created: bool, entity_token: &str, logs: &[String]) {
	println!("\n🏷️ Origin Registry Demo\n");
	println!("👤 Maintainer Entity");
	println!("  ↳ • Token : {entity_token}");
	if created {
		println!("  ↳ • Action: Created new entity profile");
	} else {
		println!("  ↳ • Action: Reusing existing entity profile");
	}
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
