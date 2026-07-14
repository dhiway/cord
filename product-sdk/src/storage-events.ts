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

import { invalidDomainInput } from "./errors.ts";
import type { AccountId, AgreementId, BlockHash, BlockNumber, BucketId, ChallengeId, ContentCommitment, DecimalU64, DriveId, ObjectId, ProviderId } from "./types.ts";

export type StorageNativeEventKind =
  | "provider_registered" | "provider_updated" | "provider_status_changed" | "provider_removed" | "heartbeat"
  | "agreement_proposed" | "agreement_accepted" | "agreement_cancelled" | "agreement_renewal_requested" | "agreement_renewed" | "agreement_expired" | "agreement_pruned"
  | "challenge_issued" | "checkpoint_submitted" | "challenge_timed_out" | "provider_root_committed" | "deletion_acknowledged"
  | "drive_created" | "drive_root_updated" | "drive_controller_changed" | "drive_transferred" | "drive_archived"
  | "bucket_created" | "bucket_controller_changed" | "bucket_transferred" | "bucket_archived" | "bucket_versioning_changed" | "object_put" | "object_deleted" | "bucket_deleted";

type E<K extends StorageNativeEventKind,D>={readonly event:K;readonly data:Readonly<D>};
export type StorageNativeEvent =
 | E<"provider_registered",{provider:ProviderId;capacity_bytes:DecimalU64}> | E<"provider_updated",{provider:ProviderId;capacity_bytes:DecimalU64}> | E<"provider_status_changed",{provider:ProviderId;status:"active"|"suspended"}> | E<"provider_removed",{provider:ProviderId}> | E<"heartbeat",{provider:ProviderId;at:BlockNumber}>
 | E<"agreement_proposed",{agreement:AgreementId;owner:AccountId;provider:ProviderId}> | E<"agreement_accepted",{agreement:AgreementId}> | E<"agreement_cancelled",{agreement:AgreementId}> | E<"agreement_renewal_requested",{agreement:AgreementId;expires_at:BlockNumber}> | E<"agreement_renewed",{agreement:AgreementId;expires_at:BlockNumber}> | E<"agreement_expired",{agreement:AgreementId}> | E<"agreement_pruned",{agreement:AgreementId}>
 | E<"challenge_issued",{challenge:ChallengeId;provider:ProviderId;due_at:BlockNumber}> | E<"checkpoint_submitted",{challenge:ChallengeId;proof_commitment:ContentCommitment}> | E<"challenge_timed_out",{challenge:ChallengeId;provider:ProviderId}> | E<"provider_root_committed",{provider:ProviderId;sequence:DecimalU64;root:ContentCommitment;leaf_count:DecimalU64}> | E<"deletion_acknowledged",{agreement:AgreementId;provider:ProviderId;content_commitment:ContentCommitment;tombstone_root:ContentCommitment;root_sequence:DecimalU64;leaf_index:DecimalU64;leaf_count:DecimalU64;proof_commitment:ContentCommitment}>
 | E<"drive_created",{drive:DriveId;owner:AccountId}> | E<"drive_root_updated",{drive:DriveId;version:DecimalU64}> | E<"drive_controller_changed",{drive:DriveId;controller:AccountId;enabled:boolean}> | E<"drive_transferred",{drive:DriveId;old_owner:AccountId;new_owner:AccountId}> | E<"drive_archived",{drive:DriveId}>
 | E<"bucket_created",{bucket:BucketId;name:string;owner:AccountId}> | E<"bucket_controller_changed",{bucket:BucketId;controller:AccountId;enabled:boolean;version:DecimalU64}> | E<"bucket_transferred",{bucket:BucketId;from:AccountId;to:AccountId;version:DecimalU64}> | E<"bucket_archived",{bucket:BucketId;archived:boolean;version:DecimalU64}> | E<"bucket_versioning_changed",{bucket:BucketId;enabled:boolean;version:DecimalU64}> | E<"object_put",{bucket:BucketId;object:ObjectId;key:string;content_commitment:ContentCommitment;version:DecimalU64}> | E<"object_deleted",{bucket:BucketId;object:ObjectId;key:string;version:DecimalU64}> | E<"bucket_deleted",{bucket:BucketId;name:string;owner:AccountId}>;

