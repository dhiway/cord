use anyhow::{anyhow, Result};
use bs58;
	registry::{LookupSpecView, RegistryAttributeView, RegistryInfoView},
	view::ElementView,
	view_api::{RegisterDetailsRequest, RegisterLookupSpecsRequest},
};
use hex;
use oc::{
	demo,
	demo::util::{
		ensure_entity_token_verbose, fresh_authorization, init_logging, parse_identifier,
		signer_account_id, LogSink, RunMode, TxExecutor, TxFlow, ViewStyle,
	},
	tx::{self, TxSubmitter},
	utils, ChainFlavor, Client,
};
use serde_json::json;
use std::time::Duration;

struct CliOptions {
	view: ViewStyle,
	output_json: bool,
	node: Option<String>,
	mode: RunMode,
	flow: TxFlow,
	registry: Option<String>,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	let mut style: Option<ViewStyle> = None;
	let mut json = false;
	let mut node: Option<String> = None;
	let mut mode = RunMode::Transaction;
	let mut flow = TxFlow::Direct;
	let mut registry: Option<String> = None;
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
				flow = flow_from_value(&value)
					.ok_or_else(|| anyhow!("invalid --flow value: {value} (expected direct|relay)"))?;
			},
			_ if arg.starts_with("--flow=") => {
				let value = arg.trim_start_matches("--flow=");
				flow = flow_from_value(value)
					.ok_or_else(|| anyhow!("invalid --flow value: {value} (expected direct|relay)"))?;
			},
			"--registry" | "-r" => {
				registry = Some(require_value(&mut iter, arg.as_str())?);
			},
			_ if arg.starts_with("--registry=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-r=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			"--" => break,
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
	if mode == RunMode::View && registry.is_none() {
		return Err(anyhow!("--registry <identifier> is required in view mode"));
	}
	Ok(CliOptions { view: resolved_style, output_json: json, node, mode, flow, registry })
}

fn require_value<'a>(
	iter: &mut std::iter::Peekable<std::slice::Iter<'a, String>>,
	flag: &str,
) -> Result<String> {
	iter
		.next()
		.map(|value| value.clone())
		.ok_or_else(|| anyhow!("{flag} expects a value"))
}

fn parse_display_style(value: &str) -> Result<ViewStyle> {
	style_from_value(value)
		.ok_or_else(|| anyhow!("invalid display style '{value}' (expected less|more/full)"))
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

fn print_usage() {
	println!(
		r#"Usage: register-demo [OPTIONS]

Transaction mode (default):
  register-demo
  register-demo --flow relay

View mode (read-only):
  register-demo --mode view --registry <identifier>

Options:
  -m, --mode <tx|view>         Run mode (default: tx)
  -f, --flow <direct|relay>    Transaction flow when in transaction mode (default: direct)
  -r, --registry <identifier>  Target registry for view mode
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
	let client = utils::connect_or_default(cli.node.as_deref(), ChainFlavor::Auto).await?;
	match cli.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client).await,
		RunMode::View => run_view_flow(&cli, &client).await,
	}
}

async fn run_transaction_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let label = demo::random_label("register-demo");
	let signer = tx::signer::dev_alice();
	let account_id = signer_account_id(&signer);

	let mut direct_submitter = TxSubmitter::new(client, &signer);
	let mut relayer_signer =
		if cli.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut tx_executor = match cli.flow {
		TxFlow::Direct => TxExecutor::Direct { submitter: &mut direct_submitter },
		TxFlow::Relayed => TxExecutor::Relayed {
			relayer: relayer_submitter
				.as_mut()
				.expect("relayer submitter should exist in relayed mode"),
			meta_signer: &signer,
		},
	};

	let profile = demo::entity_profile(&label);
	let (entity_token, created, mut entity_logs) =
		ensure_entity_token_verbose(client, &signer, &account_id, &profile, &mut tx_executor).await?;
	print_entity_setup(created, &entity_token, &entity_logs);

	let mut registry_logs = Vec::new();
	let registry_spec = demo::registry_blueprint(&label);
	let mut sink = LogSink::new(Some(&mut registry_logs));
	let registry_id =
		demo::create_registry_with_executor(client, &mut tx_executor, registry_spec, &mut sink)?;
	let registry_ss58 = demo::ss58_string(&registry_id);
	print_registry_header(&entity_token, &registry_ss58);
	for line in registry_logs {
		println!("{line}");
	}

	utils::short_delay(Duration::from_secs(3)).await;

	let auth = fresh_authorization(&signer)?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let details = client.query().register().details(&details_req).await?;
	let lookup_req = RegisterLookupSpecsRequest { auth, registry: registry_id.clone() };
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	render_registry_snapshot(&registry_ss58, &details, &lookups, cli.view, cli.output_json)
}

