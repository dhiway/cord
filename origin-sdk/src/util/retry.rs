use std::time::Duration;

/// Simple retry/backoff policy.
#[derive(Clone, Debug)]
pub struct RetryPolicy {
	pub max_retries: usize,
	pub base_delay: Duration,
}

impl Default for RetryPolicy {
	fn default() -> Self {
		Self { max_retries: 3, base_delay: Duration::from_millis(200) }
	}
}

impl RetryPolicy {
	pub async fn retry<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
	where
		F: FnMut() -> Fut,
		Fut: std::future::Future<Output = Result<T, E>>,
	{
		let mut attempt = 0;
		loop {
			match f().await {
				Ok(v) => return Ok(v),
				Err(_e) if attempt < self.max_retries => {
					attempt += 1;
					tokio::time::sleep(self.base_delay * attempt as u32).await;
				}
				Err(e) => return Err(e),
			}
		}
	}
}
