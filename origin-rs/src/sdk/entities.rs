use origin_primitives::view_api::AuthorizationRequest;

use crate::{
	client::Client,
	sdk::{
		error::Result,
		types::{Entity, EntityId, HistoryEntry},
		wire,
	},
};

/// Public-facing Entity API.
pub struct EntityApi<'a> {
	client: &'a Client,
}

impl<'a> EntityApi<'a> {
	pub(crate) fn new(client: &'a Client) -> Self {
		Self { client }
	}

	/// Fetch the current state of an Entity by id.
	pub async fn get(&self, auth: &AuthorizationRequest, id: &EntityId) -> Result<Entity> {
		wire::entity::fetch_entity(self.client, auth, id).await
	}

	/// Fetch the change history for an Entity.
	pub async fn history(
		&self,
		auth: &AuthorizationRequest,
		id: &EntityId,
	) -> Result<Vec<HistoryEntry>> {
		wire::entity::fetch_history(self.client, auth, id).await
	}

	/// Create or update an Entity on-chain (scaffolding; not wired yet).
	pub async fn upsert(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		entity: &Entity,
		opts: crate::tx::TxOptions,
	) -> Result<()> {
		wire::entity::upsert_entity(self.client, signer, entity, opts).await
	}
}
