use anyhow::{anyhow, Result};
use cord_primitives::{
	packet::PacketStatus,
	registry::{RegistryPermissions, RegistryStatus},
	view::{DevAttr, DevElement},
	view_api::{RegisterDetailsRequest, RegisterPacketSnapshotRequest, TokenTimelineRequest},
};
use hex;
use oc::{
	demo,
	demo::util::{
		ensure_entity_token_verbose, fresh_authorization, init_logging, parse_identifier,
		signer_account_id, LogSink, RunMode, TxExecutor, TxFlow, ViewStyle,
	},
	error::Error as SdkError,
	query::register::PacketSnapshotView,
	tx::{self, TxSubmitter},
	types::token::StateEventRecord,
	utils, ChainFlavor, Client,
};
use serde_json::json;
use sp_core::crypto::Ss58AddressFormat;
use std::time::Duration;

struct CliOptions {
	view: ViewStyle,
	output_json: bool,
	node: Option<String>,
	mode: RunMode,
	flow: TxFlow,
	registry: Option<String>,
	packet: Option<String>,
}

fn parse_args(args: &[String]) -> Result<CliOptions> {
	let mut style: Option<ViewStyle> = None;
	let mut json = false;
	let mut node: Option<String> = None;
	let mut mode = RunMode::Transaction;
	let mut flow = TxFlow::Direct;
	let mut registry: Option<String> = None;
	let mut packet: Option<String> = None;
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
			"--node" | "-n" => node = Some(require_value(&mut iter, arg.as_str())?),
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
			"--registry" | "-r" => registry = Some(require_value(&mut iter, arg.as_str())?),
			_ if arg.starts_with("--registry=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-r=") => {
				registry = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			"--packet" | "-p" => packet = Some(require_value(&mut iter, arg.as_str())?),
			_ if arg.starts_with("--packet=") => {
				packet = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-p=") => {
				packet = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
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
	if mode == RunMode::View && (registry.is_none() || packet.is_none()) {
		return Err(anyhow!("--registry and --packet are required in view mode"));
	}
	Ok(CliOptions { view: resolved_style, output_json: json, node, mode, flow, registry, packet })
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
		r#"Usage: packet-demo [OPTIONS]

Transaction mode (default):
  packet-demo
  packet-demo --flow relay

View mode (read-only):
  packet-demo --mode view --registry <identifier> --packet <identifier>

Options:
  -m, --mode <tx|view>         Run mode (default: tx)
  -f, --flow <direct|relay>    Transaction flow when in transaction mode (default: direct)
  -r, --registry <identifier>  Target registry for view mode
  -p, --packet <identifier>    Target packet for view mode
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
	let chain_prefix = client.chain_prefix().await;
	match cli.mode {
		RunMode::Transaction => run_transaction_flow(&cli, &client, chain_prefix).await,
		RunMode::View => run_view_flow(&cli, &client).await,
	}
}

async fn run_transaction_flow(
	cli: &CliOptions,
	client: &Client,
	chain_prefix: Ss58AddressFormat,
) -> Result<()> {
	let label = demo::random_label("packet-demo");
	let maintainer = tx::signer::dev_alice();
	let delegate_signer = tx::signer::dev_bob();
	let maint_account = signer_account_id(&maintainer);
	let delegate_account = signer_account_id(&delegate_signer);
	let delegate_ss58 = utils::format_account(&delegate_account, chain_prefix);

	let mut direct_submitter = TxSubmitter::new(client, &maintainer);
	let relayer_signer =
		if cli.flow == TxFlow::Relayed { Some(tx::signer::dev_bob()) } else { None };
	let mut relayer_submitter =
		relayer_signer.as_ref().map(|relayer| TxSubmitter::new(client, relayer));
	let mut maint_executor = match cli.flow {
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
	utils::short_delay(Duration::from_secs(10)).await;

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

	let auth = fresh_authorization(&maintainer)?;
	let details_req = RegisterDetailsRequest { auth: auth.clone(), registry: registry_id.clone() };
	let registry_details = client.query().register().details(&details_req).await?;
	let packet_payload = demo::packet_attributes(&label, &entity_token);

	if cli.flow == TxFlow::Relayed {
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

	utils::short_delay(Duration::from_secs(3)).await;

	let snapshot_req = RegisterPacketSnapshotRequest {
		auth: fresh_authorization(&maintainer)?,
		registry: registry_id.clone(),
		packet: packet_id.clone(),
		version: None,
	};
	let snapshot = fetch_packet_snapshot_with_retry(client, &snapshot_req, "transaction").await?;
	let timeline_req = TokenTimelineRequest {
		auth: fresh_authorization(&maintainer)?,
		token: packet_id.clone(),
		start: None,
		limit: Some(12),
	};
	let (timeline, next_cursor) = client.query().token().timeline(&timeline_req).await?;
	render_packet_snapshot(
		&registry_ss58,
		&packet_ss58,
		&delegate_ss58,
		&snapshot,
		&timeline,
		next_cursor,
		cli.view,
		cli.output_json,
	)
}

async fn run_view_flow(cli: &CliOptions, client: &Client) -> Result<()> {
	let registry_str = cli.registry.as_deref().ok_or_else(|| anyhow!("--registry is required"))?;
	let packet_str = cli.packet.as_deref().ok_or_else(|| anyhow!("--packet is required"))?;
	let registry_id = parse_identifier(registry_str)?;
	let packet_id = parse_identifier(packet_str)?;
	let signer = tx::signer::dev_alice();
	let snapshot_req = RegisterPacketSnapshotRequest {
		auth: fresh_authorization(&signer)?,
		registry: registry_id,
		packet: packet_id.clone(),
		version: None,
	};
	let snapshot = fetch_packet_snapshot_with_retry(client, &snapshot_req, "view").await?;
	let timeline_req = TokenTimelineRequest {
		auth: fresh_authorization(&signer)?,
		token: packet_id.clone(),
		start: None,
		limit: Some(12),
	};
	let (timeline, next_cursor) = client.query().token().timeline(&timeline_req).await?;
	render_packet_snapshot(
		registry_str,
		packet_str,
		"(delegate not resolved)",
		&snapshot,
		&timeline,
		next_cursor,
		cli.view,
		cli.output_json,
	)
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
				println!(
					"⏱️ waiting for packet snapshot ({} attempt {}/{})",
					label,
					attempt + 1,
					MAX_ATTEMPTS
				);
				utils::short_delay(Duration::from_secs(2)).await;
			},
			Err(SdkError::NotFound(msg)) => return Err(anyhow!(msg)),
			Err(other) => return Err(anyhow!(other)),
		}
	}
	Err(anyhow!("packet snapshot unavailable after retries"))
}

fn render_packet_snapshot(
	registry_ss58: &str,
	packet_ss58: &str,
	delegate_ss58: &str,
	snapshot: &PacketSnapshotView,
	timeline: &[StateEventRecord],
	next_cursor: Option<u32>,
	style: ViewStyle,
	output_json: bool,
) -> Result<()> {
	if output_json {
		let json = json!({
			"registry": registry_ss58,
			"packet": packet_ss58,
			"delegate": delegate_ss58,
			"snapshot": snapshot,
			"timeline": timeline,
			"nextCursor": next_cursor,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!("\n🧾 Packet Snapshot");
	println!("  ↳ • Packet     : {packet_ss58}");
	println!("  ↳ • Registry   : {registry_ss58}");
	println!("  ↳ • Delegate   : {delegate_ss58}");
	println!("  ↳ • Controller : {}", snapshot.state.controller_ss58);
	println!(
		"  ↳ • Status     : {} (registry {})",
		describe_packet_status(&snapshot.state.status),
		describe_registry_status(&snapshot.registry_status)
	);
	println!("  ↳ • Version    : {}", snapshot.state.version);
	println!("  ↳ • Attr Hash  : {}", snapshot.state.attributes_hash_hex);
	print_packet_attributes(&snapshot.state.attributes, style.is_full());
	print_timeline(timeline, next_cursor, style.is_full());
	Ok(())
}

fn print_packet_attributes(attrs: &[DevAttr], full: bool) {
	println!("\n🧬 Packet Attributes");
	if attrs.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { attrs.len() } else { attrs.len().min(6) };
	for attr in attrs.iter().take(limit) {
		let key = attr.key_utf8.clone().unwrap_or_else(|| attr.key_hex.clone());
		println!("  ↳ • {:<18} : {}", key, describe_dev_element(&attr.value));
		shown += 1;
	}
	if !full && attrs.len() > shown {
		println!("    … {} more", attrs.len() - shown);
	}
}

fn describe_dev_element(value: &DevElement) -> String {
	match value {
		DevElement::None => "(none)".into(),
		DevElement::Bool(flag) => format!("bool:{flag}"),
		DevElement::U64(v) => format!("u64:{v}"),
		DevElement::U128(v) => format!("u128:{v}"),
		DevElement::HashHex(hex_str) => format!("hash:{hex_str}"),
		DevElement::TokenSs58(token) => format!("token:{token}"),
		DevElement::CidBase58(cid) => format!("cid:{cid}"),
		DevElement::RawBase64(data) => format!("raw(base64):{data}"),
	}
}

fn describe_packet_status(status: &PacketStatus) -> &'static str {
	match status {
		PacketStatus::Active => "Active",
		PacketStatus::Revoked => "Revoked",
		PacketStatus::Deleted => "Deleted",
	}
}

fn describe_registry_status(status: &RegistryStatus) -> &'static str {
	match status {
		RegistryStatus::Active => "Active",
		RegistryStatus::Revoked => "Revoked",
		RegistryStatus::Deleted => "Deleted",
	}
}

fn print_timeline(records: &[StateEventRecord], next_cursor: Option<u32>, full: bool) {
	println!("\n⏱️ Token Timeline");
	if records.is_empty() {
		println!("  ↳ • (no events)");
	} else {
		let limit = if full { records.len() } else { records.len().min(8) };
		for (idx, event) in records.iter().take(limit).enumerate() {
			let action_utf8 = String::from_utf8(event.action.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(&event.action)));
			let digest = format!("0x{}", hex::encode(event.digest));
			println!(
				"  ↳ • #{:<2} action={:<24} block=#{} extrinsic={} digest={}",
				idx + 1,
				action_utf8,
				event.seal.height,
				event.seal.index,
				digest
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
