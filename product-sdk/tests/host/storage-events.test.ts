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
import { decodeStorageNativeEvent, subscribeStorageNativeEvents } from "../../packages/host/src/storage-events.ts";
import { storageNativeEventSubscription, type StorageNativeEventKind } from "@cord-network/origin-sdk-cloud-storage";
import type { TypedFinalizedEventSource } from "../../packages/host/src/attestation-events.ts";
import type { BlockHash } from "@cord-network/origin-sdk-cloud-storage";

const h = (n: number) => `0x${n.toString(16).padStart(2, "0").repeat(32)}`;
const a = (n: number) => `account-${n}`;
type Case = readonly [string, string, Readonly<Record<string, unknown>>, StorageNativeEventKind];
const cases: readonly Case[] = [
  ["StorageProvider", "ProviderRegistered", { provider: a(1), capacity_bytes: 2 }, "provider_registered"],
  ["StorageProvider", "ProviderUpdated", { provider: a(1), capacity_bytes: "3" }, "provider_updated"],
  ["StorageProvider", "ProviderStatusChanged", { provider: a(1), status: { type: "Active" } }, "provider_status_changed"],
  ["StorageProvider", "ProviderRemoved", { provider: a(1) }, "provider_removed"],
  ["StorageProvider", "Heartbeat", { provider: a(1), at: 2 }, "heartbeat"],
  ["StorageProvider", "BucketCreated", { bucket_id: h(1), owner: a(2), primary: a(1), replicas: [a(3), a(4)], version: 1 }, "storage_bucket_created"],
  ["StorageProvider", "BucketGrantChanged", { bucket_id: h(1), account: a(5), role: "Writer", previous_version: 1, new_version: 2 }, "storage_bucket_grant_changed"],
  ["StorageProvider", "AgreementTransitioned", { agreement_id: h(2), previous: null, current: "Proposed", previous_version: 0, new_version: 1 }, "agreement_transitioned"],
  ["StorageProvider", "AgreementCapacityReleased", { agreement_id: h(2) }, "agreement_capacity_released"],
  ["StorageProvider", "AgreementProviderRebound", { agreement_id: h(2), old_provider: a(1), new_provider: a(3), status: "Active", bytes: 12 }, "agreement_provider_rebound"],
  ["StorageProvider", "ChallengeIssued", { challenge_id: h(3), bucket_id: h(1), provider: a(1), due_at: 4 }, "challenge_issued"],
  ["StorageProvider", "ChallengeProved", { challenge_id: h(3), provider: a(1) }, "challenge_proved"],
  ["StorageProvider", "ChallengeTimedOut", { challenge_id: h(3), provider: a(1), checkpoint: 5 }, "challenge_timed_out"],
  ["StorageProvider", "CheckpointAccepted", { bucket_id: h(1), commitment: { mmr_root: h(4), start_seq: 2, leaf_count: 3 }, checkpoint: 6, replica_confirmations: [a(3), a(4)] }, "checkpoint_accepted"],
  ["StorageProvider", "CheckpointEquivocation", { code: 7, bucket_id: h(1), provider: a(1), accepted_root: h(4), conflicting_root: h(5), nonce: 6 }, "checkpoint_equivocation"],
  ["StorageProvider", "ReplicaSelected", { bucket_id: h(1), provider: a(3), checkpoint: 6 }, "replica_selected"],
  ["StorageProvider", "PrimaryPromoted", { bucket_id: h(1), old_provider: a(1), new_provider: a(3), checkpoint: 7 }, "primary_promoted"],
  ["StorageProvider", "BucketReplicaReplaced", { bucket_id: h(1), old_provider: a(4), new_provider: a(5), previous_version: 2, new_version: 3 }, "bucket_replica_replaced"],
  ["StorageProvider", "ManifestCommitmentChanged", { manifest: h(6), bucket_id: h(1), state: "Publishable", checkpoint: 7 }, "manifest_commitment_changed"],
  ["StorageProvider", "ManifestDeletionAcknowledged", { manifest: h(6), bucket_id: h(1), provider: a(3), evidence_hash: h(7), acknowledged_at: 8 }, "manifest_deletion_acknowledged"],
  ["Drive", "DriveCreated", { drive_id: h(8), owner: a(2), version: 1 }, "drive_created"],
  ["Drive", "DriveRootUpdated", { drive_id: h(8), previous_root: null, new_root: h(6), previous_version: 1, version: 2 }, "drive_root_updated"],
  ["Drive", "GrantChanged", { drive_id: h(8), subject: a(3), role: "Reader", previous_version: 2, version: 3 }, "drive_grant_changed"],
  ["Drive", "DriveTransferred", { drive_id: h(8), old_owner: a(2), new_owner: a(3), previous_version: 3, version: 4 }, "drive_transferred"],
  ["Drive", "DriveArchived", { drive_id: h(8), previous_version: 4, version: 5 }, "drive_archived"],
  ["Drive", "NodeWritten", { drive_id: h(8), path: [100, 105, 114, 47, 102], kind: "File", previous_version: 5, version: 6 }, "drive_node_written"],
  ["Drive", "NodeRemoved", { drive_id: h(8), path: "dir/f", previous_version: 6, version: 7 }, "drive_node_removed"],
  ["S3", "BucketCreated", { bucket: h(9), name: [98], owner: a(2) }, "s3_bucket_created"],
  ["S3", "ControllerChanged", { bucket: h(9), controller: a(3), enabled: true, version: 2 }, "s3_controller_changed"],
  ["S3", "BucketTransferred", { bucket: h(9), from: a(2), to: a(3), version: 3 }, "s3_bucket_transferred"],
  ["S3", "BucketArchived", { bucket: h(9), archived: true, version: 4 }, "s3_bucket_archived"],
  ["S3", "BucketVersioningChanged", { bucket: h(9), enabled: true, version: 5 }, "s3_bucket_versioning_changed"],
  ["S3", "ObjectPut", { bucket: h(9), object: h(10), key: [107], content_hash: h(11), version: 1 }, "s3_object_put"],
  ["S3", "ObjectDeleted", { bucket: h(9), object: h(10), key: [107], version: 2 }, "s3_object_deleted"],
  ["S3", "ObjectPurged", { bucket: h(9), key: [107] }, "s3_object_purged"],
  ["S3", "BucketDeleted", { bucket: h(9), name: [98], owner: a(3) }, "s3_bucket_deleted"],
  ["S3", "ObjectHistoryPruned", { bucket: h(9), key: [107], through_version: 2, removed: 1 }, "s3_object_history_pruned"],
];

