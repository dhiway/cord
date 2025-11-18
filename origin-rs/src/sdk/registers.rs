use origin_primitives::view_api::AuthorizationRequest;

use crate::{
	client::Client,
	sdk::{
		error::Result,
		types::{Register, RegisterId},
		wire,
	},
};

/// Public-facing Register API.
pub struct RegisterApi<'a> {
	client: &'a Client,
}

impl<'a> RegisterApi<'a> {
	pub(crate) fn new(client: &'a Client) -> Self {
		Self { client }
	}

	pub async fn get(&self, auth: &AuthorizationRequest, id: &RegisterId) -> Result<Register> {
		wire::register::fetch_register(self.client, auth, id).await
	}

	/// Placeholder for future register updates.
	pub async fn update(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		register: &Register,
		opts: crate::tx::TxOptions,
	) -> Result<()> {
		wire::register::update_register_info(self.client, signer, register, opts).await
	}
}
