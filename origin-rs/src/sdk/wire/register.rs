use origin_primitives::view_api::{AuthorizationRequest, RegisterDetailsRequest};

use crate::{
	client::Client,
	sdk::{
		error::{OriginError, Result},
		types::{Register, RegisterId},
	},
	tx::TxOptions,
};
use subxt::tx::Signer;

pub(crate) async fn fetch_register(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &RegisterId,
) -> Result<Register> {
	let req = RegisterDetailsRequest { auth: auth.clone(), registry: id.clone() };
	let view = client.query().register().details(&req).await?;
	Ok(Register::from_view(id.clone(), view))
}

pub(crate) async fn update_register_info<S>(
	client: &Client,
	signer: &S,
	register: &Register,
	opts: TxOptions,
) -> Result<()>
where
	S: Signer<crate::params::config::OriginConfig>,
{
	use crate::types::{element::element_json_to_dynamic, element::ElementJson, identifier_value};
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

	let info = element_json_to_dynamic(&to_element_json(&register.info))?;
	let registry_value = identifier_value(&String::from_utf8_lossy(register.id.as_ref()))?;
	let args = Value::named_composite([("registry", registry_value), ("info", info)]);

	let call = client.tx().build("Register", "update_registry_info", args).await.map_err(OriginError::from)?;
	client.tx().sign_and_submit(call, signer, opts).await.map_err(OriginError::from)?;
	Ok(())
}

pub(crate) async fn fetch_overview(
	client: &Client,
	auth: &origin_primitives::view_api::AuthorizationRequest,
	id: &RegisterId,
) -> Result<Register> {
	let (info, specs) = client.query().register().overview(auth, id).await?;
	let register = Register {
		id: id.clone(),
		info: info.info.clone(),
		maintainer: info.maintainer,
		attributes: info.attributes,
		token_spec: info.token_spec,
		lookup_specs: specs,
		kind: info.kind,
		status: info.status,
	};
	Ok(register)
}
use base64;
use base64::Engine;
use bs58;
use hex;
