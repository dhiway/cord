use crate::{
	builders::build_packet_attributes,
	context::{ExampleContext, Hash},
	cord,
	formatting::token_to_string,
	pallets::register::RegistryResult,
	sample_data::{SampleData, TemplateContext},
};
use color_eyre::eyre::{eyre, Result};
use cord::runtime_types::cord_primitives::identifier::Ss58Identifier;

pub struct PacketResult {
	pub token: Ss58Identifier,
	pub message: String,
	pub block_hash: Hash,
}

pub async fn create_packet(
	ctx: &ExampleContext,
	samples: &SampleData,
	template: &TemplateContext,
	entity_token: &Ss58Identifier,
	registry: &RegistryResult,
) -> Result<PacketResult> {
	let payload = build_packet_attributes(
		samples,
		&registry.blueprint,
		entity_token,
		&registry.token,
		template,
	)?;
	let tx = cord::tx().register().create_packet(registry.token.clone(), payload);
	let submit = ctx.submit("Register::create_packet", &tx).await?;
	let created = submit
		.events
		.find_first::<cord::register::events::PacketCreated>()?
		.ok_or_else(|| eyre!("PacketCreated event missing"))?;

	Ok(PacketResult {
		token: created.packet,
		message: format!("Packet anchored under registry {}", token_to_string(&registry.token)),
		block_hash: submit.block_hash,
	})
}
