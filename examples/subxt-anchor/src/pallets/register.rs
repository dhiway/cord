use crate::{
	builders::{build_registry_blueprint, RegistryBlueprint},
	context::{ExampleContext, Hash},
	cord,
	sample_data::{SampleData, TemplateContext},
};
use color_eyre::eyre::{eyre, Result};
use cord::runtime_types::cord_primitives::identifier::Ss58Identifier;

pub struct RegistryResult {
	pub token: Ss58Identifier,
	pub message: String,
	pub blueprint: RegistryBlueprint,
	pub block_hash: Hash,
}

pub async fn create_registry(
	ctx: &ExampleContext,
	samples: &SampleData,
	template: &TemplateContext,
) -> Result<RegistryResult> {
	let blueprint = build_registry_blueprint(samples, template)?;
	let tx = cord::tx().register().create_registry(
		blueprint.info.clone(),
		blueprint.kind.clone(),
		blueprint.attribute_schema.clone(),
		blueprint.token_spec.clone(),
		blueprint.lookup_specs.clone(),
	);
	let submit = ctx.submit("Register::create_registry", &tx).await?;
	let created = submit
		.events
		.find_first::<cord::register::events::RegistryCreated>()?
		.ok_or_else(|| eyre!("RegistryCreated event missing"))?;

	#[cfg(debug_assertions)]
	{
		use cord::storage;
		use tracing::debug;
		let snapshot = ctx.client.storage().at(submit.block_hash);
		let storage_key = storage().register().registries(created.registry.clone());
		let exists = snapshot.fetch(&storage_key).await?.is_some();
		debug_assert!(exists, "registry not present in storage after creation");
		debug!(target: "anchor", "registry {} storage_exists={exists}", crate::formatting::token_to_string(&created.registry));
	}

	Ok(RegistryResult {
		token: created.registry,
		message: "Registry minted with pallet-register::create_registry".into(),
		blueprint,
		block_hash: submit.block_hash,
	})
}
