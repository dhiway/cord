// This file is part of CORD - https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finalized Orbis runtime authority binding.

use async_trait::async_trait;
use codec::{Decode, Encode};
use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use orbis_storage_runtime_api::{
	AgreementInfo, AgreementStatus, ChallengeInfo, ChallengeStatus, Page, ProviderInfo,
	ProviderStatus, Versioned, RESPONSE_VERSION,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, H256};

const MAX_CHALLENGE_BLOCK_CATCHUP: u32 = 128;
const API_PAGE_SIZE: u32 = 100;

/// Authorization returned after checking a storage agreement at a finalized block.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgreementAuthorization {
	/// Finalized block used for the decision.
	pub finalized_hash: String,
	/// Agreement identifier.
	pub agreement_id: String,
	/// Provider account authorized by the agreement.
	pub provider: String,
	/// Canonical container reference.
	pub container_ref: String,
	/// Maximum byte length authorized by the agreement.
	pub bytes: u64,
	/// Expiry block.
	pub expires_at: u32,
}

/// Open proof duty discovered from a finalized `StorageProviderApi::challenges_at` query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChallengeDuty {
	/// Challenge identifier passed to `submit_checkpoint`.
	pub challenge_id: String,
	/// Agreement being challenged.
	pub agreement_id: String,
	/// Agreement's canonical raw-content commitment.
	pub content_commitment: String,
	/// Provider root committed by the challenge issuer.
	pub expected_commitment: String,
	/// Inclusive due block.
	pub due_at: u32,
	/// Finalized hash at which the duty was observed.
	pub observed_at: String,
}

/// Bounded finalized challenge scan result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChallengeBatch {
	/// Highest finalized block scanned.
	pub finalized_number: u32,
	/// Hash of that finalized block.
	pub finalized_hash: String,
	/// Highest challenge due-block inspected in the finalized state.
	pub scanned_through: u32,
	/// Open duties belonging to this provider.
	pub duties: Vec<ChallengeDuty>,
}

/// Errors from the finalized chain authority.
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
	/// Transport or JSON-RPC failure.
	#[error("Orbis RPC failed: {0}")]
	Rpc(String),
	/// Runtime API returned malformed SCALE.
	#[error("Orbis runtime API response is invalid: {0}")]
	Decode(String),
	/// Agreement/provider/challenge state rejected the operation.
	#[error("storage authority rejected: {0}")]
	Rejected(String),
}

/// Canonical authority seam for all content mutations and proof duties.
#[async_trait]
pub trait ChainAuthority: Send + Sync + 'static {
	/// Validate a content write against one exact finalized Orbis state.
	async fn authorize_commit(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
		bytes: u64,
	) -> Result<AgreementAuthorization, ChainError>;

	/// Validate deletion against a cancelled or expired finalized agreement.
	async fn authorize_delete(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
	) -> Result<AgreementAuthorization, ChainError>;

	/// Scan a bounded future due-block window in finalized state. The returned safe cursor never
	/// advances past the current finalized block, so newly-created future duties cannot be skipped.
	async fn challenge_duties(
		&self,
		after_block: Option<u32>,
	) -> Result<ChallengeBatch, ChainError>;
}

/// Runtime-API client that validates decisions at `chain_getFinalizedHead`.
pub struct FinalizedRuntimeAuthority {
	client: HttpClient,
	provider: AccountId32,
	service_key: [u8; 32],
}

impl FinalizedRuntimeAuthority {
	/// Connect to an Orbis HTTP JSON-RPC endpoint and bind one provider/service-key pair.
	pub fn connect(
		endpoint: &str,
		provider: AccountId32,
		service_key: [u8; 32],
	) -> Result<Self, ChainError> {
		let client = jsonrpsee::http_client::HttpClientBuilder::default()
			.build(endpoint)
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		Ok(Self { client, provider, service_key })
	}

