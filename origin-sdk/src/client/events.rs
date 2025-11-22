use std::sync::Arc;

use crate::types::error::OriginSdkError;
use super::connection::Connection;

/// Event streaming client (stub).
#[derive(Clone)]
pub struct EventClient {
	connection: Arc<Connection>,
}

impl EventClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	pub async fn subscribe(&self, _pallet: Option<&str>) -> Result<(), OriginSdkError> {
		// TODO: implement event filters + reconnect
		Ok(())
	}
}
