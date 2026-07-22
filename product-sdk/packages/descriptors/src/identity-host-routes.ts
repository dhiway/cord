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

import type {
  AccountId,
  Hash32,
  IdentityInfo,
  IdentityJudgement,
} from "@cord-network/origin-sdk-identity";
import type { LitePersonAttestation } from "@cord-network/origin-sdk-personhood";
import { finalizedRead, submitAndFinalize, type RequestContext } from "../../../src/host.ts";

/** Exact host envelopes for native People identity routes. */
export const identityHostRoutes = {
  identityStatus(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "identity_status", { account });
  },
  setIdentity(context: RequestContext, info: IdentityInfo) {
    return submitAndFinalize("identity", context, "identity", "set_identity", {
      info: { ...info, additional: info.additional.map(({ key, value }) => ({ key, value })) },
    });
  },
  clearIdentity(context: RequestContext) {
    return submitAndFinalize("identity", context, "identity", "clear_identity", {});
  },
  requestJudgement(context: RequestContext, registrar: AccountId) {
    return submitAndFinalize("identity", context, "identity", "request_judgement", { registrar });
  },
  cancelJudgementRequest(context: RequestContext, registrar: AccountId) {
    return submitAndFinalize("identity", context, "identity", "cancel_judgement_request", { registrar });
  },
  provideJudgement(
    context: RequestContext,
    target: AccountId,
    judgement: IdentityJudgement,
    identity_hash: Hash32,
  ) {
    return submitAndFinalize("identity", context, "identity", "provide_judgement", {
      target,
      judgement,
      identity_hash,
    });
  },
} as const;

/** Exact host envelopes for native personhood and PeopleLite routes. */
export const personhoodHostRoutes = {
  personhoodStatus(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "personhood_status", { account });
  },
  attestationAllowance(context: RequestContext, account: AccountId) {
    return finalizedRead("identity", context, "identity", "attestation_allowance", { account });
  },
  attestLitePerson(context: RequestContext, input: LitePersonAttestation) {
    return submitAndFinalize("identity", context, "identity", "attest_lite_person", { ...input });
  },
} as const;
