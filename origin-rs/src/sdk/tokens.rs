use origin_primitives::view_api::AuthorizationRequest;

use crate::{
	client::Client,
	sdk::{
		error::Result,
		types::{Token, TokenId},
		wire,
	},
	types::token::StateEventRecord,
};

/// Public-facing Token API.
pub struct TokenApi<'a> {
	client: &'a Client,
}

impl<'a> TokenApi<'a> {
	pub(crate) fn new(client: &'a Client) -> Self {
		Self { client }
	}

	pub async fn version(&self, auth: &AuthorizationRequest, id: &TokenId) -> Result<u32> {
		wire::token::state_version(self.client, auth, id).await
	}

	pub async fn timeline(
		&self,
		auth: &AuthorizationRequest,
		id: &TokenId,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<(Vec<StateEventRecord>, Option<u32>)> {
		wire::token::timeline(self.client, auth, id, start, limit).await
	}

	pub fn token(&self, id: TokenId) -> Token {
		Token { id }
	}
}
