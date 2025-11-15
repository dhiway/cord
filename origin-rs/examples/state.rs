use anyhow::{anyhow, Result};
use oc::{
	demo,
	demo::{
		cli::{parse_common_cli, require_value, CommonCliOptions},
		entity::{self, EntitySnapshot},
		packet::render_packet_snapshot_cli,
		register::render_registry_snapshot_cli,
		util::{
			fresh_authorization_with_client, init_logging, parse_identifier, resolve_token_target,
			token_timeline, RunMode, TokenTarget,
		},
	},
	entity::EntityChainState,
	query::register::PacketSnapshotView,
	tx, utils, ChainFlavor, Client,
};
use origin_primitives::{
	identifier::Ss58Identifier,
	registry::RegistryInfoView,
	view_api::{EntityNymRequest, RegisterLookupSpecsRequest},
};
use sp_core::crypto::Ss58AddressFormat;

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
	if token.is_none() {
		return Err(anyhow!("--token <identifier> is required"));
	}
	Ok(CliOptions { common: parsed.common, token })
}

fn print_usage() {
	println!(
		r#"Usage: state [OPTIONS]

Unified token viewer. Resolves the identifier to entity/registry/packet and prints
its latest state using the SDK renderers.

Examples:
  cargo run -p origin-rs --example state -- --token <identifier>
  cargo run -p origin-rs --example state -- --token <identifier> --json

Options:
  -t, --token <identifier>     Target identifier (required)
  -m, --mode <tx|view>         Only 'view' is supported (default: view)
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
	if cli.common.mode == RunMode::Transaction {
		return Err(anyhow!("state example only supports --mode view"));
	}

	let client = utils::connect_or_default(cli.common.node.as_deref(), ChainFlavor::Auto).await?;
	let chain_prefix = client.chain_prefix().await;
	let signer = tx::signer::dev_alice();
	let token_str = cli.token.as_deref().expect("token required");
	let token_id = parse_identifier(token_str)?;
	let auth = fresh_authorization_with_client(&client, &signer).await?;
	let target = resolve_token_target(&client, &auth, &token_id).await?;

	match target {
		TokenTarget::Entity { token } =>
			render_entity_state(&client, &signer, &token, token_str, chain_prefix, &cli.common)
				.await,
		TokenTarget::Registry { registry, info } =>
			render_registry_state(&client, &signer, registry, &info, &cli.common).await,
		TokenTarget::Packet { registry, packet, snapshot } =>
			render_packet_state(&client, &signer, registry, packet, snapshot, &cli.common).await,
	}
}

async fn render_entity_state(
	client: &Client,
	signer: &tx::signer::Keypair,
	token: &Ss58Identifier,
	token_str: &str,
	chain_prefix: Ss58AddressFormat,
	common: &CommonCliOptions,
) -> Result<()> {
	let auth = fresh_authorization_with_client(client, signer).await?;
	let entity_info = client
		.query()
		.entity()
		.details(&auth, token)
		.await?
		.ok_or_else(|| anyhow!(format!("no entity info found for token {token_str}")))?;
	let chain_state = EntityChainState::from_record(&entity_info);
	let mut snapshot = EntitySnapshot::from_chain_state(&chain_state, token_str);

	let nym_req = EntityNymRequest {
		auth: fresh_authorization_with_client(client, signer).await?,
		token: token.clone(),
	};
	if let Some(nym) = client.query().entity().entity_nym(&nym_req).await? {
		snapshot.set_entity_nym(nym);
	}

	entity::render_entity_snapshot(
		client,
		signer,
		token,
		&mut snapshot,
		common.view,
		common.output_json,
		chain_prefix,
		0,
		true,
	)
	.await
}

async fn render_registry_state(
	client: &Client,
	signer: &tx::signer::Keypair,
	registry: Ss58Identifier,
	info: &RegistryInfoView,
	common: &CommonCliOptions,
) -> Result<()> {
	let registry_ss58 = demo::ss58_string(&registry);
	let auth = fresh_authorization_with_client(client, signer).await?;
	let lookup_req = RegisterLookupSpecsRequest { auth: auth.clone(), registry: registry.clone() };
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	let (timeline, next_cursor) = token_timeline(client, &auth, &registry, Some(12)).await?;
	render_registry_snapshot_cli(
		&registry_ss58,
		info,
		&lookups,
		&timeline,
		next_cursor,
		common.view,
		common.output_json,
	)
}

async fn render_packet_state(
	client: &Client,
	signer: &tx::signer::Keypair,
	registry: Ss58Identifier,
	packet: Ss58Identifier,
	snapshot: PacketSnapshotView,
	common: &CommonCliOptions,
) -> Result<()> {
	let registry_ss58 = demo::ss58_string(&registry);
	let packet_ss58 = demo::ss58_string(&packet);
	let auth = fresh_authorization_with_client(client, signer).await?;
	let (timeline, next_cursor) = token_timeline(client, &auth, &packet, Some(12)).await?;
	render_packet_snapshot_cli(
		&registry_ss58,
		&packet_ss58,
		None,
		&snapshot,
		&timeline,
		next_cursor,
		common.view,
		common.output_json,
	);
	Ok(())
}
