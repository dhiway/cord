use std::{sync::Arc, time::Duration};

use super::OriginConfig;
use crate::types::error::OriginSdkError;
use crate::util::retry::RetryPolicy;

/// Shared connection wrapper.
#[derive(Clone)]
pub struct Connection {
	api: subxt::OnlineClient<OriginConfig>,
	#[allow(dead_code)]
	endpoint: String,
	#[allow(dead_code)]
	backoff: RetryPolicy,
}

impl Connection {
	pub(crate) async fn connect(
		endpoint: String,
		backoff: RetryPolicy,
		timeout: Duration,
		auto_reconnect: bool,
	) -> Result<Self, OriginSdkError> {
		let _ = (timeout, auto_reconnect); // reserved for future logic
		let api = backoff
			.retry(|| {
				let endpoint = endpoint.clone();
				async move {
					let client =
						subxt::OnlineClient::<OriginConfig>::from_url(endpoint.as_str()).await?;
					Ok::<_, subxt::Error>(client)
				}
			})
			.await
			.map_err(|e| OriginSdkError::Connection(e.to_string()))?;
		Ok(Self { api, endpoint, backoff })
	}

	pub fn metadata(&self) -> subxt::Metadata {
		self.api.metadata()
	}

	pub fn online(&self) -> &subxt::OnlineClient<OriginConfig> {
		&self.api
	}

	#[allow(dead_code)]
	pub fn endpoint(&self) -> &str {
		&self.endpoint
	}

	#[allow(dead_code)]
	pub fn backoff(&self) -> &RetryPolicy {
		&self.backoff
	}
}

/// Fluent builder for `OriginClient`.
pub struct ConnectionBuilder {
	endpoint: Option<String>,
	backoff: RetryPolicy,
	timeout: Duration,
	auto_reconnect: bool,
}

impl Default for ConnectionBuilder {
	fn default() -> Self {
		Self {
			endpoint: None,
			backoff: RetryPolicy::default(),
			timeout: Duration::from_secs(30),
			auto_reconnect: true,
		}
	}
}

impl ConnectionBuilder {
	pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
		self.endpoint = Some(endpoint.into());
		self
	}

	pub fn backoff(mut self, backoff: RetryPolicy) -> Self {
		self.backoff = backoff;
		self
	}

	pub fn timeout(mut self, timeout: Duration) -> Self {
		self.timeout = timeout;
		self
	}

	pub fn auto_reconnect(mut self, enabled: bool) -> Self {
		self.auto_reconnect = enabled;
		self
	}

	pub async fn build(self) -> Result<super::OriginClient, OriginSdkError> {
		let endpoint = self
			.endpoint
			.ok_or_else(|| OriginSdkError::InvalidInput("endpoint is required".into()))?;
		let connection =
			Connection::connect(endpoint, self.backoff, self.timeout, self.auto_reconnect).await?;
		let connection = Arc::new(connection);
		Ok(super::OriginClient { connection, signer: None })
	}
}
