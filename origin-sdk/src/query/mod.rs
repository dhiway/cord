pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::{signer::Signer, OriginClient};

/// Unified query facade regrouped by pallet.
pub struct Query<'a> {
	client: &'a OriginClient,
}

impl<'a> Query<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> QueryWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		QueryWithSigner { client: self.client, signer }
	}
}

/// Query facade with an attached signer for view authorization and tx shortcuts.
pub struct QueryWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> QueryWithSigner<'a, S> {
	pub fn entity(&self) -> entity::EntityClientWithSigner<'a, S> {
		entity::EntityClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn registry(&self) -> registry::RegistryClientWithSigner<'a, S> {
		registry::RegistryClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn packet(&self) -> packet::PacketClientWithSigner<'a, S> {
		packet::PacketClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn token(&self) -> token::TokenClientWithSigner<'a, S> {
		token::TokenClientWithSigner::new(self.client, self.signer.clone())
	}
}
