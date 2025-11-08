use crate::{
	builders::{build_entity_info, build_packet_attributes, build_registry_blueprint, RegistryBlueprint},
	context::ExampleContext,
	cord,
};
use color_eyre::eyre::{eyre, Result};
use comfy_table::{presets::UTF8_FULL, Table};
use cord::runtime_types::{
	bounded_collections::bounded_vec::BoundedVec,
	cord_primitives::{element::Elum, identifier::Ss58Identifier},
};
use hex::encode as hex_encode;
type AttributeKey = BoundedVec<u8>;

pub struct FlowOptions {
	pub label: String,
}

pub async fn run_walkthrough(ctx: &ExampleContext, opts: &FlowOptions) -> Result<()> {
	let mut printer = FlowPrinter::new(&opts.label);
	let entity_token = ensure_entity(ctx, &mut printer, opts).await?;
	let registry = create_registry(ctx, &mut printer, opts).await?;
	let packet_token = create_packet(ctx, &entity_token, &registry, &mut printer, opts).await?;

	printer.note("Packet", &packet_token, "Packet anchored and ready for pallet testing");
	printer.finish();
	Ok(())
}

struct RegistryArtifacts {
	pub token: Ss58Identifier,
	pub blueprint: RegistryBlueprint,
}

async fn ensure_entity(
	ctx: &ExampleContext,
	printer: &mut FlowPrinter,
	opts: &FlowOptions,
) -> Result<Ss58Identifier> {
	let snapshot = ctx.client.storage().at_latest().await?;
	let lookup = cord::storage().entity().ss58_of_active_accounts(ctx.account_id.clone());
	if let Some(existing) = snapshot.fetch(&lookup).await? {
		printer.note("Identifiers", &existing, "Re-used existing entity token");
		return Ok(existing);
	}

	let info = build_entity_info(&opts.label)?;
	let tx = cord::tx().entity().set_info(info);
	let events = ctx.submit("Entity::set_info", &tx).await?;
	let record = events
		.find_first::<cord::entity::events::EntityInfoSet>()?
		.ok_or_else(|| eyre!("EntityInfoSet event not emitted"))?;
	printer.note("Identifiers", &record.token, "Entity profile set via pallet-entity::set_info");
	Ok(record.token)
}

async fn create_registry(
	ctx: &ExampleContext,
	printer: &mut FlowPrinter,
	opts: &FlowOptions,
) -> Result<RegistryArtifacts> {
	let blueprint = build_registry_blueprint(&opts.label)?;
	let tx = cord::tx().register().create_registry(
		blueprint.info.clone(),
		blueprint.kind.clone(),
		blueprint.attribute_schema.clone(),
		blueprint.token_spec.clone(),
		blueprint.lookup_specs.clone(),
	);
	let events = ctx.submit("Register::create_registry", &tx).await?;
	let created = events
		.find_first::<cord::register::events::RegistryCreated>()?
		.ok_or_else(|| eyre!("RegistryCreated event missing"))?;
	printer.note(
		"Registers",
		&created.registry,
		"Registry minted with pallet-register::create_registry",
	);

	Ok(RegistryArtifacts { token: created.registry, blueprint })
}

async fn create_packet(
	ctx: &ExampleContext,
	entity_token: &Ss58Identifier,
	registry: &RegistryArtifacts,
	printer: &mut FlowPrinter,
	opts: &FlowOptions,
) -> Result<Ss58Identifier> {
	let payload = build_packet_attributes(&registry.blueprint, entity_token, &opts.label)?;
	let tx = cord::tx().register().create_packet(registry.token.clone(), payload.clone());
	let events = ctx.submit("Register::create_packet", &tx).await?;
	let created = events
		.find_first::<cord::register::events::PacketCreated>()?
		.ok_or_else(|| eyre!("PacketCreated event missing"))?;

	printer.note(
		"Packets",
		&created.packet,
		&format!("Packet anchored under registry {}", token_to_string(&registry.token)),
	);

	dump_packet_state(ctx, &created.packet, payload).await?;
	Ok(created.packet)
}

async fn dump_packet_state(
	ctx: &ExampleContext,
	packet: &Ss58Identifier,
	attrs: BoundedVec<(AttributeKey, Elum)>,
) -> Result<()> {
	let snapshot = ctx.client.storage().at_latest().await?;
	let storage = cord::storage().register().packets(packet.clone());
	if let Some(state) = snapshot.fetch(&storage).await? {
		tracing::info!(target: "anchor", "packet {:?} latest version {:?}", token_to_string(packet), state.latest_version);
		tracing::debug!(target: "anchor", payload = ?describe_attributes(&attrs));
	}
	Ok(())
}

struct FlowPrinter {
	table: Table,
}

impl FlowPrinter {
	fn new(label: &str) -> Self {
		let mut table = Table::new();
		table.load_preset(UTF8_FULL);
		table.set_header(["Stage", "Token", "Outcome"]);
		table.add_row(["Context", label, "Demonstrating identifiers → registry → packet"]);
		Self { table }
	}

	fn note(&mut self, stage: &str, token: &Ss58Identifier, message: impl AsRef<str>) {
		self.table.add_row([stage, &token_to_string(token), message.as_ref()]);
	}

	fn finish(self) {
		println!("\n{}", self.table);
	}
}

fn token_to_string(token: &Ss58Identifier) -> String {
	let raw = (token.0).0.clone();
	String::from_utf8(raw).unwrap_or_else(|_| "<invalid>".into())
}

fn describe_attributes(attrs: &BoundedVec<(AttributeKey, Elum)>) -> Vec<(String, String)> {
	(attrs.0)
		.iter()
		.map(|(key, value)| {
			let key_bytes = key.0.clone();
			let key_str = String::from_utf8_lossy(&key_bytes).into_owned();
			(key_str, describe_element(value))
		})
		.collect()
}

fn describe_element(element: &Elum) -> String {
	match element {
		Elum::None => "None".into(),
		Elum::Raw(data) => {
			let bytes = data.0.clone();
			format!("Raw({})", String::from_utf8_lossy(&bytes))
		},
		Elum::Bool(flag) => format!("Bool({flag})"),
		Elum::U64(bytes) => format!("U64({})", u64::from_le_bytes(*bytes)),
		Elum::U128(bytes) => format!("U128({})", u128::from_le_bytes(*bytes)),
		Elum::Hash(digest) => format!("Hash(0x{})", hex_encode(digest)),
		Elum::Token(id) => format!("Token({})", token_to_string(id)),
		Elum::CID(cid) => {
			let data = cid.0.clone();
			format!("CID({})", hex_encode(data))
		},
	}
}
