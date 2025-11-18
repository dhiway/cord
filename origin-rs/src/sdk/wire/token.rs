use origin_primitives::view_api::{
	AuthorizationRequest, TokenStateVersionRequest, TokenTimelineRequest,
};

use crate::{
	client::Client,
	sdk::{
		error::Result,
		types::TokenId,
	},
	types::token::StateEventRecord,
};

pub(crate) async fn state_version(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &TokenId,
) -> Result<u32> {
	let req = TokenStateVersionRequest { auth: auth.clone(), token: id.clone() };
	let version = client.query().token().state_version(&req).await?;
	Ok(version)
}

pub(crate) async fn timeline(
	client: &Client,
	auth: &AuthorizationRequest,
	id: &TokenId,
	start: Option<u32>,
	limit: Option<u32>,
) -> Result<(Vec<StateEventRecord>, Option<u32>)> {
	let req = TokenTimelineRequest { auth: auth.clone(), token: id.clone(), start, limit };
	let resp = client.query().token().timeline(&req).await?;
	Ok(resp)
}
