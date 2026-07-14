use std::sync::Arc;

use codec::Encode;
use futures::{future::BoxFuture, FutureExt};
use origin_commons_runtime::{Block, RuntimeCall};
use sc_block_builder::BlockBuilderBuilder;
use sp_blockchain::{ApplyExtrinsicFailed, Error as BlockchainError};
use sp_consensus::{Environment, Proposal, ProposeArgs, Proposer};
use sp_runtime::{
	traits::{Block as BlockT, ExtrinsicCall, Header as HeaderT},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
};

use super::{
	provider::CampaignLatch,
	receipt,
	service::{Client, Pool},
	Campaign, FaultKind,
};

type BaseEnvironment = sc_basic_authorship::ProposerFactory<Pool, Client>;
type BaseProposer = sc_basic_authorship::Proposer<Block, Client, Pool>;

/// Environment wrapper which performs the Orbis-only duplicate mandatory-inherent probe.
pub struct CampaignEnvironment {
	inner: BaseEnvironment,
	client: Arc<Client>,
	campaign: Campaign,
	latch: Arc<CampaignLatch>,
}

impl Clone for CampaignEnvironment {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			client: self.client.clone(),
			campaign: self.campaign.clone(),
			latch: self.latch.clone(),
		}
	}
}

impl CampaignEnvironment {
	pub fn new(
		inner: BaseEnvironment,
		client: Arc<Client>,
		campaign: Campaign,
		latch: Arc<CampaignLatch>,
	) -> Self {
		Self { inner, client, campaign, latch }
	}
}

impl Environment<Block> for CampaignEnvironment {
	type Proposer = CampaignProposer;
	type CreateProposer = BoxFuture<'static, Result<Self::Proposer, Self::Error>>;
	type Error = BlockchainError;

	fn init(&mut self, parent_header: &<Block as BlockT>::Header) -> Self::CreateProposer {
		let future = self.inner.init(parent_header);
		let client = self.client.clone();
		let campaign = self.campaign.clone();
		let latch = self.latch.clone();
		let parent_hash = parent_header.hash();
		let parent_number = *parent_header.number();
		async move {
			future.await.map(|inner| CampaignProposer {
				inner,
				client,
				campaign,
				latch,
				parent_hash,
				parent_number,
			})
		}
		.boxed()
	}
}

pub struct CampaignProposer {
	inner: BaseProposer,
	client: Arc<Client>,
	campaign: Campaign,
	latch: Arc<CampaignLatch>,
	parent_hash: <Block as BlockT>::Hash,
	parent_number: u32,
}

impl Proposer<Block> for CampaignProposer {
	type Error = BlockchainError;
	type Proposal = BoxFuture<'static, Result<Proposal<Block>, Self::Error>>;

	fn propose(self, args: ProposeArgs<Block>) -> Self::Proposal {
		async move {
			let campaign = self.campaign.clone();
			let latch = self.latch.clone();
			let authored_number = self.parent_number + 1u32;
			let at_target = authored_number == campaign.target;
			let fault_attempt = at_target && latch.is_armed();
			let recovery_attempt = at_target && latch.is_rejected();
			let inherent_digests = args.inherent_digests.clone();
			let result = self.inner.propose(args).await;

			if recovery_attempt {
				return match result {
					Ok(proposal) => {
						latch.mark_recovery_proposed().map_err(|error| fatal(error.to_string()))?;
						receipt(
							&campaign,
							"recovery-proposed",
							serde_json::json!({ "canonical": true }),
						);
						Ok(proposal)
					},
					Err(error) => Err(error),
				};
			}

			if !fault_attempt {
				return result;
			}

			if campaign.fault != FaultKind::Duplicate {
				return match result {
					Err(error) => {
						mark_expected_rejection(&campaign, &latch, error.to_string())?;
						Err(error)
					},
					Ok(_) => Err(fatal(
						"faulty proof unexpectedly produced a proposal; publication aborted",
					)),
				};
			}

			let proposal = result.map_err(|error| {
				fatal(format!("duplicate canonical proposal failed before probe: {error}"))
			})?;

			probe_duplicate(
				&*self.client,
				self.parent_hash,
				self.parent_number,
				inherent_digests,
				proposal.block,
			)?;

			mark_expected_rejection(&campaign, &latch, "BadMandatory")?;
			Err(fatal(
				"expected duplicate BadMandatory observed; publication intentionally aborted",
			))
		}
		.boxed()
	}
}

