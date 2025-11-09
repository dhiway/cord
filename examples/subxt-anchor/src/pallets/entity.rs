use crate::{
	builders::build_entity_info,
	context::ExampleContext,
	cord,
	formatting::describe_element,
	sample_data::{SampleData, TemplateContext},
};
use color_eyre::eyre::{eyre, Result};
use cord::runtime_types::cord_primitives::{element::Elum, identifier::Ss58Identifier};

pub struct EntityResult {
	pub token: Ss58Identifier,
	pub message: String,
	pub view_summary: String,
}

pub async fn ensure_profile(
	ctx: &ExampleContext,
	samples: &SampleData,
	template: &TemplateContext,
) -> Result<EntityResult> {
	let snapshot = ctx.client.storage().at_latest().await?;
	let lookup = cord::storage().entity().ss58_of_active_accounts(ctx.account_id.clone());
	if let Some(existing) = snapshot.fetch(&lookup).await? {
		let view = view_entity(ctx, &existing).await?;
		return Ok(EntityResult {
			token: existing,
			message: "Re-used existing entity token".into(),
			view_summary: view,
		});
	}

	let info = build_entity_info(samples, template)?;
	let tx = cord::tx().entity().set_info(info);
	let submit = ctx.submit("Entity::set_info", &tx).await?;
	let record = submit
		.events
		.find_first::<cord::entity::events::EntityInfoSet>()?
		.ok_or_else(|| eyre!("EntityInfoSet event not emitted"))?;
	let view = view_entity(ctx, &record.token).await?;

	Ok(EntityResult {
		token: record.token,
		message: "Entity profile set via pallet-entity::set_info".into(),
		view_summary: view,
	})
}

async fn view_entity(ctx: &ExampleContext, token: &Ss58Identifier) -> Result<String> {
	let snapshot = ctx.client.storage().at_latest().await?;
	let storage = cord::storage().entity().entity_info_of(token.clone());
	if let Some(info) = snapshot.fetch(&storage).await? {
		let display = inline(&info.display);
		let attr_count = info.attributes.as_ref().map(|attrs| (attrs.0).0.len()).unwrap_or(0);
		Ok(format!("display={display}; attributes={attr_count}"))
	} else {
		Ok("entity info unavailable".into())
	}
}

fn inline(value: &Elum) -> String {
	match value {
		cord::runtime_types::cord_primitives::element::Elum::Raw(data) => {
			String::from_utf8_lossy(&data.0).into_owned()
		},
		other => describe_element(other),
	}
}