export type StorageLifecycleAction="registered"|"updated"|"status_changed"|"removed"|"heartbeat"|"proposed"|"accepted"|"cancelled"|"renewal_requested"|"renewed"|"expired"|"pruned"|"issued"|"checkpoint_submitted"|"timed_out"|"root_committed"|"deletion_acknowledged"|"created"|"root_updated"|"controller_changed"|"transferred"|"archived"|"versioning_changed"|"put"|"deleted";
export type StorageNativeOutcome=
 | {readonly outcome:"provider";readonly data:{provider:ProviderId;action:StorageLifecycleAction}}
 | {readonly outcome:"agreement";readonly data:{agreement:AgreementId;action:StorageLifecycleAction}}
 | {readonly outcome:"challenge";readonly data:{challenge:ChallengeId;action:StorageLifecycleAction}}
 | {readonly outcome:"drive";readonly data:{drive:DriveId;action:StorageLifecycleAction;version:DecimalU64|null}}
 | {readonly outcome:"bucket";readonly data:{bucket:BucketId;action:StorageLifecycleAction;version:DecimalU64|null}}
 | {readonly outcome:"object";readonly data:{bucket:BucketId;object:ObjectId;action:StorageLifecycleAction;version:DecimalU64}};
export interface FinalizedStorageNativeEvent{readonly finalized_block_hash:BlockHash;readonly event_index:number;readonly event:StorageNativeEvent}
export interface StorageNativeEventSubscription{readonly finality:"finalized";readonly from_finalized_block:BlockHash;readonly kinds:readonly StorageNativeEventKind[]}
export function storageNativeEventSubscription(from_finalized_block:BlockHash,kinds:readonly StorageNativeEventKind[]):StorageNativeEventSubscription{if(kinds.length<1||kinds.length>32||new Set(kinds).size!==kinds.length)invalidDomainInput("storage","event_subscription","subscription requires 1-32 unique event kinds");return{finality:"finalized",from_finalized_block,kinds:[...kinds]}}

export function storageNativeEventOutcome(e:StorageNativeEvent):StorageNativeOutcome{const d=e.data as any;const action=({provider_registered:"registered",provider_updated:"updated",provider_status_changed:"status_changed",provider_removed:"removed",heartbeat:"heartbeat",agreement_proposed:"proposed",agreement_accepted:"accepted",agreement_cancelled:"cancelled",agreement_renewal_requested:"renewal_requested",agreement_renewed:"renewed",agreement_expired:"expired",agreement_pruned:"pruned",challenge_issued:"issued",checkpoint_submitted:"checkpoint_submitted",challenge_timed_out:"timed_out",provider_root_committed:"root_committed",deletion_acknowledged:"deletion_acknowledged",drive_created:"created",drive_root_updated:"root_updated",drive_controller_changed:"controller_changed",drive_transferred:"transferred",drive_archived:"archived",bucket_created:"created",bucket_controller_changed:"controller_changed",bucket_transferred:"transferred",bucket_archived:"archived",bucket_versioning_changed:"versioning_changed",object_put:"put",object_deleted:"deleted",bucket_deleted:"deleted"} as const)[e.event];
 if(e.event.startsWith("provider_")||e.event==="heartbeat")return{outcome:"provider",data:{provider:d.provider,action}};
 if(e.event.startsWith("agreement_")||e.event==="deletion_acknowledged")return{outcome:"agreement",data:{agreement:d.agreement,action}};
 if(e.event.startsWith("challenge_")||e.event==="checkpoint_submitted")return{outcome:"challenge",data:{challenge:d.challenge,action}};
 if(e.event.startsWith("drive_"))return{outcome:"drive",data:{drive:d.drive,action,version:d.version??null}};
 if(e.event.startsWith("object_"))return{outcome:"object",data:{bucket:d.bucket,object:d.object,action,version:d.version}};
 return{outcome:"bucket",data:{bucket:d.bucket,action,version:d.version??null}};
}
