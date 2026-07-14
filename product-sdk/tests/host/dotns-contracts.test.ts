import assert from "node:assert/strict";
import test from "node:test";
import { ORBIS_CANDIDATE_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import { dotns, dotnsAddress, normalizedLabel, registrationSalt, textKey, textValue } from "../../src/dotns.ts";
import type { AccountId, AttestationId, BlockNumber, ContentCommitment, NameId, RegistrationCommitment, SubjectId } from "../../src/types.ts";

const owner = "owner:alice" as AccountId;
const target = "owner:bob" as AccountId;
const name = `0x${"11".repeat(32)}` as NameId;
const ref = `0x${"22".repeat(32)}`;
const subject = "3trSqr2ErU2GA8YAzj1YmXfRkg4L4PrMbptvhKTxgv2zdXGL" as SubjectId;
let index = 0;
function context(method: string) { index += 1; return { request_id: `dotns-contract-${String(index).padStart(4, "0")}`, application_id: "festival", network: ORBIS_CANDIDATE_NETWORK_BINDING, consent: { scopes: [`dotns:${method}`], expires_at: 2_000, nonce: `dotns-consent-${String(index).padStart(4, "0")}` } }; }

test("all native DotNS methods have exact finality and executable Host routes", async () => {
  const label = normalizedLabel("alice"); const salt = registrationSalt("secret");
  const requests = [
    dotns.labelPolicyVersion(context("label_policy_version")),
    dotns.nameById(context("name_by_id"), name), dotns.rootNameByNormalizedLabel(context("root_name_by_normalized_label"), label),
    dotns.ownerNames(context("owner_names"), owner, { limit: 0 }), dotns.resolveAddress(context("resolve_address"), name),
    dotns.controllers(context("controllers"), name),
    dotns.resolveSubject(context("resolve_subject"), name), dotns.resolveAttestation(context("resolve_attestation"), name),
    dotns.resolveContent(context("resolve_content"), name), dotns.resolveText(context("resolve_text"), name, textKey("url")),
    dotns.primaryName(context("primary_name"), owner), dotns.nameStatus(context("name_status"), name),
    dotns.commit(context("commit"), ref as RegistrationCommitment), dotns.cancelCommitment(context("cancel_commitment"), ref as RegistrationCommitment),
    dotns.pruneExpiredCommitment(context("prune_expired_commitment"), owner, ref as RegistrationCommitment),
    dotns.register(context("register"), null, label, salt), dotns.renew(context("renew"), name, "10" as BlockNumber),
    dotns.transfer(context("transfer"), name, target), dotns.addController(context("add_controller"), name, target),
    dotns.removeController(context("remove_controller"), name, target), dotns.setAddress(context("set_address"), name, dotnsAddress("cord:alice")),
    dotns.setSubject(context("set_subject"), name, subject), dotns.setAttestation(context("set_attestation"), name, ref as AttestationId),
    dotns.setContent(context("set_content"), name, ref as ContentCommitment), dotns.setText(context("set_text"), name, textKey("url"), textValue("https://example.test")),
    dotns.setPrimaryName(context("set_primary_name"), name), dotns.release(context("release"), name),
    dotns.removeExpiredName(context("remove_expired_name"), name), dotns.reserveName(context("reserve_name"), null, label, owner, "20" as BlockNumber),
    dotns.clearReservation(context("clear_reservation"), name), dotns.setLabelProtection(context("set_label_protection"), label, true),
    dotns.setPaused(context("set_paused"), true), dotns.forceTransfer(context("force_transfer"), name, target), dotns.forceRevoke(context("force_revoke"), name),
    dotns.setRegistrar(context("set_registrar"), target, true),
  ] as const;
  assert.equal(requests.length, 35);
  assert.deepEqual(requests.slice(0, 12).map((r) => r.finality), Array(12).fill("finalized"));
  assert.deepEqual(requests.slice(12).map((r) => r.finality), Array(23).fill("submit-and-finalize"));
  assert.deepEqual(requests[0].payload, {}); assert.deepEqual(requests[3].payload, { owner, cursor: null, limit: 0 });
  const routed: string[] = [];
  const host = new FakeHost({ finalizedRead: async ({ request }) => { routed.push(`read:${request.method}`); return { finalizedHash: ref }; }, submitAndFinalize: async ({ request }) => { routed.push(`write:${request.method}`); return { finalizedHash: ref }; } });
  host.grant("festival", requests.map(({ method }) => `dotns:${method}`));
  for (const request of requests) {
    const hostRequest = request as HostRequest;
    host.issueConsent(hostRequest.application_id, hostRequest.consent);
    await host.execute(hostRequest);
  }
  assert.equal(routed.filter((route) => route.startsWith("read:")).length, 12);
  assert.equal(routed.filter((route) => route.startsWith("write:")).length, 23);
});
