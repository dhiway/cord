// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

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
				},
				Err(e) => return Err(e),
			}
		}
	}
}