test("every exposed Commons storage event decodes to one exact distinct kind", () => {
  const found = new Set<StorageNativeEventKind>();
  cases.forEach(([pallet, event, data, kind], index) => {
    const decoded = decodeStorageNativeEvent({ pallet, event, data, index });
    assert.equal(decoded?.event, kind);
    found.add(decoded!.event);
  });
  assert.equal(found.size, 37);
  assert.equal(decodeStorageNativeEvent({ pallet: "System", event: "ExtrinsicSuccess", data: {}, index: 38 }), null);
  assert.equal(decodeStorageNativeEvent({ pallet: "StorageProvider", event: "ServiceKeyRotated", data: {}, index: 39 }), null);
  assert.throws(() => decodeStorageNativeEvent({ pallet: "S3", event: "FutureEvent", data: {}, index: 0 }), /unknown native storage event/);
  assert.throws(() => decodeStorageNativeEvent({ pallet: "StorageProvider", event: "CheckpointAccepted", data: { ...cases[13]![2], replica_confirmations: [a(3), a(3)] }, index: 0 }), /duplicate accounts/);
});

test("finalized source filters exact kinds and emits deterministic domain outcomes", async () => {
  const source: TypedFinalizedEventSource = {
    async *subscribeFinalizedEvents() {
      yield { hash: h(12), events: cases.map(([pallet, event, data], index) => ({ pallet, event, data, index })) };
    },
  };
  const wanted: StorageNativeEventKind[] = ["checkpoint_accepted", "manifest_deletion_acknowledged", "s3_object_put"];
  const subscription = storageNativeEventSubscription(h(1) as BlockHash, wanted);
  const received = [];
  for await (const item of subscribeStorageNativeEvents(source, subscription)) received.push(item);
  assert.deepEqual(received.map((item) => item.event.event.event), wanted);
  assert.deepEqual(received.map((item) => item.event.event_index), [13, 19, 32]);
  assert.deepEqual(received.map((item) => item.outcome.outcome), ["checkpoint", "manifest", "s3_object"]);
});
