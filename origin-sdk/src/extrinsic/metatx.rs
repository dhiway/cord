use std::sync::Arc;

use crate::types::error::OriginSdkError;
use crate::client::connection::Connection;
use super::builder::DynamicCall;

/// Meta-transaction helper (scaffold).
#[derive(Clone)]
pub struct MetaTxClient {
	connection: Arc<Connection>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	pub fn wrap(&self, _call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		// TODO: construct meta-tx pallet call
		Err(OriginSdkError::Unimplemented("meta-tx wrapper".into()))
	}
}
