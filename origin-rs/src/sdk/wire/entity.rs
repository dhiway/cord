use base64;
use base64::Engine;
use bs58;
use hex;
use origin_primitives::{
	identifier::Ss58Identifier,
	view_api::{AuthorizationRequest, EntityAttributeHistoryRequest},
};

use crate::{
	client::Client,
	sdk::{
		error::{OriginError, Result},
		types::{Attribute, Entity, EntityId, EntityOverview, HistoryEntry},
	},
	tx::TxOptions,
	types::entity::EntityInfoRecord,
};
use subxt::tx::Signer;

pub(crate) async fn fetch_entity(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &EntityId,
) -> Result<Entity> {
	let info: Option<EntityInfoRecord> = client.query().entity().details(auth, id).await?;
	let info = info.ok_or(OriginError::NotFound)?;
	Ok(entity_from_record(id.clone(), info))
}

pub(crate) async fn fetch_history(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &EntityId,
) -> Result<Vec<HistoryEntry>> {
	let req = EntityAttributeHistoryRequest { auth: auth.clone(), token: id.clone() };
	let history = client.query().entity().attribute_history(&req).await?;
	Ok(history)
}

pub(crate) async fn fetch_overview(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &EntityId,
) -> Result<EntityOverview> {
	let req = origin_primitives::view_api::EntityOverviewRequest {
		auth: auth.clone(),
		token: id.clone(),
		history_limit: None,
	};
	match client.query().entity().overview(&req).await {
		Ok(Some(view)) => {
			let entity = entity_from_view(id.clone(), &view.info);
			let history = view
				.history
				.into_iter()
				.map(|entry| HistoryEntry {
					key_hex: entry.key_hex,
					key_utf8: entry.key_utf8,
					version: entry.version,
					old_value_base64: entry.old_value_base64,
					block: crate::types::entity::BlockRef {
						height: entry.block.height,
						index: entry.block.index,
					},
				})
				.collect();
			let timeline = Vec::new();
			let nym = view.nym.and_then(|bytes| String::from_utf8(bytes).ok());
			let linked_accounts = view.linked_accounts;
			let entity_overview =
				EntityOverview { entity, history, timeline, nym, linked_accounts };
			Ok(entity_overview)
		},
		Ok(None) => Err(OriginError::NotFound),
		Err(err) => Err(OriginError::Rpc(err)),
	}
}

fn entity_from_record(id: Ss58Identifier, record: EntityInfoRecord) -> Entity {
	let attributes =
		record.attributes.unwrap_or_default().into_iter().map(Attribute::from).collect();

	Entity { id, display: record.display, web: record.web, email: record.email, attributes }
}

fn entity_from_view(id: Ss58Identifier, view: &origin_primitives::view::EntityInfoView) -> Entity {
	let attributes = view
		.attributes
		.clone()
		.unwrap_or_default()
		.into_iter()
		.map(Attribute::from)
		.collect();
	Entity { id, display: view.display.clone(), web: view.web.clone(), email: view.email.clone(), attributes }
}

/// Encode an entity and submit `Entity::set_info`.
pub(crate) async fn upsert_entity<S>(
	client: &Client,
	signer: &S,
	entity: &Entity,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	use crate::types::{attribute_pair_value, element::element_json_to_dynamic, element::ElementJson};
	use scale_value::Composite;
	use subxt::dynamic::Value;

	let to_element_json = |el: &origin_primitives::view::ElementView| -> ElementJson {
		match el {
			origin_primitives::view::ElementView::None => ElementJson::None,
			origin_primitives::view::ElementView::Raw(bytes) => {
				ElementJson::RawBase64(base64::engine::general_purpose::STANDARD.encode(bytes))
			},
			origin_primitives::view::ElementView::Bool(v) => ElementJson::Bool(*v),
			origin_primitives::view::ElementView::U64(v) => ElementJson::U64(*v),
			origin_primitives::view::ElementView::U128(v) => ElementJson::U128(*v),
			origin_primitives::view::ElementView::Hash(h) => ElementJson::HashHex(hex::encode(h)),
			origin_primitives::view::ElementView::Token(tok) => {
				ElementJson::TokenSs58(String::from_utf8_lossy(tok.as_ref()).into_owned())
			},
			origin_primitives::view::ElementView::Cid(cid) => {
				ElementJson::CidBase58(bs58::encode(cid).into_string())
			},
		}
	};

	let display = element_json_to_dynamic(&to_element_json(&entity.display))?;
	let web = element_json_to_dynamic(&to_element_json(&entity.web))?;
	let email = element_json_to_dynamic(&to_element_json(&entity.email))?;

	let attrs_value = if entity.attributes.is_empty() {
		Value::variant("None", Composite::unnamed(Vec::new()))
	} else {
		let pairs = entity
			.attributes
			.iter()
			.map(|attr| -> Result<Value> {
				let elem = to_element_json(&attr.value);
				let pair = attribute_pair_value(&attr.key, &elem)?;
				Ok(pair)
			})
			.collect::<Result<Vec<_>>>()?;
		let inner = Value::unnamed_composite(pairs);
		Value::variant("Some", Composite::unnamed(vec![inner]))
	};

	let info = Value::named_composite([
		("display", display),
		("web", web),
		("email", email),
		("attributes", attrs_value),
	]);

	let args = Value::named_composite([("info", info)]);
	let call = client.tx().build("Entity", "set_info", args).await.map_err(OriginError::from)?;
	client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}
