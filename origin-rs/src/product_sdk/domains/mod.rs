//! Typed, transport-neutral contracts for native Orbis application domains.
//!
//! These modules describe semantic runtime API queries and submit-and-finalize commands. They do
//! not contain pallet indices, SCALE encoding, metadata code generation, network transport, or
//! contract-era surfaces. A Subxt adapter must bind them to current runtime metadata.

pub mod attestation;
pub mod common;
pub mod names;
pub mod drive;
pub mod identity_personhood;
pub mod s3;
pub mod storage;
pub mod storage_events;
pub mod storage_provider;

pub use common::{
	AccountId, AgreementId, AttestationId, BlockNumber, BucketId, ChallengeId, ContainerId,
	ContentCommitment, ContentHash, DomainResult, DriveId, FinalizedPage, FinalizedQuery,
	FinalizedValue, Hash32, NameId, ObjectId, PageRequest, PayloadCommitment, ProofCommitment,
	ProviderReference, RegistrationCommitment, ReservationId, ReservationReference, SchemaId,
	StatusCommitment, SubjectCommitment, SubjectId, SubmitAndFinalize, UniquenessCommitment,
	Validate, DOMAIN_CONTRACT_VERSION, MAX_PAGE_SIZE,
};
