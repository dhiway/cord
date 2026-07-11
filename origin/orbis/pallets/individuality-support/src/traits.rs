// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod reality;
pub use crate::members_notifier_subscriber::{OnRingRootChange, RingRootOp, RingRootsProvider};
pub use reality::{
	AddOnlyPeopleTrait, Alias, AllocateStorage, AppendOnlyMembers, AppendOnlyMembersWeightInfo,
	Callback, CleanUpAlias, CommunicationIdentifier, ConsumerRegistrar, Context, ContextualAlias,
	CountedMembers, CurrentBlockRandomness, EvidenceHash, FlexibleMembers, Identifier,
	IdentityData, InkSpec, Judgement, JudgementContext, MembershipMultiProver, MembershipProver,
	PageIndex, PeopleTrait, PersonalId, PersonhoodLookup, PersonhoodProofRequest, RevisedAlias,
	RevisedContextualAlias, RevisionIndex, RingExponent, RingIndex, RingMembersState, RingMode,
	RingMutationMode, RingPosition, RingSize, RingStatus, Social, Statement, StatementOracle,
	Truth, Username, CONTEXT_SIZE, PEOPLE_IDENTIFIER, PEOPLE_LITE_IDENTIFIER, RI_ZERO,
};
pub use verifiable::BatchProofItem;
