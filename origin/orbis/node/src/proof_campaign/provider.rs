use std::{
	error::Error,
	sync::{
		atomic::{AtomicU8, Ordering},
		Arc,
	},
};

use origin_orbis_runtime::Block;
use sc_client_api::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use sp_inherents::InherentDataProvider as _;
use sp_runtime::traits::Block as BlockT;
use sp_transaction_storage_proof::{
	runtime_api::TransactionStorageApi, InherentDataProvider, TransactionStorageProof,
	TransactionStorageProofInherentData,
};

use super::{receipt, service::Client, Campaign, FaultKind};

type BoxError = Box<dyn Error + Send + Sync>;

/// Shared one-shot state machine.  The faulty proposal must be observed as rejected before a
/// canonical recovery proposal is permitted at the same block number.
#[derive(Debug, Default)]
pub struct CampaignLatch(AtomicU8);

impl CampaignLatch {
	const PENDING: u8 = 0;
	const ARMED: u8 = 1;
	const REJECTED: u8 = 2;
	const RECOVERY_PROPOSED: u8 = 3;
	const RECOVERY_IMPORTED: u8 = 4;
	const RECOVERY_FINALIZED: u8 = 5;

	fn arm(&self) -> Result<(), BoxError> {
		self.0
			.compare_exchange(Self::PENDING, Self::ARMED, Ordering::AcqRel, Ordering::Acquire)
			.map(|_| ())
			.map_err(|_| "proof campaign fault has already been armed".into())
	}

	pub fn is_armed(&self) -> bool {
		self.0.load(Ordering::Acquire) == Self::ARMED
	}

	pub fn is_rejected(&self) -> bool {
		self.0.load(Ordering::Acquire) == Self::REJECTED
	}

	pub fn recovery_started(&self) -> bool {
		self.0.load(Ordering::Acquire) >= Self::RECOVERY_PROPOSED
	}

	pub fn mark_rejected(&self) -> Result<(), BoxError> {
		self.transition(Self::ARMED, Self::REJECTED, "fault rejection")
	}

	pub fn mark_recovery_proposed(&self) -> Result<(), BoxError> {
		self.transition(Self::REJECTED, Self::RECOVERY_PROPOSED, "recovery proposal")
	}

	pub fn mark_recovery_imported(&self) -> Result<(), BoxError> {
		self.transition(Self::RECOVERY_PROPOSED, Self::RECOVERY_IMPORTED, "recovery import")
	}

	pub fn mark_recovery_finalized(&self) -> Result<(), BoxError> {
		self.transition(Self::RECOVERY_IMPORTED, Self::RECOVERY_FINALIZED, "recovery finality")
	}

	fn transition(&self, from: u8, to: u8, label: &str) -> Result<(), BoxError> {
		self.0
			.compare_exchange(from, to, Ordering::AcqRel, Ordering::Acquire)
			.map(|_| ())
			.map_err(|actual| {
				format!("invalid proof campaign {label} transition: state={actual}").into()
			})
	}
}

/// Build the proof provider for one prospective authored block.
///
/// The canonical provider is always evaluated first at the target and must contain `Some(proof)`.
/// This makes every campaign case non-vacuous.  A missing canonical proof aborts authoring rather
/// than being mistaken for a successful negative test.
pub async fn create(
	client: Arc<Client>,
	parent: <Block as BlockT>::Hash,
	campaign: Campaign,
	latch: Arc<CampaignLatch>,
) -> Result<Vec<InherentDataProvider>, BoxError> {
	let parent_number = HeaderBackend::<Block>::number(&*client, parent)?
		.ok_or_else(|| format!("proof campaign parent {parent:?} is unknown"))?;
	let authored_number = parent_number.saturating_add(1);

	if authored_number > campaign.target && !latch.recovery_started() {
		return Err(format!(
			"proof campaign passed target {} before a proven recovery proposal",
			campaign.target
		)
		.into());
	}

	let retention = client.runtime_api().retention_period(parent)?;
	let canonical = sp_transaction_storage_proof::registration::new_data_provider(
		&*client, &parent, retention,
	)?;

	if authored_number < campaign.target || authored_number > campaign.target {
		return Ok(vec![canonical]);
	}

	let canonical_proof = match extract(&canonical).await? {
		Some(proof) => proof,
		None => {
			receipt(
				&campaign,
				"canonical-proof-missing",
				serde_json::json!({ "authoring_aborted": true }),
			);
			return Err(format!(
				"proof campaign target {} is vacuous: canonical provider returned None",
				campaign.target
			)
			.into());
		},
	};

	if latch.is_rejected() || latch.recovery_started() {
		receipt(&campaign, "recovery-provider", serde_json::json!({ "canonical_proof": "Some" }));
		return Ok(vec![canonical]);
	}
	if latch.is_armed() {
		return Err(
			"fault proposal is still pending rejection; refusing concurrent target authoring"
				.into(),
		);
	}

	let selected = match campaign.fault {
		FaultKind::Missing => InherentDataProvider::new(None),
		FaultKind::Invalid => InherentDataProvider::new(Some(corrupt(canonical_proof))),
		FaultKind::Stale => {
			let stale_retention = retention.saturating_add(1);
			let stale = sp_transaction_storage_proof::registration::new_data_provider(
				&*client,
				&parent,
				stale_retention,
			)?;
			if extract(&stale).await?.is_none() {
				receipt(
					&campaign,
					"stale-proof-missing",
					serde_json::json!({ "authoring_aborted": true }),
				);
				return Err(format!(
					"proof campaign target {} is vacuous: stale provider returned None",
					campaign.target
				)
				.into());
			}
			stale
		},
		FaultKind::Duplicate => canonical,
	};

	// Arm only after both canonical and selected fault providers have been constructed.
	latch.arm()?;
	receipt(
		&campaign,
		"fault-armed",
		serde_json::json!({
			"canonical_proof": "Some",
			"one_shot": true,
		}),
	);
	Ok(vec![selected])
}

async fn extract(
	provider: &InherentDataProvider,
) -> Result<Option<TransactionStorageProof>, BoxError> {
	let data = provider.create_inherent_data().await?;
	Ok(data.storage_proof()?)
}

fn corrupt(mut proof: TransactionStorageProof) -> TransactionStorageProof {
	if proof.chunk.is_empty() {
		proof.chunk.push(0xA5);
	} else {
		proof.chunk[0] ^= 0xA5;
	}
	proof
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn latch_enforces_rejection_before_recovery() {
		let latch = CampaignLatch::default();
		assert!(!latch.is_armed());
		assert!(latch.arm().is_ok());
		assert!(latch.is_armed());
		assert!(latch.arm().is_err());
		assert!(latch.mark_recovery_proposed().is_err());
		assert!(latch.mark_rejected().is_ok());
		assert!(latch.mark_recovery_proposed().is_ok());
		assert!(latch.mark_recovery_imported().is_ok());
		assert!(latch.mark_recovery_finalized().is_ok());
	}

	#[test]
	fn corruption_is_deterministic_and_non_identity() {
		let proof = TransactionStorageProof { chunk: vec![1, 2], proof: vec![] };
		let changed = corrupt(proof.clone());
		assert_ne!(changed.chunk, proof.chunk);
		assert_eq!(changed.chunk, corrupt(proof).chunk);
	}

	#[test]
	fn empty_chunk_is_still_corrupted() {
		let changed = corrupt(TransactionStorageProof { chunk: vec![], proof: vec![] });
		assert_eq!(changed.chunk, vec![0xA5]);
	}
}
