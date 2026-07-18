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

//! Typed, transport-neutral contracts for native Orbis application domains.
//!
//! These modules describe semantic runtime API queries and submit-and-finalize commands. They do
//! not contain pallet indices, SCALE encoding, metadata code generation, network transport, or
//! contract-era surfaces. A Subxt adapter must bind them to current runtime metadata.

pub mod attestation;
pub mod common;
pub mod drive;
#[allow(dead_code)]
pub(crate) mod identity_personhood;
pub mod identity_v2;
pub mod names;
pub mod s3;
pub mod storage_events;
pub mod storage_provider;

pub use common::{
	AccountId, AgreementId, AttestationId, BlockNumber, BucketId, ChallengeId, ContainerId,
	ContentCommitment, ContentHash, DomainResult, DriveId, FinalizedPage, FinalizedQuery,
	FinalizedValue, Hash32, NameId, ObjectId, OperationId, PageRequest, PayloadCommitment,
	ProofCommitment, ProviderReference, RegistrationCommitment, ReservationId,
	ReservationReference, SchemaId, StatusCommitment, SubjectCommitment, SubjectId,
	SubmitAndFinalize, UniquenessCommitment, Validate, DOMAIN_CONTRACT_VERSION, MAX_PAGE_SIZE,
};