async fn run_view_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let Some(registry_str) = cli.registry.as_deref() else {
		return Err(anyhow!("--registry <identifier> is required in view mode"));
	};
	let registry_id = parse_identifier(registry_str)?;
	let signer = tx::signer::dev_alice();
	let auth = fresh_authorization(&signer)?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let details = client.query().register().details(&details_req).await?;
	let lookup_req = RegisterLookupSpecsRequest { auth, registry: registry_id.clone() };
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	render_registry_snapshot(registry_str, &details, &lookups, cli.view, cli.output_json)
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

fn render_registry_snapshot(
	registry_ss58: &str,
	details: &RegistryInfoView,
	lookups: &[LookupSpecView],
	style: ViewStyle,
	output_json: bool,
) -> Result<()> {
	if output_json {
		let json = json!({
			"registry": registry_ss58,
			"maintainer": String::from_utf8_lossy(&details.maintainer),
			"details": details,
			"lookupSpecs": lookups,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!("\n🗂️ Registry Snapshot");
	let maintainer = String::from_utf8_lossy(&details.maintainer);
	println!("  ↳ • Registry   : {registry_ss58}");
	println!("  ↳ • Maintainer : {maintainer}");
	println!("  ↳ • Kind       : {:?}", details.kind);
	println!("  ↳ • Status     : {:?}", details.status);
	println!("\n📝 Info\n  {}", describe_element(&details.info));
	println!("\n🔑 Token Spec\n  {}", describe_lookup(&details.token_spec));
	print_attribute_schema(&details.attributes, style.is_full());
	print_lookup_specs(lookups, style.is_full());
	Ok(())
}

fn print_attribute_schema(attributes: &[RegistryAttributeView], full: bool) {
	println!("\n🔣 Attribute Schema");
	if attributes.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { attributes.len() } else { attributes.len().min(8) };
	for attr in attributes.iter().take(limit) {
		let key = key_to_label(&attr.key);
		println!(
			"  ↳ • {:<16} kind={:?}{}",
			key,
			attr.kind,
			if attr.optional { " [optional]" } else { "" }
		);
		shown += 1;
	}
	if !full && attributes.len() > shown {
		println!("    … {} more", attributes.len() - shown);
	}
}

fn print_lookup_specs(specs: &[LookupSpecView], full: bool) {
	println!("\n🔍 Lookup Specs");
	if specs.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { specs.len() } else { specs.len().min(5) };
	for spec in specs.iter().take(limit) {
		println!("  ↳ • {}", describe_lookup(spec));
		shown += 1;
	}
	if !full && specs.len() > shown {
		println!("    … {} more", specs.len() - shown);
	}
}

fn describe_lookup(spec: &LookupSpecView) -> String {
	match spec {
		LookupSpecView::Single(key) => format!("single:{}", key_to_label(key)),
		LookupSpecView::Combo(keys) => {
			let joined = keys.iter().map(|k| key_to_label(k)).collect::<Vec<_>>().join(", ");
			format!("combo:[{}]", joined)
		},
	}
}

fn describe_element(view: &ElementView) -> String {
	match view {
		ElementView::None => "(none)".into(),
		ElementView::Bool(value) => format!("bool:{value}"),
		ElementView::U64(value) => format!("u64:{value}"),
		ElementView::U128(value) => format!("u128:{value}"),
		ElementView::Hash(bytes) => format!("hash:0x{}", hex::encode(bytes)),
		ElementView::Token(token) => format!("token:{}", demo::ss58_string(token)),
		ElementView::Cid(bytes) => format!("cid:{}", bs58::encode(bytes).into_string()),
		ElementView::Raw(bytes) => {
			let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)));
			format!("raw:{text}")
		},
	}
}

fn key_to_label(bytes: &[u8]) -> String {
	String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)))
}
