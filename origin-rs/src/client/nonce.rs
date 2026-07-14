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

use std::{
	collections::HashMap,
	sync::Arc,
	time::{Duration, Instant},
};

use tokio::sync::Mutex;

use crate::{config::OriginConfig, tx::config::NonceMode, types::error::OriginSdkError};

#[derive(Debug)]
struct NonceState {
	next: Option<u64>,
	last_refresh: Instant,
}

#[derive(Debug)]
struct AccountNonce {
	inner: Mutex<NonceState>,
}

/// Central nonce allocator shared by all tx queues.
#[derive(Debug)]
pub struct NonceManager {
	mode: NonceMode,
	refresh_after: Duration,
	accounts: Mutex<HashMap<[u8; 32], Arc<AccountNonce>>>,
}

impl NonceManager {
	pub fn new(mode: NonceMode, refresh_after: Duration) -> Self {
		Self { mode, refresh_after, accounts: Mutex::new(HashMap::new()) }
	}

	async fn account_entry(&self, account: &origin_primitives::AccountId) -> Arc<AccountNonce> {
		let key: [u8; 32] = account.clone().into();
		let mut guard = self.accounts.lock().await;
		guard
			.entry(key)
			.or_insert_with(|| {
				Arc::new(AccountNonce {
					inner: Mutex::new(NonceState {
						next: None,
						last_refresh: Instant::now() - self.refresh_after,
					}),
				})
			})
			.clone()
	}

	pub async fn allocate(
		&self,
		client: &subxt::OnlineClient<OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		match self.mode {
			NonceMode::RpcPerTx => {
				let account_nonce = self.account_entry(account).await;
				let mut guard = account_nonce.inner.lock().await;
				let fresh = self.fetch(client, account).await?;
				guard.next = Some(fresh.saturating_add(1));
				Ok(fresh)
			},
			NonceMode::LocalCache => {
				let account_nonce = self.account_entry(account).await;
				let mut guard = account_nonce.inner.lock().await;
				if guard.next.is_none() || guard.last_refresh.elapsed() >= self.refresh_after {
					let fresh = self.fetch(client, account).await?;
					guard.next = Some(fresh);
					guard.last_refresh = Instant::now();
				}
				let nonce = guard.next.unwrap_or(0);
				guard.next = Some(nonce.saturating_add(1));
				Ok(nonce)
			},
		}
	}

	pub async fn refresh(
		&self,
		client: &subxt::OnlineClient<OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		let fresh = self.fetch(client, account).await?;
		let account_nonce = self.account_entry(account).await;
		let mut guard = account_nonce.inner.lock().await;
		guard.next = Some(fresh);
		guard.last_refresh = Instant::now();
		Ok(fresh)
	}

	async fn fetch(
		&self,
		client: &subxt::OnlineClient<OriginConfig>,
		account: &origin_primitives::AccountId,
	) -> Result<u64, OriginSdkError> {
		client
			.tx()
			.account_nonce(account)
			.await
			.map_err(|e| OriginSdkError::Nonce(e.to_string()))
	}
}
