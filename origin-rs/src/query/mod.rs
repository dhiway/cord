pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::{signer::OriginSigner, OriginClient};

/// Unified query facade regrouped by pallet.
pub struct Query<'a> {
	client: &'a OriginClient,
}

impl<'a> Query<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using(&self, signer: OriginSigner) -> QueryWithSigner<'a> {
		QueryWithSigner { client: self.client, signer }
	}
}

/// Query facade with an attached signer for view authorization and tx shortcuts.
pub struct QueryWithSigner<'a> {
	client: &'a OriginClient,
	signer: OriginSigner,
}

impl<'a> QueryWithSigner<'a> {
	pub fn entity(&self) -> entity::EntityClientWithSigner<'a> {
		entity::EntityClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn registry(&self) -> registry::RegistryClientWithSigner<'a> {
		registry::RegistryClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn packet(&self) -> packet::PacketClientWithSigner<'a> {
		packet::PacketClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn token(&self) -> token::TokenClientWithSigner<'a> {
		token::TokenClientWithSigner::new(self.client, self.signer.clone())
	}
}
