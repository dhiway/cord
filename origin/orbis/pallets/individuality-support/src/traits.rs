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

mod reality;
pub use crate::members_notifier_subscriber::{OnRingRootChange, RingRootOp, RingRootsProvider};
pub use reality::{
	AddOnlyPeopleTrait, Alias, AllocateStorage, AppendOnlyMembers, AppendOnlyMembersWeightInfo,
	Callback, ClaimCleanupOutcome, CleanUpAlias, CommunicationIdentifier, ConsumerRegistrar,
	Context, ContextualAlias, CountedMembers, CurrentBlockRandomness, EvidenceHash,
	FlexibleMembers, Identifier, IdentityData, InkSpec, Judgement, JudgementContext,
	MembershipMultiProver, MembershipProver, PageIndex, PeopleTrait, PersonalId, PersonhoodLookup,
	PersonhoodProofRequest, ResourceClaimLifecycle, RevisedAlias, RevisedContextualAlias,
	RevisionIndex, RingExponent, RingIndex, RingMembersState, RingMode, RingMutationMode,
	RingPosition, RingSize, RingStatus, Social, Statement, StatementOracle, Truth, TwoPhaseStorage,
	Username, CONTEXT_SIZE, PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER, RI_ZERO,
};
pub use verifiable::BatchProofItem;
