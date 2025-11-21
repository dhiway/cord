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
	use crate::types::{element::element_json_to_dynamic, identifier_value};
	use subxt::dynamic::Value;

	let info = element_json_to_dynamic(&crate::sdk::types::element_view_to_json(&register.info))?;
	let registry_value = identifier_value(&String::from_utf8_lossy(register.id.as_ref()))?;
	let args = Value::named_composite([("registry", registry_value), ("info", info)]);

	let call = client
		.tx()
		.build("Register", "update_registry_info", args)
		.await
		.map_err(OriginError::from)?;
	client
		.tx()
		.sign_and_submit(call, signer, opts)
		.await
		.map_err(OriginError::from)?;
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
