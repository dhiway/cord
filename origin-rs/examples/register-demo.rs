use anyhow::{anyhow, Result};
use bs58;
use cord_primitives::{
	packet::ElementType,
	registry::{
		LookupSpecView, RegistryAttributeView, RegistryInfoView, RegistryKind, RegistryStatus,
	},
	view::ElementView,
	view_api::{RegisterDetailsRequest, RegisterLookupSpecsRequest},
};
use hex;
use oc::demo::packet::render_packet_snapshot_cli;
use oc::types::token::StateEventRecord;
use oc::{
	demo,
	demo::cli::{parse_common_cli, require_value, CommonCliOptions},
	demo::util::{
		ensure_entity_token_verbose, fresh_authorization, init_logging, parse_identifier,
		resolve_token_target, signer_account_id, token_timeline, LogSink, RunMode, TokenTarget,
		TxExecutor, TxFlow, ViewStyle,
	},
	tx::{self, TxSubmitter},
	utils, ChainFlavor, Client,
};
use serde_json::json;
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
	let client = utils::connect_or_default(cli.common.node.as_deref(), ChainFlavor::Auto).await?;
	match cli.common.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client).await,
		RunMode::View => run_view_flow(&cli, &client).await,
	}
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

	utils::short_delay(Duration::from_secs(6)).await;

	let auth = fresh_authorization(&signer)?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let details = client.query().register().details(&details_req).await?;
	let lookup_req =
		RegisterLookupSpecsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	let (timeline, next_cursor) = token_timeline(client, &auth, &registry_id, Some(12)).await?;

	render_registry_snapshot(
		&registry_ss58,
		&details,
		&lookups,
		&timeline,
		next_cursor,
		cli.common.view,
		cli.common.output_json,
	)
}

async fn run_view_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let signer = tx::signer::dev_alice();
	let auth = fresh_authorization(&signer)?;
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
				return render_registry_snapshot(
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
	render_registry_snapshot(
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

fn render_registry_snapshot(
	registry_ss58: &str,
	details: &RegistryInfoView,
	lookups: &[LookupSpecView],
	timeline: &[StateEventRecord],
	next_cursor: Option<u32>,
	style: ViewStyle,
	output_json: bool,
) -> Result<()> {
	if output_json {
		let json = json!({
			"registry": registry_ss58,
			"maintainer": demo::ss58_string(&details.maintainer),
			"details": details,
			"lookupSpecs": lookups,
			"timeline": timeline,
			"nextCursor": next_cursor,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!("\n🗂️ Registry Snapshot");
	let maintainer = demo::ss58_string(&details.maintainer);
	println!("  ↳ • registry   : {registry_ss58}");
	println!("  ↳ • maintainer : {maintainer}");
	println!("  ↳ • kind       : {}", describe_registry_kind(&details.kind));
	println!("  ↳ • status     : {}", describe_registry_status(&details.status));
	println!("\n📝 Info\n  {}", describe_element(&details.info));
	println!("\n🔑 Token Spec\n  {}", describe_lookup(&details.token_spec));
	print_attribute_schema(&details.attributes, style.is_full());
	print_lookup_specs(lookups, style.is_full());
	print_registry_timeline(timeline, next_cursor, style.is_full());
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
			"  ↳ • {:<16} {}{}",
			key,
			describe_element_type(&attr.kind),
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

fn print_registry_timeline(records: &[StateEventRecord], next_cursor: Option<u32>, full: bool) {
	println!("\n⏱️ Token Timeline");
	if records.is_empty() {
		println!("  ↳ • (no events)");
	} else {
		let limit = if full { records.len() } else { records.len().min(8) };
		for (idx, event) in records.iter().take(limit).enumerate() {
			let action_utf8 = String::from_utf8(event.action.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(&event.action)));
			println!(
				"  ↳ • #{:<2} action={:<24} block=#{} extrinsic={} digest=0x{}",
				idx + 1,
				action_utf8,
				event.seal.height,
				event.seal.index,
				hex::encode(event.digest)
			);
		}
		if !full && records.len() > limit {
			println!("    … {} more", records.len() - limit);
		}
	}
	if let Some(cursor) = next_cursor {
		println!("  ↳ • next cursor: {cursor}");
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
			let text = String::from_utf8(bytes.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)));
			format!("raw:{text}")
		},
	}
}

fn key_to_label(bytes: &[u8]) -> String {
	String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)))
}

fn describe_registry_kind(kind: &RegistryKind) -> &'static str {
	match kind {
		RegistryKind::Raw => "Raw",
		RegistryKind::Token => "Token",
		RegistryKind::Hash => "Hash",
	}
}

fn describe_registry_status(status: &RegistryStatus) -> &'static str {
	match status {
		RegistryStatus::Active => "Active",
		RegistryStatus::Revoked => "Revoked",
		RegistryStatus::Deleted => "Deleted",
	}
}

fn describe_element_type(kind: &ElementType) -> &'static str {
	match kind {
		ElementType::None => "None",
		ElementType::Raw => "Raw",
		ElementType::Bool => "Bool",
		ElementType::U64 => "U64",
		ElementType::U128 => "U128",
		ElementType::Hash => "Hash",
		ElementType::Token => "Token",
		ElementType::Cid => "Cid",
	}
}