	async fn finalized_context(&self) -> Result<(String, u32), ChainError> {
		let hash: String = self
			.client
			.request("chain_getFinalizedHead", rpc_params![])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let header: RpcHeader = self
			.client
			.request("chain_getHeader", rpc_params![hash.clone()])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let number = u32::from_str_radix(header.number.trim_start_matches("0x"), 16)
			.map_err(|error| ChainError::Decode(error.to_string()))?;
		self.ensure_provider(&hash).await?;
		Ok((hash, number))
	}

	async fn ensure_provider(&self, hash: &str) -> Result<(), ChainError> {
		let response: Versioned<ProviderInfo<u32>> = self
			.runtime_call("StorageProviderApi_provider", self.provider.encode(), hash)
			.await?;
		ensure_version(response.version)?;
		let provider = response
			.value
			.ok_or_else(|| ChainError::Rejected("provider is not registered".into()))?;
		if provider.status != ProviderStatus::Active {
			return Err(ChainError::Rejected("provider is suspended".into()));
		}
		if provider.service_key.as_slice() != self.service_key {
			return Err(ChainError::Rejected(
				"local service key does not match finalized provider record".into(),
			));
		}
		Ok(())
	}

	async fn agreement(
		&self,
		agreement_id: [u8; 32],
		hash: &str,
	) -> Result<AgreementInfo<AccountId32, H256, u32>, ChainError> {
		let response: Versioned<AgreementInfo<AccountId32, H256, u32>> = self
			.runtime_call("StorageProviderApi_agreement", H256::from(agreement_id).encode(), hash)
			.await?;
		ensure_version(response.version)?;
		let agreement = response
			.value
			.ok_or_else(|| ChainError::Rejected("agreement not found".into()))?;
		if agreement.provider != self.provider {
			return Err(ChainError::Rejected("agreement belongs to another provider".into()));
		}
		Ok(agreement)
	}

	async fn runtime_call<T: Decode>(
		&self,
		method: &str,
		params: Vec<u8>,
		hash: &str,
	) -> Result<T, ChainError> {
		let encoded: String = self
			.client
			.request("state_call", rpc_params![method, format!("0x{}", hex::encode(params)), hash])
			.await
			.map_err(|error| ChainError::Rpc(error.to_string()))?;
		let raw = hex::decode(encoded.trim_start_matches("0x"))
			.map_err(|error| ChainError::Decode(error.to_string()))?;
		T::decode(&mut &raw[..]).map_err(|error| ChainError::Decode(error.to_string()))
	}

	fn authorization(
		&self,
		agreement: AgreementInfo<AccountId32, H256, u32>,
		finalized_hash: String,
	) -> AgreementAuthorization {
		let provider_bytes: &[u8] = self.provider.as_ref();
		AgreementAuthorization {
			finalized_hash,
			agreement_id: format!("{:#x}", agreement.agreement_id),
			provider: format!("0x{}", hex::encode(provider_bytes)),
			container_ref: format!("{:#x}", agreement.container_ref),
			bytes: agreement.bytes,
			expires_at: agreement.expires_at,
		}
	}
}

#[async_trait]
impl ChainAuthority for FinalizedRuntimeAuthority {
	async fn authorize_commit(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
		bytes: u64,
	) -> Result<AgreementAuthorization, ChainError> {
		let (hash, number) = self.finalized_context().await?;
		let agreement = self.agreement(agreement_id, &hash).await?;
		if agreement.status != AgreementStatus::Active || number >= agreement.expires_at {
			return Err(ChainError::Rejected("agreement is not live and active".into()));
		}
		if agreement.content_commitment != H256::from(content_commitment) {
			return Err(ChainError::Rejected("content commitment mismatch".into()));
		}
		if agreement.bytes != bytes {
			return Err(ChainError::Rejected(format!(
				"content length {bytes} does not equal agreement length {}",
				agreement.bytes
			)));
		}
		Ok(self.authorization(agreement, hash))
	}