fn mark_expected_rejection(
	campaign: &Campaign,
	latch: &CampaignLatch,
	reason: impl Into<String>,
) -> Result<(), BlockchainError> {
	latch.mark_rejected().map_err(|error| fatal(error.to_string()))?;
	receipt(
		campaign,
		"fault-rejected-before-proposal",
		serde_json::json!({
			"reason": reason.into(),
			"proposal_returned": false,
			"fault_block_imported": false,
			"fault_block_finalized": false,
		}),
	);
	Ok(())
}

fn probe_duplicate(
	client: &Client,
	parent_hash: <Block as BlockT>::Hash,
	parent_number: u32,
	inherent_digests: sp_runtime::Digest,
	block: Block,
) -> Result<(), BlockchainError> {
	let (_, extrinsics) = block.deconstruct();
	let duplicate = extrinsics
		.iter()
		.find(|extrinsic| is_orbis_proof_inherent(extrinsic))
		.cloned()
		.ok_or_else(|| fatal("canonical Some(proof) inherent missing from authored Orbis block"))?;

	let mut builder = BlockBuilderBuilder::<Block, Client>::new(client)
		.on_parent_block(parent_hash)
		.with_parent_block_number(parent_number)
		.with_inherent_digests(inherent_digests)
		.build()?;
	for extrinsic in extrinsics {
		builder.push(extrinsic)?;
	}

	match builder.push(duplicate) {
		Err(BlockchainError::ApplyExtrinsicFailed(ApplyExtrinsicFailed::Validity(
			TransactionValidityError::Invalid(InvalidTransaction::BadMandatory),
		))) => Ok(()),
		Ok(()) => Err(fatal("duplicate mandatory inherent unexpectedly succeeded")),
		Err(error) => Err(fatal(format!(
			"duplicate mandatory inherent returned {error:?}; expected BadMandatory"
		))),
	}
}

/// Runtime-specific identification: Orbis pallet index 110, call index 14, and SCALE `Some`.
/// The concrete `RuntimeCall` match prevents these bytes from being interpreted against another
/// runtime; the byte checks then avoid a direct node dependency on the pallet implementation.
fn is_orbis_proof_inherent(extrinsic: &origin_commons_runtime::UncheckedExtrinsic) -> bool {
	if !matches!(extrinsic.call(), RuntimeCall::TransactionStorage(_)) {
		return false;
	}
	let encoded = extrinsic.call().encode();
	encoded.starts_with(&[110, 14, 1])
}

fn fatal(message: impl Into<String>) -> BlockchainError {
	BlockchainError::Backend(format!("orbis proof campaign: {}", message.into()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn bad_mandatory_match_is_exact() {
		let expected = BlockchainError::ApplyExtrinsicFailed(ApplyExtrinsicFailed::Validity(
			TransactionValidityError::Invalid(InvalidTransaction::BadMandatory),
		));
		assert!(matches!(
			expected,
			BlockchainError::ApplyExtrinsicFailed(ApplyExtrinsicFailed::Validity(
				TransactionValidityError::Invalid(InvalidTransaction::BadMandatory)
			))
		));
	}

	#[test]
	fn mandatory_validation_is_not_accepted_as_bad_mandatory() {
		let wrong = TransactionValidityError::Invalid(InvalidTransaction::MandatoryValidation);
		assert!(!matches!(
			wrong,
			TransactionValidityError::Invalid(InvalidTransaction::BadMandatory)
		));
	}
}
