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

import assert from "node:assert/strict";
import test from "node:test";
import { ORBIS_CANDIDATE_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import { namesAddress, normalizedLabel, registrationSalt, textKey, textValue, type AccountId, type AttestationId, type BlockNumber, type ContentCommitment, type NameId, type RegistrationCommitment, type SubjectId } from "@cord-network/origin-sdk-names";
import { namesHostRoutes } from "../../packages/descriptors/src/names-host-routes.ts";

const owner = "owner:alice" as AccountId;
const target = "owner:bob" as AccountId;
const name = `0x${"11".repeat(32)}` as NameId;
const ref = `0x${"22".repeat(32)}`;
const subject = "3trSqr2ErU2GA8YAzj1YmXfRkg4L4PrMbptvhKTxgv2zdXGL" as SubjectId;
let index = 0;
function context(method: string) { index += 1; return { request_id: `names-contract-${String(index).padStart(4, "0")}`, application_id: "festival", network: ORBIS_CANDIDATE_NETWORK_BINDING, consent: { scopes: [`names:${method}`], expires_at: 2_000, nonce: `names-consent-${String(index).padStart(4, "0")}` } }; }

test("all native Orbis Names methods have exact finality and executable Host routes", async () => {
  const label = normalizedLabel("alice"); const salt = registrationSalt("secret");
  const requests = [
    namesHostRoutes.labelPolicyVersion(context("label_policy_version")),
    namesHostRoutes.nameById(context("name_by_id"), name), namesHostRoutes.rootNameByNormalizedLabel(context("root_name_by_normalized_label"), label),
    namesHostRoutes.ownerNames(context("owner_names"), owner, { limit: 0 }), namesHostRoutes.resolveAddress(context("resolve_address"), name),
    namesHostRoutes.controllers(context("controllers"), name),
    namesHostRoutes.resolveSubject(context("resolve_subject"), name), namesHostRoutes.resolveAttestation(context("resolve_attestation"), name),
    namesHostRoutes.resolveContent(context("resolve_content"), name), namesHostRoutes.resolveText(context("resolve_text"), name, textKey("url")),
    namesHostRoutes.primaryName(context("primary_name"), owner), namesHostRoutes.nameStatus(context("name_status"), name),
    namesHostRoutes.commit(context("commit"), ref as RegistrationCommitment), namesHostRoutes.cancelCommitment(context("cancel_commitment"), ref as RegistrationCommitment),
    namesHostRoutes.pruneExpiredCommitment(context("prune_expired_commitment"), owner, ref as RegistrationCommitment),
    namesHostRoutes.register(context("register"), null, label, salt), namesHostRoutes.renew(context("renew"), name, "10" as BlockNumber),
    namesHostRoutes.transfer(context("transfer"), name, target), namesHostRoutes.addController(context("add_controller"), name, target),
    namesHostRoutes.removeController(context("remove_controller"), name, target), namesHostRoutes.setAddress(context("set_address"), name, namesAddress("cord:alice")),
    namesHostRoutes.setSubject(context("set_subject"), name, subject), namesHostRoutes.setAttestation(context("set_attestation"), name, ref as AttestationId),
    namesHostRoutes.setContent(context("set_content"), name, ref as ContentCommitment), namesHostRoutes.setText(context("set_text"), name, textKey("url"), textValue("https://example.test")),
    namesHostRoutes.setPrimaryName(context("set_primary_name"), name), namesHostRoutes.release(context("release"), name),
    namesHostRoutes.removeExpiredName(context("remove_expired_name"), name), namesHostRoutes.reserveName(context("reserve_name"), null, label, owner, "20" as BlockNumber),
    namesHostRoutes.clearReservation(context("clear_reservation"), name), namesHostRoutes.setLabelProtection(context("set_label_protection"), label, true),
    namesHostRoutes.setPaused(context("set_paused"), true), namesHostRoutes.forceTransfer(context("force_transfer"), name, target), namesHostRoutes.forceRevoke(context("force_revoke"), name),
    namesHostRoutes.setRegistrar(context("set_registrar"), target, true),
  ] as const;
  assert.equal(requests.length, 35);
  assert.deepEqual(requests.slice(0, 12).map((r) => r.finality), Array(12).fill("finalized"));
  assert.deepEqual(requests.slice(12).map((r) => r.finality), Array(23).fill("submit-and-finalize"));
  assert.deepEqual(requests[0].payload, {}); assert.deepEqual(requests[3].payload, { owner, cursor: null, limit: 0 });
  const routed: string[] = [];
  const host = new FakeHost({ finalizedRead: async ({ request }) => { routed.push(`read:${request.method}`); return { finalizedHash: ref }; }, submitAndFinalize: async ({ request }) => { routed.push(`write:${request.method}`); return { finalizedHash: ref }; } });
  host.grant("festival", requests.map(({ method }) => `names:${method}`));
  for (const request of requests) {
    const hostRequest = request as HostRequest;
    host.issueConsent(hostRequest.application_id, hostRequest.consent);
    await host.execute(hostRequest);
  }
  assert.equal(routed.filter((route) => route.startsWith("read:")).length, 12);
  assert.equal(routed.filter((route) => route.startsWith("write:")).length, 23);
});