	async fn authorize_delete(
		&self,
		agreement_id: [u8; 32],
		content_commitment: [u8; 32],
	) -> Result<AgreementAuthorization, ChainError> {
		let (hash, _number) = self.finalized_context().await?;
		let agreement = self.agreement(agreement_id, &hash).await?;
		if agreement.content_commitment != H256::from(content_commitment) {
			return Err(ChainError::Rejected("content commitment mismatch".into()));
		}
		let terminal =
			matches!(agreement.status, AgreementStatus::Cancelled | AgreementStatus::Expired);
		if !terminal {
			return Err(ChainError::Rejected(
				"agreement must be finalized as cancelled or expired before content deletion"
					.into(),
			));
		}
		Ok(self.authorization(agreement, hash))
	}

	async fn challenge_duties(
		&self,
		after_block: Option<u32>,
	) -> Result<ChallengeBatch, ChainError> {
		let (hash, finalized_number) = self.finalized_context().await?;
		let (start, scan_end, safe_cursor) = challenge_scan_window(finalized_number, after_block)?;
		// Future due buckets are deliberately rescanned. Advancing a durable cursor beyond
		// finalized height would skip a duty created later for a due block already inspected in
		// an older state.
		let mut duties = Vec::new();
		if start <= scan_end {
			for block in start..=scan_end {
				let mut cursor = None;
				loop {
					let params = (block, cursor, API_PAGE_SIZE).encode();
					let page: Page<ChallengeInfo<AccountId32, H256, u32>> = self
						.runtime_call("StorageProviderApi_challenges_at", params, &hash)
						.await?;
					ensure_version(page.version)?;
					for challenge in page.items {
						if challenge.provider == self.provider
							&& challenge.status == ChallengeStatus::Open
							&& finalized_number <= challenge.due_at
						{
							let agreement =
								self.agreement(challenge.agreement_id.into(), &hash).await?;
							duties.push(ChallengeDuty {
								challenge_id: format!("{:#x}", challenge.challenge_id),
								agreement_id: format!("{:#x}", challenge.agreement_id),
								content_commitment: format!("{:#x}", agreement.content_commitment),
								expected_commitment: format!(
									"{:#x}",
									challenge.expected_commitment
								),
								due_at: challenge.due_at,
								observed_at: hash.clone(),
							});
						}
					}
					cursor = page.next_cursor;
					if cursor.is_none() {
						break;
					}
				}
			}
		}
		Ok(ChallengeBatch {
			finalized_number,
			finalized_hash: hash,
			scanned_through: safe_cursor,
			duties,
		})
	}
}

fn challenge_scan_window(
	finalized_number: u32,
	after_block: Option<u32>,
) -> Result<(u32, u32, u32), ChainError> {
	if after_block.is_some_and(|cursor| cursor > finalized_number) {
		return Err(ChainError::Rejected(
			"finalized height regressed behind the challenge scan cursor".into(),
		));
	}
	Ok((
		finalized_number,
		finalized_number.saturating_add(MAX_CHALLENGE_BLOCK_CATCHUP),
		finalized_number,
	))
}

fn ensure_version(version: u16) -> Result<(), ChainError> {
	if version == RESPONSE_VERSION {
		Ok(())
	} else {
		Err(ChainError::Rejected(format!(
			"unsupported StorageProviderApi response version {version}"
		)))
	}
}

#[derive(Deserialize)]
struct RpcHeader {
	number: String,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn future_scan_window_never_advances_safe_cursor_past_finality() {
		assert_eq!(challenge_scan_window(50, Some(49)).unwrap(), (50, 178, 50));
		assert!(challenge_scan_window(50, Some(51)).is_err());
		assert_eq!(challenge_scan_window(u32::MAX, None).unwrap(), (u32::MAX, u32::MAX, u32::MAX));
	}
}
