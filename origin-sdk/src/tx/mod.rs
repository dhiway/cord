pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::{signer::Signer, OriginClient};

/// High-level tx facade regrouped by pallet. Uses typed builders and schema transforms.
pub struct Tx<'a> {
	client: &'a OriginClient,
}

impl<'a> Tx<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> TxWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		TxWithSigner { client: self.client, signer }
	}

	pub fn entity(&self) -> entity::EntityTx<'a> {
		entity::EntityTx::new(self.client)
	}

	pub fn registry(&self) -> registry::RegistryTx<'a> {
		registry::RegistryTx::new(self.client)
	}

	pub fn packet(&self) -> packet::PacketTx<'a> {
		packet::PacketTx::new(self.client)
	}

	pub fn token(&self) -> token::TokenTx<'a> {
		token::TokenTx::new(self.client)
	}
}

/// Tx facade with signer bound for convenience.
pub struct TxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> TxWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub fn entity(&self) -> entity::EntityTxWithSigner<'a, S> {
		entity::EntityTxWithSigner::new(self.client, self.signer.clone())
	}

	pub fn registry(&self) -> registry::RegistryTxWithSigner<'a, S> {
		registry::RegistryTxWithSigner::new(self.client, self.signer.clone())
	}

	pub fn packet(&self) -> packet::PacketTxWithSigner<'a, S> {
		packet::PacketTxWithSigner::new(self.client, self.signer.clone())
	}

	pub fn token(&self) -> token::TokenTxWithSigner<'a, S> {
		token::TokenTxWithSigner::new(self.client, self.signer.clone())
	}
}
