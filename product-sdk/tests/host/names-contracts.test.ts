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
import { names, namesAddress, normalizedLabel, registrationSalt, textKey, textValue } from "../../src/names.ts";
import type { AccountId, AttestationId, BlockNumber, ContentCommitment, NameId, RegistrationCommitment, SubjectId } from "../../src/types.ts";

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
    names.labelPolicyVersion(context("label_policy_version")),
    names.nameById(context("name_by_id"), name), names.rootNameByNormalizedLabel(context("root_name_by_normalized_label"), label),
    names.ownerNames(context("owner_names"), owner, { limit: 0 }), names.resolveAddress(context("resolve_address"), name),
    names.controllers(context("controllers"), name),
    names.resolveSubject(context("resolve_subject"), name), names.resolveAttestation(context("resolve_attestation"), name),
    names.resolveContent(context("resolve_content"), name), names.resolveText(context("resolve_text"), name, textKey("url")),
    names.primaryName(context("primary_name"), owner), names.nameStatus(context("name_status"), name),
    names.commit(context("commit"), ref as RegistrationCommitment), names.cancelCommitment(context("cancel_commitment"), ref as RegistrationCommitment),
    names.pruneExpiredCommitment(context("prune_expired_commitment"), owner, ref as RegistrationCommitment),
    names.register(context("register"), null, label, salt), names.renew(context("renew"), name, "10" as BlockNumber),
    names.transfer(context("transfer"), name, target), names.addController(context("add_controller"), name, target),
    names.removeController(context("remove_controller"), name, target), names.setAddress(context("set_address"), name, namesAddress("cord:alice")),
    names.setSubject(context("set_subject"), name, subject), names.setAttestation(context("set_attestation"), name, ref as AttestationId),
    names.setContent(context("set_content"), name, ref as ContentCommitment), names.setText(context("set_text"), name, textKey("url"), textValue("https://example.test")),
    names.setPrimaryName(context("set_primary_name"), name), names.release(context("release"), name),
    names.removeExpiredName(context("remove_expired_name"), name), names.reserveName(context("reserve_name"), null, label, owner, "20" as BlockNumber),
    names.clearReservation(context("clear_reservation"), name), names.setLabelProtection(context("set_label_protection"), label, true),
    names.setPaused(context("set_paused"), true), names.forceTransfer(context("force_transfer"), name, target), names.forceRevoke(context("force_revoke"), name),
    names.setRegistrar(context("set_registrar"), target, true),
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
