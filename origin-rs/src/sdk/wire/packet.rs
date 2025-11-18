use origin_primitives::view_api::{
	AuthorizationRequest, RegisterPacketSnapshotByTokenRequest, RegisterPacketSnapshotRequest,
};

use crate::{
	client::Client,
	query::register::PacketSnapshotView,
	sdk::{
		error::{OriginError, Result},
		types::{PacketId, PacketState, RegisterId},
	},
	tx::TxOptions,
};
use origin_primitives::registry::RegistryInfoView;
use subxt::tx::Signer;
use base64::Engine;

pub(crate) async fn fetch_packet_state(
	client: &Client,
	auth: &AuthorizationRequest,
	registry: &RegisterId,
	packet: &PacketId,
	version: Option<u32>,
) -> Result<PacketState> {
	let req = RegisterPacketSnapshotRequest {
		auth: auth.clone(),
		registry: registry.clone(),
		packet: packet.clone(),
		version,
	};
	let view: PacketSnapshotView = client.query().register().packet_snapshot(&req).await?;
	Ok(PacketState::from_dev(packet.clone(), view.state))
}

pub(crate) async fn fetch_packet_by_token(
	client: &Client,
	auth: &AuthorizationRequest,
	token: &PacketId,
	version: Option<u32>,
) -> Result<Option<PacketState>> {
	let req =
		RegisterPacketSnapshotByTokenRequest { auth: auth.clone(), token: token.clone(), version };
	let view: Option<PacketSnapshotView> =
		client.query().register().packet_snapshot_by_token(&req).await?;
	Ok(view.map(|snapshot| PacketState::from_dev(token.clone(), snapshot.state)))
}

pub(crate) async fn create_packet<S>(
	client: &Client,
	signer: &S,
	registry: &RegisterId,
	attributes_json: serde_json::Value,
	schema: &RegistryInfoView,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	let registry_ss58 = String::from_utf8_lossy(registry.as_ref()).into_owned();
	let call = client
		.tx()
		.packet_create_json(&registry_ss58, attributes_json, schema)
		.await
		.map_err(OriginError::from)?;
	let _tx_in_block =
		client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}

pub(crate) async fn update_packet<S>(
	client: &Client,
	signer: &S,
	registry: &RegisterId,
	packet: &PacketId,
	attributes_json: serde_json::Value,
	schema: &RegistryInfoView,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	let registry_ss58 = String::from_utf8_lossy(registry.as_ref()).into_owned();
	let packet_ss58 = String::from_utf8_lossy(packet.as_ref()).into_owned();
	let call = client
		.tx()
		.packet_update_json(&registry_ss58, &packet_ss58, attributes_json, schema)
	.await
	.map_err(OriginError::from)?;
	client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}

pub(crate) async fn create_packet_typed<S>(
	client: &Client,
	signer: &S,
	registry: &RegisterId,
	attributes: Vec<crate::sdk::types::Attribute>,
	schema: &RegistryInfoView,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	let payload = build_payload_from_attributes(attributes, schema, crate::types::PayloadMode::Full)?;
	let registry_ss58 = String::from_utf8_lossy(registry.as_ref()).into_owned();
	let args = subxt::dynamic::Value::named_composite([
		("rtoken", crate::types::identifier_value(&registry_ss58).map_err(OriginError::from)?),
		("attributes", payload),
	]);
	let call = client.tx().build("Register", "create_packet", args).await.map_err(OriginError::from)?;
	client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}

pub(crate) async fn update_packet_typed<S>(
	client: &Client,
	signer: &S,
	registry: &RegisterId,
	packet: &PacketId,
	attributes: Vec<crate::sdk::types::Attribute>,
	schema: &RegistryInfoView,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	let payload =
		build_payload_from_attributes(attributes, schema, crate::types::PayloadMode::Partial)?;
	let registry_ss58 = String::from_utf8_lossy(registry.as_ref()).into_owned();
	let packet_ss58 = String::from_utf8_lossy(packet.as_ref()).into_owned();
	let args = subxt::dynamic::Value::named_composite([
		("rtoken", crate::types::identifier_value(&registry_ss58).map_err(OriginError::from)?),
		("ptoken", crate::types::identifier_value(&packet_ss58).map_err(OriginError::from)?),
		("attributes", payload),
	]);
	let call = client.tx().build("Register", "update_packet", args).await.map_err(OriginError::from)?;
	client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}

fn build_payload_from_attributes(
	attributes: Vec<crate::sdk::types::Attribute>,
	view: &RegistryInfoView,
	mode: crate::types::PayloadMode,
) -> Result<subxt::dynamic::Value> {
    use crate::types::{attribute_pair_value, registry::RegistrySchema};

	let schema = RegistrySchema::from_view(view);
	let mut collected = Vec::new();
	let mut seen = std::collections::BTreeSet::new();

	for attr in attributes {
		if !seen.insert(attr.key.clone()) {
			return Err(OriginError::Decode(crate::sdk::error::DecodeError::Dynamic(
				"duplicate attribute key".into(),
			)));
		}
		let Some(schema_attr) = schema.attribute(&attr.key) else {
			return Err(OriginError::Decode(crate::sdk::error::DecodeError::Dynamic(
				"unknown attribute key".into(),
			)));
		};
		let element_json = element_json_from_view(&attr.value)?;
		collected.push((schema_attr.key.clone(), element_json));
	}

	if mode == crate::types::PayloadMode::Full {
		for schema_attr in view.attributes.iter().filter(|a| !a.optional) {
			if !seen.contains(&schema_attr.key) {
				return Err(OriginError::Decode(crate::sdk::error::DecodeError::Dynamic(
					"missing required attribute".into(),
				)));
			}
		}
	}

	collected.sort_by(|a, b| a.0.cmp(&b.0));

	let mut pairs = Vec::new();
	for (key, element) in collected {
		pairs.push(attribute_pair_value(&key, &element).map_err(OriginError::from)?);
	}
	Ok(subxt::dynamic::Value::unnamed_composite(pairs))
}

fn element_json_from_view(
	el: &origin_primitives::view::ElementView,
) -> Result<crate::types::element::ElementJson> {
	use crate::types::element::ElementJson;
	Ok(match el {
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
	})
}
