use crate::{
	context::ExampleContext,
	formatting::token_to_string,
	pallets::{entity, packet, register},
	sample_data::{SampleData, TemplateContext},
	view_client::{PacketViewData, RegisterViewClient, RegistryViewData},
	view_types::LookupSpecView,
};
use color_eyre::eyre::Result;

pub struct FlowOptions {
	pub base_label: String,
	pub run_id: String,
	pub scoped_label: String,
}

impl FlowOptions {
	pub fn new(base_label: String, run_id: String) -> Self {
		let scoped_label = format!("{base_label}-{run_id}");
		Self { base_label, run_id, scoped_label }
	}

	pub fn label(&self) -> &str {
		&self.scoped_label
	}
}

pub async fn run_walkthrough(
	ctx: &ExampleContext,
	opts: &FlowOptions,
	samples: &SampleData,
) -> Result<()> {
	let template = TemplateContext::new(
		opts.base_label.clone(),
		opts.run_id.clone(),
		opts.label().to_string(),
	);
	let entity = entity::ensure_profile(ctx, samples, &template).await?;
	let registry = register::create_registry(ctx, samples, &template).await?;
	let packet = packet::create_packet(ctx, samples, &template, &entity.token, &registry).await?;
	tokio::time::sleep(std::time::Duration::from_secs(6)).await;

	#[cfg(debug_assertions)]
	{
		let register_snapshot = ctx.client.storage().at(registry.block_hash);
		let register_key = crate::cord::storage().register().registries(registry.token.clone());
		let exists = register_snapshot.fetch(&register_key).await?.is_some();
		let entity_snapshot = ctx.client.storage().at_latest().await?;
		let entity_key =
			crate::cord::storage().entity().ss58_of_active_accounts(ctx.account_id.clone());
		let mapped = entity_snapshot.fetch(&entity_key).await?.is_some();
		tracing::info!(
			target: "anchor",
			"registry {} storage_exists={exists}; account_mapped={mapped}",
			token_to_string(&registry.token)
		);
	}

	let view_client = RegisterViewClient::new(ctx, packet.block_hash).await?;
	let registry_view = view_client.registry_overview(&registry.token).await?;
	let packet_view = view_client.packet_snapshot(&registry.token, &packet.token).await?;

	FlowPrinter::render(opts, &entity, &registry, &registry_view, &packet, &packet_view);
	Ok(())
}

struct FlowPrinter;

impl FlowPrinter {
	fn render(
		opts: &FlowOptions,
		entity: &entity::EntityResult,
		registry: &register::RegistryResult,
		registry_view: &RegistryViewData,
		packet: &packet::PacketResult,
		packet_view: &PacketViewData,
	) {
		println!(
			"\nContext: {} identifiers → registry → packet (run {})",
			opts.base_label, opts.run_id
		);
		println!();

		println!("Entity");
		println!("  Token    : {}", token_to_string(&entity.token));
		println!("  Outcome  : {}", entity.message);
		println!("  Profile  : {}", entity.view_summary);

		println!("\nRegistry");
		println!("  Token    : {}", token_to_string(&registry.token));
		println!("  Outcome  : {}", registry.message);
		println!("  Kind     : {:?}", registry_view.info.kind);
		println!("  Status   : {:?}", registry_view.info.status);
		println!("  Maintainer: {}", token_to_string(&registry_view.info.maintainer));
		println!("  Info     : {}", registry_view.info.info);
		println!("  Token Spec: {}", format_lookup_spec(&registry_view.info.token_spec));
		if !registry_view.info.lookup_specs.is_empty() {
			let lookups = registry_view
				.info
				.lookup_specs
				.iter()
				.map(format_lookup_spec)
				.collect::<Vec<_>>()
				.join(", ");
			println!("  Lookups  : {lookups}");
		}
		println!("  Schema   :");
		for attr in &registry_view.info.attributes {
			let optional = if attr.optional { " [optional]" } else { "" };
			println!("    - {} ({:?}){}", attr.key_label, attr.kind, optional);
		}

		println!("\nPacket");
		println!("  Token    : {}", token_to_string(&packet.token));
		println!("  Outcome  : {}", packet.message);
		println!("  Controller: {}", token_to_string(&packet_view.snapshot.state.controller));
		println!("  Status   : {:?}", packet_view.snapshot.state.status);
		println!("  Attributes:");
		for attr in &packet_view.snapshot.state.attributes {
			println!("    - {} = {}", attr.key_label, attr.value);
		}
	}
}

fn format_lookup_spec(spec: &LookupSpecView) -> String {
	match spec {
		LookupSpecView::Single(attr) => attr.clone(),
		LookupSpecView::Combo(list) => {
			let joined = list.join(" + ");
			format!("[{joined}]")
		},
	}
}
