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

// Generated from the frozen cord.origin.host/2 CDDL and operation registry. Do not edit.
export const HOST_V2_PROTOCOL = 'cord.origin.host/2' as const;
export const HOST_V2_MAJOR = 2 as const;
export const HOST_V2_MINOR = 1 as const;
export const HOST_V2_REGISTRY_SHA256 = 'e88cff81e8296a4451eb9eb94e1b8db064031311c85ef9ad08984b6bc057c64e' as const;
export const HOST_V2_JSON_PROJECTION_SHA256 = '906fc06646cd5b20be7c13baf27669985d226fafdec3c979f37403bce944da15' as const;
export const HOST_V2_SEMANTIC_SCHEMA_SHA256 = '9c3e085dea1d8ae2a06654315b201548e53449fce571aa28f6544df63e9e1d25' as const;
export type HostV2OperationName = "storage.bucket.create" | "storage.bucket.get" | "storage.bucket.grant" | "storage.bucket.revoke" | "storage.object.put" | "storage.object.get" | "storage.object.range" | "storage.object.delete" | "storage.object.status" | "storage.checkpoint.status" | "storage.checkpoint.subscribe" | "storage.replica.status" | "storage.replica.subscribe" | "storage.deletion.status" | "storage.deletion.subscribe" | "storage.drive.read" | "storage.drive.commit" | "storage.drive.share" | "storage.s3.put" | "storage.s3.get" | "storage.s3.list" | "storage.s3.delete" | "storage.publish" | "storage.resolve" | "storage.keys.export" | "storage.keys.import" | "identity.account" | "identity.profile.read" | "identity.profile.disclose" | "identity.humanity.status" | "identity.humanity.prove" | "identity.subject.derive" | "identity.entitlements.read" | "transaction.sign";
export type HostV2ErrorName = "WIRE_SCHEMA_INVALID" | "WIRE_NON_CANONICAL" | "WIRE_VERSION_MISMATCH" | "WIRE_GENESIS_MISMATCH" | "WIRE_DESCRIPTOR_MISMATCH" | "WIRE_SEQUENCE_INVALID" | "REQUEST_DEADLINE_EXPIRED" | "REQUEST_CANCELLED" | "REQUEST_NOT_FOUND" | "GRANT_REQUIRED" | "GRANT_SCOPE_DENIED" | "GRANT_EXPIRED" | "GRANT_REVOKED" | "HOST_OUTBOX_UNAVAILABLE" | "HOST_OUTBOX_FULL" | "HOST_OUTBOX_CORRUPT" | "HOST_OUTBOX_EXPIRED" | "STORAGE_CHUNK_OUT_OF_ORDER" | "STORAGE_CHUNK_TOO_LARGE" | "STORAGE_CHUNK_MISSING" | "STORAGE_LENGTH_MISMATCH" | "STORAGE_CID_MISMATCH" | "STORAGE_OBJECT_TOO_LARGE" | "STORAGE_IDEMPOTENCY_CONFLICT" | "STORAGE_RANGE_INVALID" | "STORAGE_INTEGRITY_FAILED" | "STORAGE_NOT_PUBLISHABLE" | "STORAGE_NOT_FOUND" | "ENCRYPTION_NONCE_REUSE" | "STORAGE_CHECKPOINT_WRONG_DOMAIN" | "STORAGE_CHECKPOINT_WRONG_VERSION" | "STORAGE_CHECKPOINT_WRONG_BUCKET" | "STORAGE_CHECKPOINT_WRONG_KEY" | "STORAGE_CHECKPOINT_STALE_NONCE" | "STORAGE_CHECKPOINT_WRONG_WINDOW" | "CAPABILITY_SIGNATURE_INVALID" | "CAPABILITY_AUDIENCE_INVALID" | "CAPABILITY_CONTENT_INVALID" | "CAPABILITY_NONCE_REPLAY" | "CAPABILITY_EXPIRED" | "CAPABILITY_ISSUER_REVOKED" | "RESUME_SIGNATURE_INVALID" | "RESUME_AUDIENCE_INVALID" | "RESUME_REPLAY" | "RESUME_EXPIRED" | "RESUME_REVOKED" | "RESUME_CURSOR_INVALID" | "PROVIDER_RECOVERY_TABLE_FULL" | "STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM" | "STORAGE_CHECKPOINT_SEQUENCE_INVALID" | "STORAGE_CHECKPOINT_EQUIVOCATION" | "BUCKET_NOT_FOUND" | "BUCKET_VERSION_CONFLICT" | "BUCKET_MEMBER_LIMIT" | "AGREEMENT_INVALID_STATE" | "AGREEMENT_CAPACITY_EXCEEDED" | "PROVIDER_INELIGIBLE" | "PROVIDER_ORG_UNKNOWN" | "PROVIDER_ATTESTATION_INVALID" | "PROVIDER_ATTESTATION_EXPIRED" | "PROVIDER_SLA_INVALID" | "PROVIDER_SERVICE_KEY_INVALID" | "STORAGE_CURSOR_STALE" | "DRIVE_NAME_INVALID" | "DRIVE_PATH_TOO_LONG" | "DRIVE_DEPTH_EXCEEDED" | "DRIVE_CHILD_LIMIT" | "DRIVE_METADATA_LIMIT" | "DRIVE_ORDER_INVALID" | "DRIVE_VERSION_CONFLICT" | "DRIVE_REFERENCE_UNPUBLISHABLE" | "S3_BUCKET_NAME_INVALID" | "S3_KEY_INVALID" | "S3_METADATA_LIMIT" | "S3_PRECONDITION_FAILED" | "S3_NOT_FOUND" | "S3_HISTORY_LIMIT" | "IDENTITY_AUDIENCE_INVALID" | "IDENTITY_CHALLENGE_REPLAY" | "IDENTITY_PROOF_EXPIRED" | "IDENTITY_EPOCH_INVALID" | "IDENTITY_DISCLOSURE_DENIED" | "IDENTITY_HUMANITY_UNAVAILABLE" | "IDENTITY_ENTITLEMENT_UNAVAILABLE" | "SIGNING_CONSENT_REQUIRED" | "IDENTITY_RECOVERY_ENTROPY_FAILED" | "IDENTITY_RECOVERY_INSTALL_FAILED" | "IDENTITY_OLD_INCARNATION" | "IDENTITY_RETIRED_SET_FULL" | "IDENTITY_AUTHORITY_UNAVAILABLE" | "IDENTITY_EFFECT_CONFLICT";
export type HostV2FeatureId = "identity.account" | "identity.entitlements" | "identity.humanity" | "identity.profile" | "identity.subject" | "storage.content" | "storage.control" | "storage.deletion" | "storage.drive" | "storage.encryption" | "storage.proof" | "storage.publish" | "storage.replica" | "storage.s3" | "transaction.sign";
export const HOST_V2_FEATURE_IDS: readonly HostV2FeatureId[] = ["identity.account","identity.entitlements","identity.humanity","identity.profile","identity.subject","storage.content","storage.control","storage.deletion","storage.drive","storage.encryption","storage.proof","storage.publish","storage.replica","storage.s3","transaction.sign"] as const;
export type HostV2TypeName = "Hash32" | "AccountId32" | "ProviderId" | "BucketId" | "AgreementId" | "GrantId" | "Subject" | "KeyId" | "RequestId" | "OperationId" | "Nonce" | "Cid" | "ProductId" | "NfcText128" | "NfcText256" | "Bytes4MiB" | "U16" | "U32" | "U64" | "EventV2" | "ErrorDetails" | "RequestV2" | "StorageBucketCreateFrame" | "StorageBucketGetFrame" | "StorageBucketGrantFrame" | "StorageBucketRevokeFrame" | "StorageObjectPutFrame" | "StorageObjectGetFrame" | "StorageObjectRangeFrame" | "StorageObjectDeleteFrame" | "StorageObjectStatusFrame" | "StorageCheckpointStatusFrame" | "StorageCheckpointSubscribeFrame" | "StorageReplicaStatusFrame" | "StorageReplicaSubscribeFrame" | "StorageDeletionStatusFrame" | "StorageDeletionSubscribeFrame" | "StorageDriveReadFrame" | "StorageDriveCommitFrame" | "StorageDriveShareFrame" | "StorageS3PutFrame" | "StorageS3GetFrame" | "StorageS3ListFrame" | "StorageS3DeleteFrame" | "StoragePublishFrame" | "StorageResolveFrame" | "StorageKeysExportFrame" | "StorageKeysImportFrame" | "IdentityAccountFrame" | "IdentityProfileReadFrame" | "IdentityProfileDiscloseFrame" | "IdentityHumanityStatusFrame" | "IdentityHumanityProveFrame" | "IdentitySubjectDeriveFrame" | "IdentityEntitlementsReadFrame" | "TransactionSignFrame" | "ErrorV2" | "Error100" | "Error101" | "Error102" | "Error103" | "Error104" | "Error105" | "Error106" | "Error107" | "Error108" | "Error109" | "Error110" | "Error111" | "Error112" | "Error113" | "Error114" | "Error115" | "Error116" | "Error200" | "Error201" | "Error202" | "Error203" | "Error204" | "Error205" | "Error206" | "Error207" | "Error208" | "Error209" | "Error210" | "Error211" | "Error220" | "Error221" | "Error222" | "Error223" | "Error224" | "Error225" | "Error226" | "Error227" | "Error228" | "Error229" | "Error230" | "Error231" | "Error232" | "Error233" | "Error234" | "Error235" | "Error236" | "Error237" | "Error238" | "Error239" | "Error240" | "Error241" | "Error250" | "Error251" | "Error252" | "Error253" | "Error254" | "Error255" | "Error256" | "Error257" | "Error258" | "Error259" | "Error260" | "Error261" | "Error300" | "Error301" | "Error302" | "Error303" | "Error304" | "Error305" | "Error306" | "Error307" | "Error320" | "Error321" | "Error322" | "Error323" | "Error324" | "Error325" | "Error400" | "Error401" | "Error402" | "Error403" | "Error404" | "Error405" | "Error406" | "Error407" | "Error408" | "Error409" | "Error410" | "Error411" | "Error412" | "Error413" | "Empty" | "Fin" | "AcceptedState" | "ProgressState" | "ResultState" | "BucketV1" | "ProviderReceiptV1" | "CheckpointV2" | "SubscriptionAckV1" | "IdentityReceiptV2" | "StorageBucketCreateRequest" | "StorageBucketCreateAccepted" | "StorageBucketCreateProgress" | "StorageBucketCreateResult" | "StorageBucketCreateError" | "StorageBucketGetRequest" | "StorageBucketGetAccepted" | "StorageBucketGetProgress" | "StorageBucketGetResult" | "StorageBucketGetError" | "StorageBucketGrantRequest" | "StorageBucketGrantAccepted" | "StorageBucketGrantProgress" | "StorageBucketGrantResult" | "StorageBucketGrantError" | "StorageBucketRevokeRequest" | "StorageBucketRevokeAccepted" | "StorageBucketRevokeProgress" | "StorageBucketRevokeResult" | "StorageBucketRevokeError" | "StorageObjectPutRequest" | "StorageObjectPutAccepted" | "StorageObjectPutProgress" | "StorageObjectPutResult" | "StorageObjectPutError" | "StorageObjectGetRequest" | "StorageObjectGetAccepted" | "StorageObjectGetProgress" | "StorageObjectGetResult" | "StorageObjectGetError" | "StorageObjectRangeRequest" | "StorageObjectRangeAccepted" | "StorageObjectRangeProgress" | "StorageObjectRangeResult" | "StorageObjectRangeError" | "StorageObjectDeleteRequest" | "StorageObjectDeleteAccepted" | "StorageObjectDeleteProgress" | "StorageObjectDeleteResult" | "StorageObjectDeleteError" | "StorageObjectStatusRequest" | "StorageObjectStatusAccepted" | "StorageObjectStatusProgress" | "StorageObjectStatusResult" | "StorageObjectStatusError" | "StorageCheckpointStatusRequest" | "StorageCheckpointStatusAccepted" | "StorageCheckpointStatusProgress" | "StorageCheckpointStatusResult" | "StorageCheckpointStatusError" | "StorageCheckpointSubscribeRequest" | "StorageCheckpointSubscribeAccepted" | "StorageCheckpointSubscribeProgress" | "StorageCheckpointSubscribeResult" | "StorageCheckpointSubscribeError" | "StorageReplicaStatusRequest" | "StorageReplicaStatusAccepted" | "StorageReplicaStatusProgress" | "StorageReplicaStatusResult" | "StorageReplicaStatusError" | "StorageReplicaSubscribeRequest" | "StorageReplicaSubscribeAccepted" | "StorageReplicaSubscribeProgress" | "StorageReplicaSubscribeResult" | "StorageReplicaSubscribeError" | "StorageDeletionStatusRequest" | "StorageDeletionStatusAccepted" | "StorageDeletionStatusProgress" | "StorageDeletionStatusResult" | "StorageDeletionStatusError" | "StorageDeletionSubscribeRequest" | "StorageDeletionSubscribeAccepted" | "StorageDeletionSubscribeProgress" | "StorageDeletionSubscribeResult" | "StorageDeletionSubscribeError" | "StorageDriveReadRequest" | "StorageDriveReadAccepted" | "StorageDriveReadProgress" | "StorageDriveReadResult" | "StorageDriveReadError" | "StorageDriveCommitRequest" | "StorageDriveCommitAccepted" | "StorageDriveCommitProgress" | "StorageDriveCommitResult" | "StorageDriveCommitError" | "StorageDriveShareRequest" | "StorageDriveShareAccepted" | "StorageDriveShareProgress" | "StorageDriveShareResult" | "StorageDriveShareError" | "StorageS3PutRequest" | "StorageS3PutAccepted" | "StorageS3PutProgress" | "StorageS3PutResult" | "StorageS3PutError" | "StorageS3GetRequest" | "StorageS3GetAccepted" | "StorageS3GetProgress" | "StorageS3GetResult" | "StorageS3GetError" | "StorageS3ListRequest" | "StorageS3ListAccepted" | "StorageS3ListProgress" | "StorageS3ListResult" | "StorageS3ListError" | "StorageS3DeleteRequest" | "StorageS3DeleteAccepted" | "StorageS3DeleteProgress" | "StorageS3DeleteResult" | "StorageS3DeleteError" | "StoragePublishRequest" | "StoragePublishAccepted" | "StoragePublishProgress" | "StoragePublishResult" | "StoragePublishError" | "StorageResolveRequest" | "StorageResolveAccepted" | "StorageResolveProgress" | "StorageResolveResult" | "StorageResolveError" | "StorageKeysExportRequest" | "StorageKeysExportAccepted" | "StorageKeysExportProgress" | "StorageKeysExportResult" | "StorageKeysExportError" | "StorageKeysImportRequest" | "StorageKeysImportAccepted" | "StorageKeysImportProgress" | "StorageKeysImportResult" | "StorageKeysImportError" | "IdentityAccountRequest" | "IdentityAccountAccepted" | "IdentityAccountProgress" | "IdentityAccountResult" | "IdentityAccountError" | "IdentityProfileReadRequest" | "IdentityProfileReadAccepted" | "IdentityProfileReadProgress" | "IdentityProfileReadResult" | "IdentityProfileReadError" | "IdentityProfileDiscloseRequest" | "IdentityProfileDiscloseAccepted" | "IdentityProfileDiscloseProgress" | "IdentityProfileDiscloseResult" | "IdentityProfileDiscloseError" | "IdentityHumanityStatusRequest" | "IdentityHumanityStatusAccepted" | "IdentityHumanityStatusProgress" | "IdentityHumanityStatusResult" | "IdentityHumanityStatusError" | "IdentityHumanityProveRequest" | "IdentityHumanityProveAccepted" | "IdentityHumanityProveProgress" | "IdentityHumanityProveResult" | "IdentityHumanityProveError" | "IdentitySubjectDeriveRequest" | "IdentitySubjectDeriveAccepted" | "IdentitySubjectDeriveProgress" | "IdentitySubjectDeriveResult" | "IdentitySubjectDeriveError" | "IdentityEntitlementsReadRequest" | "IdentityEntitlementsReadAccepted" | "IdentityEntitlementsReadProgress" | "IdentityEntitlementsReadResult" | "IdentityEntitlementsReadError" | "TransactionSignRequest" | "TransactionSignAccepted" | "TransactionSignProgress" | "TransactionSignResult" | "TransactionSignError" | "OperationRequest" | "AcceptedEventV2" | "ProgressEventV2" | "ResultEventV2" | "ErrorEventV2" | "CancelledEventV2" | "StorageCheckpointSubscribeSubscriptionEvent" | "StorageCheckpointSubscribeUnsubscribeAck" | "StorageReplicaSubscribeSubscriptionEvent" | "StorageReplicaSubscribeUnsubscribeAck" | "StorageDeletionSubscribeSubscriptionEvent" | "StorageDeletionSubscribeUnsubscribeAck" | "AllAccepted" | "AllProgress" | "AllResult" | "AllError" | "SubjectContextV2" | "SubjectProofV2" | "ProviderCapabilityV1" | "ResumeTokenV1" | "ResponseAckV1" | "HostOutboxEntryV1" | "RecoveryEntryV1" | "CheckpointSubmissionV2" | "CheckpointResultV2" | "ProviderTransferRequestV1" | "ProviderTransferChunkV1" | "ProviderTransferReceiptV1" | "SubjectProofEnvelopeV2" | "RecoveryInstallV2" | "RecoveryReceiptV2" | "DriveFileManifestV1" | "DriveManifestV1" | "DriveChangedEventV1" | "S3ObjectVersionV1" | "S3ChangedEventV1" | "DurableStateV1";
export type HostV2SchemaNode = { readonly kind:'ref'; readonly name:HostV2TypeName } | { readonly kind:'union'; readonly variants:readonly HostV2SchemaNode[] } | { readonly kind:'map'; readonly fields:readonly {readonly key:number;readonly required:boolean;readonly schema:HostV2SchemaNode}[] } | {readonly kind:'array';readonly min:number;readonly max:number;readonly items:HostV2SchemaNode} | {readonly kind:'uint';readonly min:string;readonly max:string} | {readonly kind:'bytes'|'text';readonly min:number;readonly max:number;readonly nfc?:boolean} | {readonly kind:'bool'} | {readonly kind:'const';readonly value:number|string|boolean};
export type Hash32 = Uint8Array;
export type AccountId32 = Uint8Array;
export type ProviderId = Uint8Array;
export type BucketId = Uint8Array;
export type AgreementId = Uint8Array;
export type GrantId = Uint8Array;
export type Subject = Uint8Array;
export type KeyId = Uint8Array;
export type RequestId = Uint8Array;
export type OperationId = Uint8Array;
export type Nonce = Uint8Array;
export type Cid = string;
export type ProductId = string;
export type NfcText128 = string;
export type NfcText256 = string;
export type Bytes4MiB = Uint8Array;
export type U16 = number | bigint;
export type U32 = number | bigint;
export type U64 = number | bigint;
export type EventV2 = AcceptedEventV2 | ProgressEventV2 | ResultEventV2 | ErrorEventV2 | CancelledEventV2;
export type ErrorDetails = { readonly 0?: NfcText256; readonly 1?: U64; readonly 2?: U64; readonly 3?: Hash32 };
export type RequestV2 = StorageBucketCreateFrame | StorageBucketGetFrame | StorageBucketGrantFrame | StorageBucketRevokeFrame | StorageObjectPutFrame | StorageObjectGetFrame | StorageObjectRangeFrame | StorageObjectDeleteFrame | StorageObjectStatusFrame | StorageCheckpointStatusFrame | StorageCheckpointSubscribeFrame | StorageReplicaStatusFrame | StorageReplicaSubscribeFrame | StorageDeletionStatusFrame | StorageDeletionSubscribeFrame | StorageDriveReadFrame | StorageDriveCommitFrame | StorageDriveShareFrame | StorageS3PutFrame | StorageS3GetFrame | StorageS3ListFrame | StorageS3DeleteFrame | StoragePublishFrame | StorageResolveFrame | StorageKeysExportFrame | StorageKeysImportFrame | IdentityAccountFrame | IdentityProfileReadFrame | IdentityProfileDiscloseFrame | IdentityHumanityStatusFrame | IdentityHumanityProveFrame | IdentitySubjectDeriveFrame | IdentityEntitlementsReadFrame | TransactionSignFrame;
export type StorageBucketCreateFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1000; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageBucketCreateRequest };
export type StorageBucketGetFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1001; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageBucketGetRequest };
export type StorageBucketGrantFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1002; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageBucketGrantRequest };
export type StorageBucketRevokeFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1003; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageBucketRevokeRequest };
export type StorageObjectPutFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1010; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageObjectPutRequest };
export type StorageObjectGetFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1011; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageObjectGetRequest };
export type StorageObjectRangeFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1012; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageObjectRangeRequest };
export type StorageObjectDeleteFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1013; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageObjectDeleteRequest };
export type StorageObjectStatusFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1014; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageObjectStatusRequest };
export type StorageCheckpointStatusFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1020; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageCheckpointStatusRequest };
export type StorageCheckpointSubscribeFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1021; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageCheckpointSubscribeRequest };
export type StorageReplicaStatusFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1022; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageReplicaStatusRequest };
export type StorageReplicaSubscribeFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1023; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageReplicaSubscribeRequest };
export type StorageDeletionStatusFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1024; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageDeletionStatusRequest };
export type StorageDeletionSubscribeFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1025; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageDeletionSubscribeRequest };
export type StorageDriveReadFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1030; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageDriveReadRequest };
export type StorageDriveCommitFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1031; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageDriveCommitRequest };
export type StorageDriveShareFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1032; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageDriveShareRequest };
export type StorageS3PutFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1040; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageS3PutRequest };
export type StorageS3GetFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1041; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageS3GetRequest };
export type StorageS3ListFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1042; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageS3ListRequest };
export type StorageS3DeleteFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1043; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageS3DeleteRequest };
export type StoragePublishFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1050; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StoragePublishRequest };
export type StorageResolveFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1051; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageResolveRequest };
export type StorageKeysExportFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1060; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageKeysExportRequest };
export type StorageKeysImportFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1061; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: StorageKeysImportRequest };
export type IdentityAccountFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1100; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityAccountRequest };
export type IdentityProfileReadFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1101; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityProfileReadRequest };
export type IdentityProfileDiscloseFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1102; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityProfileDiscloseRequest };
export type IdentityHumanityStatusFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1103; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityHumanityStatusRequest };
export type IdentityHumanityProveFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1104; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityHumanityProveRequest };
export type IdentitySubjectDeriveFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1105; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentitySubjectDeriveRequest };
export type IdentityEntitlementsReadFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1106; readonly 4: GrantId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: IdentityEntitlementsReadRequest };
export type TransactionSignFrame = { readonly 0: 2; readonly 1: RequestId; readonly 2: ProductId; readonly 3: 1200; readonly 4: GrantId; readonly 5: OperationId; readonly 6?: Uint8Array; readonly 7: U64; readonly 8: TransactionSignRequest };
export type ErrorV2 = Error100 | Error101 | Error102 | Error103 | Error104 | Error105 | Error106 | Error107 | Error108 | Error109 | Error110 | Error111 | Error112 | Error113 | Error114 | Error115 | Error116 | Error200 | Error201 | Error202 | Error203 | Error204 | Error205 | Error206 | Error207 | Error208 | Error209 | Error210 | Error211 | Error220 | Error221 | Error222 | Error223 | Error224 | Error225 | Error226 | Error227 | Error228 | Error229 | Error230 | Error231 | Error232 | Error233 | Error234 | Error235 | Error236 | Error237 | Error238 | Error239 | Error240 | Error241 | Error250 | Error251 | Error252 | Error253 | Error254 | Error255 | Error256 | Error257 | Error258 | Error259 | Error260 | Error261 | Error300 | Error301 | Error302 | Error303 | Error304 | Error305 | Error306 | Error307 | Error320 | Error321 | Error322 | Error323 | Error324 | Error325 | Error400 | Error401 | Error402 | Error403 | Error404 | Error405 | Error406 | Error407 | Error408 | Error409 | Error410 | Error411 | Error412 | Error413;
export type Error100 = { readonly 0: 100; readonly 1: "WIRE_SCHEMA_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error101 = { readonly 0: 101; readonly 1: "WIRE_NON_CANONICAL"; readonly 2: false; readonly 3: ErrorDetails };
export type Error102 = { readonly 0: 102; readonly 1: "WIRE_VERSION_MISMATCH"; readonly 2: false; readonly 3: ErrorDetails };
export type Error103 = { readonly 0: 103; readonly 1: "WIRE_GENESIS_MISMATCH"; readonly 2: false; readonly 3: ErrorDetails };
export type Error104 = { readonly 0: 104; readonly 1: "WIRE_DESCRIPTOR_MISMATCH"; readonly 2: false; readonly 3: ErrorDetails };
export type Error105 = { readonly 0: 105; readonly 1: "WIRE_SEQUENCE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error106 = { readonly 0: 106; readonly 1: "REQUEST_DEADLINE_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error107 = { readonly 0: 107; readonly 1: "REQUEST_CANCELLED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error108 = { readonly 0: 108; readonly 1: "REQUEST_NOT_FOUND"; readonly 2: false; readonly 3: ErrorDetails };
export type Error109 = { readonly 0: 109; readonly 1: "GRANT_REQUIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error110 = { readonly 0: 110; readonly 1: "GRANT_SCOPE_DENIED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error111 = { readonly 0: 111; readonly 1: "GRANT_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error112 = { readonly 0: 112; readonly 1: "GRANT_REVOKED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error113 = { readonly 0: 113; readonly 1: "HOST_OUTBOX_UNAVAILABLE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error114 = { readonly 0: 114; readonly 1: "HOST_OUTBOX_FULL"; readonly 2: true; readonly 3: ErrorDetails };
export type Error115 = { readonly 0: 115; readonly 1: "HOST_OUTBOX_CORRUPT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error116 = { readonly 0: 116; readonly 1: "HOST_OUTBOX_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error200 = { readonly 0: 200; readonly 1: "STORAGE_CHUNK_OUT_OF_ORDER"; readonly 2: false; readonly 3: ErrorDetails };
export type Error201 = { readonly 0: 201; readonly 1: "STORAGE_CHUNK_TOO_LARGE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error202 = { readonly 0: 202; readonly 1: "STORAGE_CHUNK_MISSING"; readonly 2: false; readonly 3: ErrorDetails };
export type Error203 = { readonly 0: 203; readonly 1: "STORAGE_LENGTH_MISMATCH"; readonly 2: false; readonly 3: ErrorDetails };
export type Error204 = { readonly 0: 204; readonly 1: "STORAGE_CID_MISMATCH"; readonly 2: false; readonly 3: ErrorDetails };
export type Error205 = { readonly 0: 205; readonly 1: "STORAGE_OBJECT_TOO_LARGE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error206 = { readonly 0: 206; readonly 1: "STORAGE_IDEMPOTENCY_CONFLICT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error207 = { readonly 0: 207; readonly 1: "STORAGE_RANGE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error208 = { readonly 0: 208; readonly 1: "STORAGE_INTEGRITY_FAILED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error209 = { readonly 0: 209; readonly 1: "STORAGE_NOT_PUBLISHABLE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error210 = { readonly 0: 210; readonly 1: "STORAGE_NOT_FOUND"; readonly 2: false; readonly 3: ErrorDetails };
export type Error211 = { readonly 0: 211; readonly 1: "ENCRYPTION_NONCE_REUSE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error220 = { readonly 0: 220; readonly 1: "STORAGE_CHECKPOINT_WRONG_DOMAIN"; readonly 2: false; readonly 3: ErrorDetails };
export type Error221 = { readonly 0: 221; readonly 1: "STORAGE_CHECKPOINT_WRONG_VERSION"; readonly 2: false; readonly 3: ErrorDetails };
export type Error222 = { readonly 0: 222; readonly 1: "STORAGE_CHECKPOINT_WRONG_BUCKET"; readonly 2: false; readonly 3: ErrorDetails };
export type Error223 = { readonly 0: 223; readonly 1: "STORAGE_CHECKPOINT_WRONG_KEY"; readonly 2: false; readonly 3: ErrorDetails };
export type Error224 = { readonly 0: 224; readonly 1: "STORAGE_CHECKPOINT_STALE_NONCE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error225 = { readonly 0: 225; readonly 1: "STORAGE_CHECKPOINT_WRONG_WINDOW"; readonly 2: false; readonly 3: ErrorDetails };
export type Error226 = { readonly 0: 226; readonly 1: "CAPABILITY_SIGNATURE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error227 = { readonly 0: 227; readonly 1: "CAPABILITY_AUDIENCE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error228 = { readonly 0: 228; readonly 1: "CAPABILITY_CONTENT_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error229 = { readonly 0: 229; readonly 1: "CAPABILITY_NONCE_REPLAY"; readonly 2: false; readonly 3: ErrorDetails };
export type Error230 = { readonly 0: 230; readonly 1: "CAPABILITY_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error231 = { readonly 0: 231; readonly 1: "CAPABILITY_ISSUER_REVOKED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error232 = { readonly 0: 232; readonly 1: "RESUME_SIGNATURE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error233 = { readonly 0: 233; readonly 1: "RESUME_AUDIENCE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error234 = { readonly 0: 234; readonly 1: "RESUME_REPLAY"; readonly 2: false; readonly 3: ErrorDetails };
export type Error235 = { readonly 0: 235; readonly 1: "RESUME_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error236 = { readonly 0: 236; readonly 1: "RESUME_REVOKED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error237 = { readonly 0: 237; readonly 1: "RESUME_CURSOR_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error238 = { readonly 0: 238; readonly 1: "PROVIDER_RECOVERY_TABLE_FULL"; readonly 2: false; readonly 3: ErrorDetails };
export type Error239 = { readonly 0: 239; readonly 1: "STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM"; readonly 2: false; readonly 3: ErrorDetails };
export type Error240 = { readonly 0: 240; readonly 1: "STORAGE_CHECKPOINT_SEQUENCE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error241 = { readonly 0: 241; readonly 1: "STORAGE_CHECKPOINT_EQUIVOCATION"; readonly 2: false; readonly 3: ErrorDetails };
export type Error250 = { readonly 0: 250; readonly 1: "BUCKET_NOT_FOUND"; readonly 2: false; readonly 3: ErrorDetails };
export type Error251 = { readonly 0: 251; readonly 1: "BUCKET_VERSION_CONFLICT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error252 = { readonly 0: 252; readonly 1: "BUCKET_MEMBER_LIMIT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error253 = { readonly 0: 253; readonly 1: "AGREEMENT_INVALID_STATE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error254 = { readonly 0: 254; readonly 1: "AGREEMENT_CAPACITY_EXCEEDED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error255 = { readonly 0: 255; readonly 1: "PROVIDER_INELIGIBLE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error256 = { readonly 0: 256; readonly 1: "PROVIDER_ORG_UNKNOWN"; readonly 2: false; readonly 3: ErrorDetails };
export type Error257 = { readonly 0: 257; readonly 1: "PROVIDER_ATTESTATION_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error258 = { readonly 0: 258; readonly 1: "PROVIDER_ATTESTATION_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error259 = { readonly 0: 259; readonly 1: "PROVIDER_SLA_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error260 = { readonly 0: 260; readonly 1: "PROVIDER_SERVICE_KEY_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error261 = { readonly 0: 261; readonly 1: "STORAGE_CURSOR_STALE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error300 = { readonly 0: 300; readonly 1: "DRIVE_NAME_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error301 = { readonly 0: 301; readonly 1: "DRIVE_PATH_TOO_LONG"; readonly 2: false; readonly 3: ErrorDetails };
export type Error302 = { readonly 0: 302; readonly 1: "DRIVE_DEPTH_EXCEEDED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error303 = { readonly 0: 303; readonly 1: "DRIVE_CHILD_LIMIT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error304 = { readonly 0: 304; readonly 1: "DRIVE_METADATA_LIMIT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error305 = { readonly 0: 305; readonly 1: "DRIVE_ORDER_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error306 = { readonly 0: 306; readonly 1: "DRIVE_VERSION_CONFLICT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error307 = { readonly 0: 307; readonly 1: "DRIVE_REFERENCE_UNPUBLISHABLE"; readonly 2: false; readonly 3: ErrorDetails };
export type Error320 = { readonly 0: 320; readonly 1: "S3_BUCKET_NAME_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error321 = { readonly 0: 321; readonly 1: "S3_KEY_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error322 = { readonly 0: 322; readonly 1: "S3_METADATA_LIMIT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error323 = { readonly 0: 323; readonly 1: "S3_PRECONDITION_FAILED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error324 = { readonly 0: 324; readonly 1: "S3_NOT_FOUND"; readonly 2: false; readonly 3: ErrorDetails };
export type Error325 = { readonly 0: 325; readonly 1: "S3_HISTORY_LIMIT"; readonly 2: false; readonly 3: ErrorDetails };
export type Error400 = { readonly 0: 400; readonly 1: "IDENTITY_AUDIENCE_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error401 = { readonly 0: 401; readonly 1: "IDENTITY_CHALLENGE_REPLAY"; readonly 2: false; readonly 3: ErrorDetails };
export type Error402 = { readonly 0: 402; readonly 1: "IDENTITY_PROOF_EXPIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error403 = { readonly 0: 403; readonly 1: "IDENTITY_EPOCH_INVALID"; readonly 2: false; readonly 3: ErrorDetails };
export type Error404 = { readonly 0: 404; readonly 1: "IDENTITY_DISCLOSURE_DENIED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error405 = { readonly 0: 405; readonly 1: "IDENTITY_HUMANITY_UNAVAILABLE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error406 = { readonly 0: 406; readonly 1: "IDENTITY_ENTITLEMENT_UNAVAILABLE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error407 = { readonly 0: 407; readonly 1: "SIGNING_CONSENT_REQUIRED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error408 = { readonly 0: 408; readonly 1: "IDENTITY_RECOVERY_ENTROPY_FAILED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error409 = { readonly 0: 409; readonly 1: "IDENTITY_RECOVERY_INSTALL_FAILED"; readonly 2: false; readonly 3: ErrorDetails };
export type Error410 = { readonly 0: 410; readonly 1: "IDENTITY_OLD_INCARNATION"; readonly 2: false; readonly 3: ErrorDetails };
export type Error411 = { readonly 0: 411; readonly 1: "IDENTITY_RETIRED_SET_FULL"; readonly 2: false; readonly 3: ErrorDetails };
export type Error412 = { readonly 0: 412; readonly 1: "IDENTITY_AUTHORITY_UNAVAILABLE"; readonly 2: true; readonly 3: ErrorDetails };
export type Error413 = { readonly 0: 413; readonly 1: "IDENTITY_EFFECT_CONFLICT"; readonly 2: false; readonly 3: ErrorDetails };
export type Empty = {  };
export type Fin = { readonly 0: U64; readonly 1: Hash32 };
export type AcceptedState = { readonly 0: number | bigint };
export type ProgressState = { readonly 0: U64; readonly 1?: U64; readonly 2?: U64; readonly 3?: U64 };
export type ResultState = { readonly 0: Hash32 };
export type BucketV1 = { readonly 0: AccountId32; readonly 1: U64; readonly 2: U32; readonly 3: ProviderId; readonly 4: readonly (ProviderId)[] };
export type ProviderReceiptV1 = { readonly 0: ProviderId; readonly 1: Cid; readonly 2: U64; readonly 3: Uint8Array };
export type CheckpointV2 = { readonly 0: Hash32; readonly 1: U64; readonly 2: U64; readonly 3: U32 };
export type SubscriptionAckV1 = { readonly 0: OperationId; readonly 1: U64 };
export type IdentityReceiptV2 = { readonly 0: Hash32; readonly 1: U64; readonly 2?: Fin };
export type StorageBucketCreateRequest = { readonly 0: U32; readonly 1: readonly (ProviderId)[]; readonly 2: number | bigint };
export type StorageBucketCreateAccepted = AcceptedState;
export type StorageBucketCreateProgress = ProgressState;
export type StorageBucketCreateResult = { readonly 0: BucketId; readonly 1: U64; readonly 2: Fin };
export type StorageBucketCreateError = ErrorV2;
export type StorageBucketGetRequest = { readonly 0: BucketId; readonly 1?: Hash32 };
export type StorageBucketGetAccepted = AcceptedState;
export type StorageBucketGetProgress = ProgressState;
export type StorageBucketGetResult = { readonly 0: BucketV1; readonly 1: Fin };
export type StorageBucketGetError = ErrorV2;
export type StorageBucketGrantRequest = { readonly 0: BucketId; readonly 1: Subject; readonly 2: number | bigint; readonly 3: U64; readonly 4: U64 };
export type StorageBucketGrantAccepted = AcceptedState;
export type StorageBucketGrantProgress = ProgressState;
export type StorageBucketGrantResult = { readonly 0: GrantId; readonly 1: U64; readonly 2: Fin };
export type StorageBucketGrantError = ErrorV2;
export type StorageBucketRevokeRequest = { readonly 0: BucketId; readonly 1: GrantId; readonly 2: U64 };
export type StorageBucketRevokeAccepted = AcceptedState;
export type StorageBucketRevokeProgress = ProgressState;
export type StorageBucketRevokeResult = { readonly 0: GrantId; readonly 1: U64; readonly 2: Fin };
export type StorageBucketRevokeError = ErrorV2;
export type StorageObjectPutRequest = { readonly 0: BucketId; readonly 1: Cid; readonly 2: U64; readonly 3: number | bigint; readonly 4: OperationId };
export type StorageObjectPutAccepted = AcceptedState;
export type StorageObjectPutProgress = ProgressState;
export type StorageObjectPutResult = { readonly 0: ProviderReceiptV1; readonly 1: boolean; readonly 2: Fin };
export type StorageObjectPutError = ErrorV2;
export type StorageObjectGetRequest = { readonly 0: BucketId; readonly 1: Cid };
export type StorageObjectGetAccepted = AcceptedState;
export type StorageObjectGetProgress = { readonly 0: U64; readonly 1: Bytes4MiB };
export type StorageObjectGetResult = { readonly 0: Cid; readonly 1: U64; readonly 2: CheckpointV2 };
export type StorageObjectGetError = ErrorV2;
export type StorageObjectRangeRequest = { readonly 0: BucketId; readonly 1: Cid; readonly 2: U64; readonly 3: U64 };
export type StorageObjectRangeAccepted = AcceptedState;
export type StorageObjectRangeProgress = { readonly 0: U64; readonly 1: Bytes4MiB };
export type StorageObjectRangeResult = { readonly 0: Cid; readonly 1: U64; readonly 2: U64; readonly 3: U64; readonly 4: CheckpointV2 };
export type StorageObjectRangeError = ErrorV2;
export type StorageObjectDeleteRequest = { readonly 0: BucketId; readonly 1: Cid; readonly 2: U64 };
export type StorageObjectDeleteAccepted = AcceptedState;
export type StorageObjectDeleteProgress = ProgressState;
export type StorageObjectDeleteResult = { readonly 0: U64; readonly 1: U32; readonly 2: U32; readonly 3: Fin };
export type StorageObjectDeleteError = ErrorV2;
export type StorageObjectStatusRequest = { readonly 0: BucketId; readonly 1: Cid };
export type StorageObjectStatusAccepted = AcceptedState;
export type StorageObjectStatusProgress = ProgressState;
export type StorageObjectStatusResult = { readonly 0: number | bigint; readonly 1?: ProviderReceiptV1; readonly 2?: CheckpointV2; readonly 3: U32; readonly 4: boolean; readonly 5: Fin };
export type StorageObjectStatusError = ErrorV2;
export type StorageCheckpointStatusRequest = { readonly 0: BucketId; readonly 1?: Hash32 };
export type StorageCheckpointStatusAccepted = AcceptedState;
export type StorageCheckpointStatusProgress = ProgressState;
export type StorageCheckpointStatusResult = { readonly 0: CheckpointV2; readonly 1: U32; readonly 2: U64; readonly 3: U32; readonly 4: Fin };
export type StorageCheckpointStatusError = ErrorV2;
export type StorageCheckpointSubscribeRequest = { readonly 0: BucketId; readonly 1: U64 };
export type StorageCheckpointSubscribeAccepted = AcceptedState;
export type StorageCheckpointSubscribeProgress = ProgressState;
export type StorageCheckpointSubscribeResult = { readonly 0: SubscriptionAckV1 };
export type StorageCheckpointSubscribeError = ErrorV2;
export type StorageReplicaStatusRequest = { readonly 0: BucketId };
export type StorageReplicaStatusAccepted = AcceptedState;
export type StorageReplicaStatusProgress = ProgressState;
export type StorageReplicaStatusResult = { readonly 0: ProviderId; readonly 1: readonly (ProviderId)[]; readonly 2: U32; readonly 3: U64; readonly 4: U32; readonly 5: Fin };
export type StorageReplicaStatusError = ErrorV2;
export type StorageReplicaSubscribeRequest = { readonly 0: BucketId; readonly 1: U64 };
export type StorageReplicaSubscribeAccepted = AcceptedState;
export type StorageReplicaSubscribeProgress = ProgressState;
export type StorageReplicaSubscribeResult = { readonly 0: SubscriptionAckV1 };
export type StorageReplicaSubscribeError = ErrorV2;
export type StorageDeletionStatusRequest = { readonly 0: BucketId; readonly 1: Cid };
export type StorageDeletionStatusAccepted = AcceptedState;
export type StorageDeletionStatusProgress = ProgressState;
export type StorageDeletionStatusResult = { readonly 0: U64; readonly 1: U32; readonly 2: Hash32; readonly 3: Fin };
export type StorageDeletionStatusError = ErrorV2;
export type StorageDeletionSubscribeRequest = { readonly 0: BucketId; readonly 1: Cid; readonly 2: U64 };
export type StorageDeletionSubscribeAccepted = AcceptedState;
export type StorageDeletionSubscribeProgress = ProgressState;
export type StorageDeletionSubscribeResult = { readonly 0: SubscriptionAckV1 };
export type StorageDeletionSubscribeError = ErrorV2;
export type StorageDriveReadRequest = { readonly 0: BucketId; readonly 1: string; readonly 2?: Cid };
export type StorageDriveReadAccepted = AcceptedState;
export type StorageDriveReadProgress = ProgressState;
export type StorageDriveReadResult = { readonly 0: Cid; readonly 1: Cid; readonly 2: U64; readonly 3: Fin };
export type StorageDriveReadError = ErrorV2;
export type StorageDriveCommitRequest = { readonly 0: BucketId; readonly 1: Cid; readonly 2: Uint8Array; readonly 3: U64; readonly 4: number | bigint };
export type StorageDriveCommitAccepted = AcceptedState;
export type StorageDriveCommitProgress = ProgressState;
export type StorageDriveCommitResult = { readonly 0: Cid; readonly 1: U64; readonly 2: CheckpointV2; readonly 3: Fin };
export type StorageDriveCommitError = ErrorV2;
export type StorageDriveShareRequest = { readonly 0: BucketId; readonly 1: Subject; readonly 2: number | bigint; readonly 3: U64; readonly 4: U64 };
export type StorageDriveShareAccepted = AcceptedState;
export type StorageDriveShareProgress = ProgressState;
export type StorageDriveShareResult = { readonly 0: GrantId; readonly 1: U64; readonly 2: Fin };
export type StorageDriveShareError = ErrorV2;
export type StorageS3PutRequest = { readonly 0: NfcText128; readonly 1: Uint8Array; readonly 2: Cid; readonly 3: Uint8Array; readonly 4: NfcText256; readonly 5?: string; readonly 6: OperationId };
export type StorageS3PutAccepted = AcceptedState;
export type StorageS3PutProgress = ProgressState;
export type StorageS3PutResult = { readonly 0: string; readonly 1: U64; readonly 2: Fin };
export type StorageS3PutError = ErrorV2;
export type StorageS3GetRequest = { readonly 0: NfcText128; readonly 1: Uint8Array; readonly 2?: U64 };
export type StorageS3GetAccepted = AcceptedState;
export type StorageS3GetProgress = { readonly 0: U64; readonly 1: Bytes4MiB };
export type StorageS3GetResult = { readonly 0: Cid; readonly 1: string; readonly 2: U64; readonly 3: Fin };
export type StorageS3GetError = ErrorV2;
export type StorageS3ListRequest = { readonly 0: NfcText128; readonly 1?: Uint8Array; readonly 2?: Uint8Array; readonly 3: number | bigint };
export type StorageS3ListAccepted = AcceptedState;
export type StorageS3ListProgress = ProgressState;
export type StorageS3ListResult = { readonly 0: readonly (Cid)[]; readonly 1?: Uint8Array; readonly 2: U64; readonly 3: Fin };
export type StorageS3ListError = ErrorV2;
export type StorageS3DeleteRequest = { readonly 0: NfcText128; readonly 1: Uint8Array; readonly 2?: string; readonly 3: OperationId };
export type StorageS3DeleteAccepted = AcceptedState;
export type StorageS3DeleteProgress = ProgressState;
export type StorageS3DeleteResult = { readonly 0: U64; readonly 1: U32; readonly 2: Fin };
export type StorageS3DeleteError = ErrorV2;
export type StoragePublishRequest = { readonly 0: Hash32; readonly 1: Cid; readonly 2?: U64 };
export type StoragePublishAccepted = AcceptedState;
export type StoragePublishProgress = ProgressState;
export type StoragePublishResult = { readonly 0: Hash32; readonly 1: Cid; readonly 2: Fin };
export type StoragePublishError = ErrorV2;
export type StorageResolveRequest = { readonly 0: NfcText256; readonly 1?: U64; readonly 2?: Hash32 };
export type StorageResolveAccepted = AcceptedState;
export type StorageResolveProgress = ProgressState;
export type StorageResolveResult = { readonly 0: Cid; readonly 1: U64; readonly 2: CheckpointV2; readonly 3: Fin };
export type StorageResolveError = ErrorV2;
export type StorageKeysExportRequest = { readonly 0: BucketId; readonly 1: U32; readonly 2: Uint8Array };
export type StorageKeysExportAccepted = AcceptedState;
export type StorageKeysExportProgress = ProgressState;
export type StorageKeysExportResult = { readonly 0: Uint8Array; readonly 1: U16; readonly 2: U32 };
export type StorageKeysExportError = ErrorV2;
export type StorageKeysImportRequest = { readonly 0: BucketId; readonly 1: Uint8Array; readonly 2: number | bigint; readonly 3: U32 };
export type StorageKeysImportAccepted = AcceptedState;
export type StorageKeysImportProgress = ProgressState;
export type StorageKeysImportResult = { readonly 0: KeyId; readonly 1: U32 };
export type StorageKeysImportError = ErrorV2;
export type IdentityAccountRequest = { readonly 0: NfcText128 };
export type IdentityAccountAccepted = AcceptedState;
export type IdentityAccountProgress = ProgressState;
export type IdentityAccountResult = { readonly 0: AccountId32; readonly 1: U64; readonly 2: Fin };
export type IdentityAccountError = ErrorV2;
export type IdentityProfileReadRequest = { readonly 0: Subject; readonly 1: readonly (NfcText128)[]; readonly 2?: Hash32 };
export type IdentityProfileReadAccepted = AcceptedState;
export type IdentityProfileReadProgress = ProgressState;
export type IdentityProfileReadResult = { readonly 0: IdentityReceiptV2 };
export type IdentityProfileReadError = ErrorV2;
export type IdentityProfileDiscloseRequest = { readonly 0: NfcText256; readonly 1: readonly (NfcText128)[]; readonly 2: NfcText256; readonly 3: U64 };
export type IdentityProfileDiscloseAccepted = AcceptedState;
export type IdentityProfileDiscloseProgress = ProgressState;
export type IdentityProfileDiscloseResult = { readonly 0: IdentityReceiptV2 };
export type IdentityProfileDiscloseError = ErrorV2;
export type IdentityHumanityStatusRequest = { readonly 0: Subject; readonly 1?: Hash32 };
export type IdentityHumanityStatusAccepted = AcceptedState;
export type IdentityHumanityStatusProgress = ProgressState;
export type IdentityHumanityStatusResult = { readonly 0: U16; readonly 1: U64; readonly 2: Fin };
export type IdentityHumanityStatusError = ErrorV2;
export type IdentityHumanityProveRequest = { readonly 0: NfcText256; readonly 1: Uint8Array; readonly 2: U64; readonly 3: readonly (NfcText128)[] };
export type IdentityHumanityProveAccepted = AcceptedState;
export type IdentityHumanityProveProgress = ProgressState;
export type IdentityHumanityProveResult = { readonly 0: Uint8Array; readonly 1: Uint8Array; readonly 2: Hash32; readonly 3: boolean; readonly 4: U64 };
export type IdentityHumanityProveError = ErrorV2;
export type IdentitySubjectDeriveRequest = { readonly 0: ProductId; readonly 1: NfcText256; readonly 2: NfcText256; readonly 3?: U32 };
export type IdentitySubjectDeriveAccepted = AcceptedState;
export type IdentitySubjectDeriveProgress = ProgressState;
export type IdentitySubjectDeriveResult = { readonly 0: Subject; readonly 1: Uint8Array; readonly 2: U32; readonly 3: Hash32; readonly 4: boolean };
export type IdentitySubjectDeriveError = ErrorV2;
export type IdentityEntitlementsReadRequest = { readonly 0: Subject; readonly 1: NfcText256; readonly 2?: Hash32 };
export type IdentityEntitlementsReadAccepted = AcceptedState;
export type IdentityEntitlementsReadProgress = ProgressState;
export type IdentityEntitlementsReadResult = { readonly 0: boolean; readonly 1: NfcText256; readonly 2: U32; readonly 3: U64; readonly 4: U64; readonly 5: Fin };
export type IdentityEntitlementsReadError = ErrorV2;
export type TransactionSignRequest = { readonly 0: Hash32; readonly 1: Hash32; readonly 2: U64 };
export type TransactionSignAccepted = AcceptedState;
export type TransactionSignProgress = ProgressState;
export type TransactionSignResult = { readonly 0: Hash32; readonly 1: Fin };
export type TransactionSignError = ErrorV2;
export type OperationRequest = StorageBucketCreateRequest | StorageBucketGetRequest | StorageBucketGrantRequest | StorageBucketRevokeRequest | StorageObjectPutRequest | StorageObjectGetRequest | StorageObjectRangeRequest | StorageObjectDeleteRequest | StorageObjectStatusRequest | StorageCheckpointStatusRequest | StorageCheckpointSubscribeRequest | StorageReplicaStatusRequest | StorageReplicaSubscribeRequest | StorageDeletionStatusRequest | StorageDeletionSubscribeRequest | StorageDriveReadRequest | StorageDriveCommitRequest | StorageDriveShareRequest | StorageS3PutRequest | StorageS3GetRequest | StorageS3ListRequest | StorageS3DeleteRequest | StoragePublishRequest | StorageResolveRequest | StorageKeysExportRequest | StorageKeysImportRequest | IdentityAccountRequest | IdentityProfileReadRequest | IdentityProfileDiscloseRequest | IdentityHumanityStatusRequest | IdentityHumanityProveRequest | IdentitySubjectDeriveRequest | IdentityEntitlementsReadRequest | TransactionSignRequest;
export type AcceptedEventV2 = { readonly 0: 2; readonly 1: RequestId; readonly 2: U32; readonly 3: 0; readonly 4: AllAccepted };
export type ProgressEventV2 = { readonly 0: 2; readonly 1: RequestId; readonly 2: U32; readonly 3: 1; readonly 4: AllProgress };
export type ResultEventV2 = { readonly 0: 2; readonly 1: RequestId; readonly 2: U32; readonly 3: 2; readonly 4: AllResult };
export type ErrorEventV2 = { readonly 0: 2; readonly 1: RequestId; readonly 2: U32; readonly 3: 3; readonly 4: AllError };
export type CancelledEventV2 = { readonly 0: 2; readonly 1: RequestId; readonly 2: U32; readonly 3: 4; readonly 4: { readonly 0: 107 } };
export type StorageCheckpointSubscribeSubscriptionEvent = { readonly 0: OperationId; readonly 1: U64; readonly 2: Fin; readonly 3: CheckpointV2; readonly 4: Hash32 };
export type StorageCheckpointSubscribeUnsubscribeAck = SubscriptionAckV1;
export type StorageReplicaSubscribeSubscriptionEvent = { readonly 0: OperationId; readonly 1: U64; readonly 2: Fin; readonly 3: ProviderId; readonly 4: readonly (ProviderId)[]; readonly 5: Hash32 };
export type StorageReplicaSubscribeUnsubscribeAck = SubscriptionAckV1;
export type StorageDeletionSubscribeSubscriptionEvent = { readonly 0: OperationId; readonly 1: U64; readonly 2: Fin; readonly 3: Cid; readonly 4: U32; readonly 5: U32; readonly 6: Hash32 };
export type StorageDeletionSubscribeUnsubscribeAck = SubscriptionAckV1;
export type AllAccepted = StorageBucketCreateAccepted | StorageBucketGetAccepted | StorageBucketGrantAccepted | StorageBucketRevokeAccepted | StorageObjectPutAccepted | StorageObjectGetAccepted | StorageObjectRangeAccepted | StorageObjectDeleteAccepted | StorageObjectStatusAccepted | StorageCheckpointStatusAccepted | StorageCheckpointSubscribeAccepted | StorageReplicaStatusAccepted | StorageReplicaSubscribeAccepted | StorageDeletionStatusAccepted | StorageDeletionSubscribeAccepted | StorageDriveReadAccepted | StorageDriveCommitAccepted | StorageDriveShareAccepted | StorageS3PutAccepted | StorageS3GetAccepted | StorageS3ListAccepted | StorageS3DeleteAccepted | StoragePublishAccepted | StorageResolveAccepted | StorageKeysExportAccepted | StorageKeysImportAccepted | IdentityAccountAccepted | IdentityProfileReadAccepted | IdentityProfileDiscloseAccepted | IdentityHumanityStatusAccepted | IdentityHumanityProveAccepted | IdentitySubjectDeriveAccepted | IdentityEntitlementsReadAccepted | TransactionSignAccepted;
export type AllProgress = StorageBucketCreateProgress | StorageBucketGetProgress | StorageBucketGrantProgress | StorageBucketRevokeProgress | StorageObjectPutProgress | StorageObjectGetProgress | StorageObjectRangeProgress | StorageObjectDeleteProgress | StorageObjectStatusProgress | StorageCheckpointStatusProgress | StorageCheckpointSubscribeProgress | StorageReplicaStatusProgress | StorageReplicaSubscribeProgress | StorageDeletionStatusProgress | StorageDeletionSubscribeProgress | StorageDriveReadProgress | StorageDriveCommitProgress | StorageDriveShareProgress | StorageS3PutProgress | StorageS3GetProgress | StorageS3ListProgress | StorageS3DeleteProgress | StoragePublishProgress | StorageResolveProgress | StorageKeysExportProgress | StorageKeysImportProgress | IdentityAccountProgress | IdentityProfileReadProgress | IdentityProfileDiscloseProgress | IdentityHumanityStatusProgress | IdentityHumanityProveProgress | IdentitySubjectDeriveProgress | IdentityEntitlementsReadProgress | TransactionSignProgress;
export type AllResult = StorageBucketCreateResult | StorageBucketGetResult | StorageBucketGrantResult | StorageBucketRevokeResult | StorageObjectPutResult | StorageObjectGetResult | StorageObjectRangeResult | StorageObjectDeleteResult | StorageObjectStatusResult | StorageCheckpointStatusResult | StorageCheckpointSubscribeResult | StorageReplicaStatusResult | StorageReplicaSubscribeResult | StorageDeletionStatusResult | StorageDeletionSubscribeResult | StorageDriveReadResult | StorageDriveCommitResult | StorageDriveShareResult | StorageS3PutResult | StorageS3GetResult | StorageS3ListResult | StorageS3DeleteResult | StoragePublishResult | StorageResolveResult | StorageKeysExportResult | StorageKeysImportResult | IdentityAccountResult | IdentityProfileReadResult | IdentityProfileDiscloseResult | IdentityHumanityStatusResult | IdentityHumanityProveResult | IdentitySubjectDeriveResult | IdentityEntitlementsReadResult | TransactionSignResult;
export type AllError = StorageBucketCreateError | StorageBucketGetError | StorageBucketGrantError | StorageBucketRevokeError | StorageObjectPutError | StorageObjectGetError | StorageObjectRangeError | StorageObjectDeleteError | StorageObjectStatusError | StorageCheckpointStatusError | StorageCheckpointSubscribeError | StorageReplicaStatusError | StorageReplicaSubscribeError | StorageDeletionStatusError | StorageDeletionSubscribeError | StorageDriveReadError | StorageDriveCommitError | StorageDriveShareError | StorageS3PutError | StorageS3GetError | StorageS3ListError | StorageS3DeleteError | StoragePublishError | StorageResolveError | StorageKeysExportError | StorageKeysImportError | IdentityAccountError | IdentityProfileReadError | IdentityProfileDiscloseError | IdentityHumanityStatusError | IdentityHumanityProveError | IdentitySubjectDeriveError | IdentityEntitlementsReadError | TransactionSignError;
export type SubjectContextV2 = { readonly 0: 2; readonly 1: Hash32; readonly 2: ProductId; readonly 3: NfcText256; readonly 4: NfcText256; readonly 5: U32; readonly 6: Hash32; readonly 7: boolean };
export type SubjectProofV2 = { readonly 0: 2; readonly 1: Hash32; readonly 2: ProductId; readonly 3: NfcText256; readonly 4: NfcText256; readonly 5: U32; readonly 6: Hash32; readonly 7: boolean; readonly 8: Subject; readonly 9: Uint8Array; readonly 10: Uint8Array; readonly 11: U64; readonly 12: U64; readonly 13: Nonce };
export type ProviderCapabilityV1 = { readonly 0: 1; readonly 1: Hash32; readonly 2: Hash32; readonly 3: GrantId; readonly 4: KeyId; readonly 5: ProductId; readonly 6: BucketId; readonly 7?: AgreementId; readonly 8: ProviderId; readonly 9: readonly (U16)[]; readonly 10?: Cid; readonly 11: U64; readonly 12: U64; readonly 13: U64; readonly 14: Nonce; readonly 15: Uint8Array };
export type ResumeTokenV1 = { readonly 0: 1; readonly 1: Hash32; readonly 2: Hash32; readonly 3: ProviderId; readonly 4: KeyId; readonly 5: OperationId; readonly 6: BucketId; readonly 7: Cid; readonly 8: U64; readonly 9: U32; readonly 10: U64; readonly 11: U64; readonly 12: U64; readonly 13: Nonce; readonly 14: boolean; readonly 15: Uint8Array };
export type ResponseAckV1 = { readonly 0: RequestId; readonly 1: OperationId; readonly 2: U64; readonly 3: Hash32 };
export type HostOutboxEntryV1 = { readonly 0: 1; readonly 1: OperationId; readonly 2: number | bigint; readonly 3: Bytes4MiB; readonly 4: Uint8Array; readonly 5: Hash32; readonly 6: RequestId; readonly 7: OperationId; readonly 8: U64; readonly 9: U32; readonly 10: Hash32; readonly 11: Hash32; readonly 12: Hash32; readonly 13: ProviderId; readonly 14: Hash32; readonly 15: U16; readonly 16?: Hash32; readonly 17: U64; readonly 18: U64; readonly 19: U64; readonly 20: U32 };
export type RecoveryEntryV1 = { readonly 0: OperationId; readonly 1: RequestId; readonly 2: U64; readonly 3: Hash32; readonly 4: Nonce; readonly 5: U32; readonly 6: U32; readonly 7: Bytes4MiB; readonly 8?: Uint8Array; readonly 9: Hash32; readonly 10: Hash32; readonly 11: boolean; readonly 12: U64 };
export type CheckpointSubmissionV2 = { readonly 0: 2; readonly 1: BucketId; readonly 2: Hash32; readonly 3: U64; readonly 4: U64; readonly 5: U32; readonly 6: ProviderId; readonly 7: Uint8Array; readonly 8: U32; readonly 9: U32; readonly 10: readonly (ProviderId)[]; readonly 11: U64 };
export type CheckpointResultV2 = { readonly 0: 2; readonly 1: BucketId; readonly 2: Hash32; readonly 3: U32; readonly 4: U32; readonly 5: U64 };
export type ProviderTransferRequestV1 = { readonly 0: 1; readonly 1: OperationId; readonly 2: BucketId; readonly 3: Cid; readonly 4: U64; readonly 5: U32; readonly 6: ProviderId; readonly 7: ProviderId; readonly 8: Hash32 };
export type ProviderTransferChunkV1 = { readonly 0: 1; readonly 1: OperationId; readonly 2: U32; readonly 3: Uint8Array; readonly 4: Hash32 };
export type ProviderTransferReceiptV1 = { readonly 0: 1; readonly 1: OperationId; readonly 2: Cid; readonly 3: U64; readonly 4: U32; readonly 5: ProviderId; readonly 6: Hash32; readonly 7: Uint8Array };
export type SubjectProofEnvelopeV2 = { readonly 0: SubjectProofV2; readonly 1: Uint8Array };
export type RecoveryInstallV2 = { readonly 0: 2; readonly 1: OperationId; readonly 2: Uint8Array; readonly 3: Hash32; readonly 4: 0; readonly 5: false; readonly 6?: Hash32; readonly 7: U64 };
export type RecoveryReceiptV2 = { readonly 0: 2; readonly 1: OperationId; readonly 2: Hash32; readonly 3: Hash32; readonly 4: 0; readonly 5: false; readonly 6?: Hash32; readonly 7: U64; readonly 8: Uint8Array };
export type DriveFileManifestV1 = { readonly 0: 1; readonly 1: readonly (Cid)[]; readonly 2: U64; readonly 3: U64; readonly 4: string; readonly 5: Uint8Array };
export type DriveManifestV1 = { readonly 0: 1; readonly 1: BucketId; readonly 2: U64; readonly 3: readonly (Cid)[]; readonly 4: Hash32 };
export type DriveChangedEventV1 = { readonly 0: BucketId; readonly 1: Cid; readonly 2: Cid; readonly 3: U64; readonly 4: U64; readonly 5: Hash32 };
export type S3ObjectVersionV1 = { readonly 0: 1; readonly 1: string; readonly 2: Uint8Array; readonly 3: Cid; readonly 4: string; readonly 5: U64; readonly 6: boolean; readonly 7: Hash32 };
export type S3ChangedEventV1 = { readonly 0: string; readonly 1: Uint8Array; readonly 2: U64; readonly 3: number | bigint; readonly 4: string; readonly 5: Hash32 };
export type DurableStateV1 = { readonly 0: 1; readonly 1: U64; readonly 2: Hash32; readonly 3: U64; readonly 4: U32 };
export interface HostV2TypeMap {
  readonly Hash32: Hash32;
  readonly AccountId32: AccountId32;
  readonly ProviderId: ProviderId;
  readonly BucketId: BucketId;
  readonly AgreementId: AgreementId;
  readonly GrantId: GrantId;
  readonly Subject: Subject;
  readonly KeyId: KeyId;
  readonly RequestId: RequestId;
  readonly OperationId: OperationId;
  readonly Nonce: Nonce;
  readonly Cid: Cid;
  readonly ProductId: ProductId;
  readonly NfcText128: NfcText128;
  readonly NfcText256: NfcText256;
  readonly Bytes4MiB: Bytes4MiB;
  readonly U16: U16;
  readonly U32: U32;
  readonly U64: U64;
  readonly EventV2: EventV2;
  readonly ErrorDetails: ErrorDetails;
  readonly RequestV2: RequestV2;
  readonly StorageBucketCreateFrame: StorageBucketCreateFrame;
  readonly StorageBucketGetFrame: StorageBucketGetFrame;
  readonly StorageBucketGrantFrame: StorageBucketGrantFrame;
  readonly StorageBucketRevokeFrame: StorageBucketRevokeFrame;
  readonly StorageObjectPutFrame: StorageObjectPutFrame;
  readonly StorageObjectGetFrame: StorageObjectGetFrame;
  readonly StorageObjectRangeFrame: StorageObjectRangeFrame;
  readonly StorageObjectDeleteFrame: StorageObjectDeleteFrame;
  readonly StorageObjectStatusFrame: StorageObjectStatusFrame;
  readonly StorageCheckpointStatusFrame: StorageCheckpointStatusFrame;
  readonly StorageCheckpointSubscribeFrame: StorageCheckpointSubscribeFrame;
  readonly StorageReplicaStatusFrame: StorageReplicaStatusFrame;
  readonly StorageReplicaSubscribeFrame: StorageReplicaSubscribeFrame;
  readonly StorageDeletionStatusFrame: StorageDeletionStatusFrame;
  readonly StorageDeletionSubscribeFrame: StorageDeletionSubscribeFrame;
  readonly StorageDriveReadFrame: StorageDriveReadFrame;
  readonly StorageDriveCommitFrame: StorageDriveCommitFrame;
  readonly StorageDriveShareFrame: StorageDriveShareFrame;
  readonly StorageS3PutFrame: StorageS3PutFrame;
  readonly StorageS3GetFrame: StorageS3GetFrame;
  readonly StorageS3ListFrame: StorageS3ListFrame;
  readonly StorageS3DeleteFrame: StorageS3DeleteFrame;
  readonly StoragePublishFrame: StoragePublishFrame;
  readonly StorageResolveFrame: StorageResolveFrame;
  readonly StorageKeysExportFrame: StorageKeysExportFrame;
  readonly StorageKeysImportFrame: StorageKeysImportFrame;
  readonly IdentityAccountFrame: IdentityAccountFrame;
  readonly IdentityProfileReadFrame: IdentityProfileReadFrame;
  readonly IdentityProfileDiscloseFrame: IdentityProfileDiscloseFrame;
  readonly IdentityHumanityStatusFrame: IdentityHumanityStatusFrame;
  readonly IdentityHumanityProveFrame: IdentityHumanityProveFrame;
  readonly IdentitySubjectDeriveFrame: IdentitySubjectDeriveFrame;
  readonly IdentityEntitlementsReadFrame: IdentityEntitlementsReadFrame;
  readonly TransactionSignFrame: TransactionSignFrame;
  readonly ErrorV2: ErrorV2;
  readonly Error100: Error100;
  readonly Error101: Error101;
  readonly Error102: Error102;
  readonly Error103: Error103;
  readonly Error104: Error104;
  readonly Error105: Error105;
  readonly Error106: Error106;
  readonly Error107: Error107;
  readonly Error108: Error108;
  readonly Error109: Error109;
  readonly Error110: Error110;
  readonly Error111: Error111;
  readonly Error112: Error112;
  readonly Error113: Error113;
  readonly Error114: Error114;
  readonly Error115: Error115;
  readonly Error116: Error116;
  readonly Error200: Error200;
  readonly Error201: Error201;
  readonly Error202: Error202;
  readonly Error203: Error203;
  readonly Error204: Error204;
  readonly Error205: Error205;
  readonly Error206: Error206;
  readonly Error207: Error207;
  readonly Error208: Error208;
  readonly Error209: Error209;
  readonly Error210: Error210;
  readonly Error211: Error211;
  readonly Error220: Error220;
  readonly Error221: Error221;
  readonly Error222: Error222;
  readonly Error223: Error223;
  readonly Error224: Error224;
  readonly Error225: Error225;
  readonly Error226: Error226;
  readonly Error227: Error227;
  readonly Error228: Error228;
  readonly Error229: Error229;
  readonly Error230: Error230;
  readonly Error231: Error231;
  readonly Error232: Error232;
  readonly Error233: Error233;
  readonly Error234: Error234;
  readonly Error235: Error235;
  readonly Error236: Error236;
  readonly Error237: Error237;
  readonly Error238: Error238;
  readonly Error239: Error239;
  readonly Error240: Error240;
  readonly Error241: Error241;
  readonly Error250: Error250;
  readonly Error251: Error251;
  readonly Error252: Error252;
  readonly Error253: Error253;
  readonly Error254: Error254;
  readonly Error255: Error255;
  readonly Error256: Error256;
  readonly Error257: Error257;
  readonly Error258: Error258;
  readonly Error259: Error259;
  readonly Error260: Error260;
  readonly Error261: Error261;
  readonly Error300: Error300;
  readonly Error301: Error301;
  readonly Error302: Error302;
  readonly Error303: Error303;
  readonly Error304: Error304;
  readonly Error305: Error305;
  readonly Error306: Error306;
  readonly Error307: Error307;
  readonly Error320: Error320;
  readonly Error321: Error321;
  readonly Error322: Error322;
  readonly Error323: Error323;
  readonly Error324: Error324;
  readonly Error325: Error325;
  readonly Error400: Error400;
  readonly Error401: Error401;
  readonly Error402: Error402;
  readonly Error403: Error403;
  readonly Error404: Error404;
  readonly Error405: Error405;
  readonly Error406: Error406;
  readonly Error407: Error407;
  readonly Error408: Error408;
  readonly Error409: Error409;
  readonly Error410: Error410;
  readonly Error411: Error411;
  readonly Error412: Error412;
  readonly Error413: Error413;
  readonly Empty: Empty;
  readonly Fin: Fin;
  readonly AcceptedState: AcceptedState;
  readonly ProgressState: ProgressState;
  readonly ResultState: ResultState;
  readonly BucketV1: BucketV1;
  readonly ProviderReceiptV1: ProviderReceiptV1;
  readonly CheckpointV2: CheckpointV2;
  readonly SubscriptionAckV1: SubscriptionAckV1;
  readonly IdentityReceiptV2: IdentityReceiptV2;
  readonly StorageBucketCreateRequest: StorageBucketCreateRequest;
  readonly StorageBucketCreateAccepted: StorageBucketCreateAccepted;
  readonly StorageBucketCreateProgress: StorageBucketCreateProgress;
  readonly StorageBucketCreateResult: StorageBucketCreateResult;
  readonly StorageBucketCreateError: StorageBucketCreateError;
  readonly StorageBucketGetRequest: StorageBucketGetRequest;
  readonly StorageBucketGetAccepted: StorageBucketGetAccepted;
  readonly StorageBucketGetProgress: StorageBucketGetProgress;
  readonly StorageBucketGetResult: StorageBucketGetResult;
  readonly StorageBucketGetError: StorageBucketGetError;
  readonly StorageBucketGrantRequest: StorageBucketGrantRequest;
  readonly StorageBucketGrantAccepted: StorageBucketGrantAccepted;
  readonly StorageBucketGrantProgress: StorageBucketGrantProgress;
  readonly StorageBucketGrantResult: StorageBucketGrantResult;
  readonly StorageBucketGrantError: StorageBucketGrantError;
  readonly StorageBucketRevokeRequest: StorageBucketRevokeRequest;
  readonly StorageBucketRevokeAccepted: StorageBucketRevokeAccepted;
  readonly StorageBucketRevokeProgress: StorageBucketRevokeProgress;
  readonly StorageBucketRevokeResult: StorageBucketRevokeResult;
  readonly StorageBucketRevokeError: StorageBucketRevokeError;
  readonly StorageObjectPutRequest: StorageObjectPutRequest;
  readonly StorageObjectPutAccepted: StorageObjectPutAccepted;
  readonly StorageObjectPutProgress: StorageObjectPutProgress;
  readonly StorageObjectPutResult: StorageObjectPutResult;
  readonly StorageObjectPutError: StorageObjectPutError;
  readonly StorageObjectGetRequest: StorageObjectGetRequest;
  readonly StorageObjectGetAccepted: StorageObjectGetAccepted;
  readonly StorageObjectGetProgress: StorageObjectGetProgress;
  readonly StorageObjectGetResult: StorageObjectGetResult;
  readonly StorageObjectGetError: StorageObjectGetError;
  readonly StorageObjectRangeRequest: StorageObjectRangeRequest;
  readonly StorageObjectRangeAccepted: StorageObjectRangeAccepted;
  readonly StorageObjectRangeProgress: StorageObjectRangeProgress;
  readonly StorageObjectRangeResult: StorageObjectRangeResult;
  readonly StorageObjectRangeError: StorageObjectRangeError;
  readonly StorageObjectDeleteRequest: StorageObjectDeleteRequest;
  readonly StorageObjectDeleteAccepted: StorageObjectDeleteAccepted;
  readonly StorageObjectDeleteProgress: StorageObjectDeleteProgress;
  readonly StorageObjectDeleteResult: StorageObjectDeleteResult;
  readonly StorageObjectDeleteError: StorageObjectDeleteError;
  readonly StorageObjectStatusRequest: StorageObjectStatusRequest;
  readonly StorageObjectStatusAccepted: StorageObjectStatusAccepted;
  readonly StorageObjectStatusProgress: StorageObjectStatusProgress;
  readonly StorageObjectStatusResult: StorageObjectStatusResult;
  readonly StorageObjectStatusError: StorageObjectStatusError;
  readonly StorageCheckpointStatusRequest: StorageCheckpointStatusRequest;
  readonly StorageCheckpointStatusAccepted: StorageCheckpointStatusAccepted;
  readonly StorageCheckpointStatusProgress: StorageCheckpointStatusProgress;
  readonly StorageCheckpointStatusResult: StorageCheckpointStatusResult;
  readonly StorageCheckpointStatusError: StorageCheckpointStatusError;
  readonly StorageCheckpointSubscribeRequest: StorageCheckpointSubscribeRequest;
  readonly StorageCheckpointSubscribeAccepted: StorageCheckpointSubscribeAccepted;
  readonly StorageCheckpointSubscribeProgress: StorageCheckpointSubscribeProgress;
  readonly StorageCheckpointSubscribeResult: StorageCheckpointSubscribeResult;
  readonly StorageCheckpointSubscribeError: StorageCheckpointSubscribeError;
  readonly StorageReplicaStatusRequest: StorageReplicaStatusRequest;
  readonly StorageReplicaStatusAccepted: StorageReplicaStatusAccepted;
  readonly StorageReplicaStatusProgress: StorageReplicaStatusProgress;
  readonly StorageReplicaStatusResult: StorageReplicaStatusResult;
  readonly StorageReplicaStatusError: StorageReplicaStatusError;
  readonly StorageReplicaSubscribeRequest: StorageReplicaSubscribeRequest;
  readonly StorageReplicaSubscribeAccepted: StorageReplicaSubscribeAccepted;
  readonly StorageReplicaSubscribeProgress: StorageReplicaSubscribeProgress;
  readonly StorageReplicaSubscribeResult: StorageReplicaSubscribeResult;
  readonly StorageReplicaSubscribeError: StorageReplicaSubscribeError;
  readonly StorageDeletionStatusRequest: StorageDeletionStatusRequest;
  readonly StorageDeletionStatusAccepted: StorageDeletionStatusAccepted;
  readonly StorageDeletionStatusProgress: StorageDeletionStatusProgress;
  readonly StorageDeletionStatusResult: StorageDeletionStatusResult;
  readonly StorageDeletionStatusError: StorageDeletionStatusError;
  readonly StorageDeletionSubscribeRequest: StorageDeletionSubscribeRequest;
  readonly StorageDeletionSubscribeAccepted: StorageDeletionSubscribeAccepted;
  readonly StorageDeletionSubscribeProgress: StorageDeletionSubscribeProgress;
  readonly StorageDeletionSubscribeResult: StorageDeletionSubscribeResult;
  readonly StorageDeletionSubscribeError: StorageDeletionSubscribeError;
  readonly StorageDriveReadRequest: StorageDriveReadRequest;
  readonly StorageDriveReadAccepted: StorageDriveReadAccepted;
  readonly StorageDriveReadProgress: StorageDriveReadProgress;
  readonly StorageDriveReadResult: StorageDriveReadResult;
  readonly StorageDriveReadError: StorageDriveReadError;
  readonly StorageDriveCommitRequest: StorageDriveCommitRequest;
  readonly StorageDriveCommitAccepted: StorageDriveCommitAccepted;
  readonly StorageDriveCommitProgress: StorageDriveCommitProgress;
  readonly StorageDriveCommitResult: StorageDriveCommitResult;
  readonly StorageDriveCommitError: StorageDriveCommitError;
  readonly StorageDriveShareRequest: StorageDriveShareRequest;
  readonly StorageDriveShareAccepted: StorageDriveShareAccepted;
  readonly StorageDriveShareProgress: StorageDriveShareProgress;
  readonly StorageDriveShareResult: StorageDriveShareResult;
  readonly StorageDriveShareError: StorageDriveShareError;
  readonly StorageS3PutRequest: StorageS3PutRequest;
  readonly StorageS3PutAccepted: StorageS3PutAccepted;
  readonly StorageS3PutProgress: StorageS3PutProgress;
  readonly StorageS3PutResult: StorageS3PutResult;
  readonly StorageS3PutError: StorageS3PutError;
  readonly StorageS3GetRequest: StorageS3GetRequest;
  readonly StorageS3GetAccepted: StorageS3GetAccepted;
  readonly StorageS3GetProgress: StorageS3GetProgress;
  readonly StorageS3GetResult: StorageS3GetResult;
  readonly StorageS3GetError: StorageS3GetError;
  readonly StorageS3ListRequest: StorageS3ListRequest;
  readonly StorageS3ListAccepted: StorageS3ListAccepted;
  readonly StorageS3ListProgress: StorageS3ListProgress;
  readonly StorageS3ListResult: StorageS3ListResult;
  readonly StorageS3ListError: StorageS3ListError;
  readonly StorageS3DeleteRequest: StorageS3DeleteRequest;
  readonly StorageS3DeleteAccepted: StorageS3DeleteAccepted;
  readonly StorageS3DeleteProgress: StorageS3DeleteProgress;
  readonly StorageS3DeleteResult: StorageS3DeleteResult;
  readonly StorageS3DeleteError: StorageS3DeleteError;
  readonly StoragePublishRequest: StoragePublishRequest;
  readonly StoragePublishAccepted: StoragePublishAccepted;
  readonly StoragePublishProgress: StoragePublishProgress;
  readonly StoragePublishResult: StoragePublishResult;
  readonly StoragePublishError: StoragePublishError;
  readonly StorageResolveRequest: StorageResolveRequest;
  readonly StorageResolveAccepted: StorageResolveAccepted;
  readonly StorageResolveProgress: StorageResolveProgress;
  readonly StorageResolveResult: StorageResolveResult;
  readonly StorageResolveError: StorageResolveError;
  readonly StorageKeysExportRequest: StorageKeysExportRequest;
  readonly StorageKeysExportAccepted: StorageKeysExportAccepted;
  readonly StorageKeysExportProgress: StorageKeysExportProgress;
  readonly StorageKeysExportResult: StorageKeysExportResult;
  readonly StorageKeysExportError: StorageKeysExportError;
  readonly StorageKeysImportRequest: StorageKeysImportRequest;
  readonly StorageKeysImportAccepted: StorageKeysImportAccepted;
  readonly StorageKeysImportProgress: StorageKeysImportProgress;
  readonly StorageKeysImportResult: StorageKeysImportResult;
  readonly StorageKeysImportError: StorageKeysImportError;
  readonly IdentityAccountRequest: IdentityAccountRequest;
  readonly IdentityAccountAccepted: IdentityAccountAccepted;
  readonly IdentityAccountProgress: IdentityAccountProgress;
  readonly IdentityAccountResult: IdentityAccountResult;
  readonly IdentityAccountError: IdentityAccountError;
  readonly IdentityProfileReadRequest: IdentityProfileReadRequest;
  readonly IdentityProfileReadAccepted: IdentityProfileReadAccepted;
  readonly IdentityProfileReadProgress: IdentityProfileReadProgress;
  readonly IdentityProfileReadResult: IdentityProfileReadResult;
  readonly IdentityProfileReadError: IdentityProfileReadError;
  readonly IdentityProfileDiscloseRequest: IdentityProfileDiscloseRequest;
  readonly IdentityProfileDiscloseAccepted: IdentityProfileDiscloseAccepted;
  readonly IdentityProfileDiscloseProgress: IdentityProfileDiscloseProgress;
  readonly IdentityProfileDiscloseResult: IdentityProfileDiscloseResult;
  readonly IdentityProfileDiscloseError: IdentityProfileDiscloseError;
  readonly IdentityHumanityStatusRequest: IdentityHumanityStatusRequest;
  readonly IdentityHumanityStatusAccepted: IdentityHumanityStatusAccepted;
  readonly IdentityHumanityStatusProgress: IdentityHumanityStatusProgress;
  readonly IdentityHumanityStatusResult: IdentityHumanityStatusResult;
  readonly IdentityHumanityStatusError: IdentityHumanityStatusError;
  readonly IdentityHumanityProveRequest: IdentityHumanityProveRequest;
  readonly IdentityHumanityProveAccepted: IdentityHumanityProveAccepted;
  readonly IdentityHumanityProveProgress: IdentityHumanityProveProgress;
  readonly IdentityHumanityProveResult: IdentityHumanityProveResult;
  readonly IdentityHumanityProveError: IdentityHumanityProveError;
  readonly IdentitySubjectDeriveRequest: IdentitySubjectDeriveRequest;
  readonly IdentitySubjectDeriveAccepted: IdentitySubjectDeriveAccepted;
  readonly IdentitySubjectDeriveProgress: IdentitySubjectDeriveProgress;
  readonly IdentitySubjectDeriveResult: IdentitySubjectDeriveResult;
  readonly IdentitySubjectDeriveError: IdentitySubjectDeriveError;
  readonly IdentityEntitlementsReadRequest: IdentityEntitlementsReadRequest;
  readonly IdentityEntitlementsReadAccepted: IdentityEntitlementsReadAccepted;
  readonly IdentityEntitlementsReadProgress: IdentityEntitlementsReadProgress;
  readonly IdentityEntitlementsReadResult: IdentityEntitlementsReadResult;
  readonly IdentityEntitlementsReadError: IdentityEntitlementsReadError;
  readonly TransactionSignRequest: TransactionSignRequest;
  readonly TransactionSignAccepted: TransactionSignAccepted;
  readonly TransactionSignProgress: TransactionSignProgress;
  readonly TransactionSignResult: TransactionSignResult;
  readonly TransactionSignError: TransactionSignError;
  readonly OperationRequest: OperationRequest;
  readonly AcceptedEventV2: AcceptedEventV2;
  readonly ProgressEventV2: ProgressEventV2;
  readonly ResultEventV2: ResultEventV2;
  readonly ErrorEventV2: ErrorEventV2;
  readonly CancelledEventV2: CancelledEventV2;
  readonly StorageCheckpointSubscribeSubscriptionEvent: StorageCheckpointSubscribeSubscriptionEvent;
  readonly StorageCheckpointSubscribeUnsubscribeAck: StorageCheckpointSubscribeUnsubscribeAck;
  readonly StorageReplicaSubscribeSubscriptionEvent: StorageReplicaSubscribeSubscriptionEvent;
  readonly StorageReplicaSubscribeUnsubscribeAck: StorageReplicaSubscribeUnsubscribeAck;
  readonly StorageDeletionSubscribeSubscriptionEvent: StorageDeletionSubscribeSubscriptionEvent;
  readonly StorageDeletionSubscribeUnsubscribeAck: StorageDeletionSubscribeUnsubscribeAck;
  readonly AllAccepted: AllAccepted;
  readonly AllProgress: AllProgress;
  readonly AllResult: AllResult;
  readonly AllError: AllError;
  readonly SubjectContextV2: SubjectContextV2;
  readonly SubjectProofV2: SubjectProofV2;
  readonly ProviderCapabilityV1: ProviderCapabilityV1;
  readonly ResumeTokenV1: ResumeTokenV1;
  readonly ResponseAckV1: ResponseAckV1;
  readonly HostOutboxEntryV1: HostOutboxEntryV1;
  readonly RecoveryEntryV1: RecoveryEntryV1;
  readonly CheckpointSubmissionV2: CheckpointSubmissionV2;
  readonly CheckpointResultV2: CheckpointResultV2;
  readonly ProviderTransferRequestV1: ProviderTransferRequestV1;
  readonly ProviderTransferChunkV1: ProviderTransferChunkV1;
  readonly ProviderTransferReceiptV1: ProviderTransferReceiptV1;
  readonly SubjectProofEnvelopeV2: SubjectProofEnvelopeV2;
  readonly RecoveryInstallV2: RecoveryInstallV2;
  readonly RecoveryReceiptV2: RecoveryReceiptV2;
  readonly DriveFileManifestV1: DriveFileManifestV1;
  readonly DriveManifestV1: DriveManifestV1;
  readonly DriveChangedEventV1: DriveChangedEventV1;
  readonly S3ObjectVersionV1: S3ObjectVersionV1;
  readonly S3ChangedEventV1: S3ChangedEventV1;
  readonly DurableStateV1: DurableStateV1;
}
export const HOST_V2_SCHEMAS:Readonly<Record<HostV2TypeName,HostV2SchemaNode>>={"Hash32":{"kind":"bytes","min":32,"max":32},"AccountId32":{"kind":"bytes","min":32,"max":32},"ProviderId":{"kind":"bytes","min":32,"max":32},"BucketId":{"kind":"bytes","min":32,"max":32},"AgreementId":{"kind":"bytes","min":32,"max":32},"GrantId":{"kind":"bytes","min":32,"max":32},"Subject":{"kind":"bytes","min":32,"max":32},"KeyId":{"kind":"bytes","min":32,"max":32},"RequestId":{"kind":"bytes","min":16,"max":16},"OperationId":{"kind":"bytes","min":16,"max":16},"Nonce":{"kind":"bytes","min":16,"max":16},"Cid":{"kind":"text","min":1,"max":128,"nfc":true},"ProductId":{"kind":"text","min":1,"max":128,"nfc":true},"NfcText128":{"kind":"text","min":1,"max":128,"nfc":true},"NfcText256":{"kind":"text","min":1,"max":256,"nfc":true},"Bytes4MiB":{"kind":"bytes","min":0,"max":4194304},"U16":{"kind":"uint","min":"0","max":"65535"},"U32":{"kind":"uint","min":"0","max":"4294967295"},"U64":{"kind":"uint","min":"0","max":"18446744073709551615"},"EventV2":{"kind":"union","variants":[{"kind":"ref","name":"AcceptedEventV2"},{"kind":"ref","name":"ProgressEventV2"},{"kind":"ref","name":"ResultEventV2"},{"kind":"ref","name":"ErrorEventV2"},{"kind":"ref","name":"CancelledEventV2"}]},"ErrorDetails":{"kind":"map","fields":[{"key":0,"required":false,"schema":{"kind":"ref","name":"NfcText256"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"RequestV2":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateFrame"},{"kind":"ref","name":"StorageBucketGetFrame"},{"kind":"ref","name":"StorageBucketGrantFrame"},{"kind":"ref","name":"StorageBucketRevokeFrame"},{"kind":"ref","name":"StorageObjectPutFrame"},{"kind":"ref","name":"StorageObjectGetFrame"},{"kind":"ref","name":"StorageObjectRangeFrame"},{"kind":"ref","name":"StorageObjectDeleteFrame"},{"kind":"ref","name":"StorageObjectStatusFrame"},{"kind":"ref","name":"StorageCheckpointStatusFrame"},{"kind":"ref","name":"StorageCheckpointSubscribeFrame"},{"kind":"ref","name":"StorageReplicaStatusFrame"},{"kind":"ref","name":"StorageReplicaSubscribeFrame"},{"kind":"ref","name":"StorageDeletionStatusFrame"},{"kind":"ref","name":"StorageDeletionSubscribeFrame"},{"kind":"ref","name":"StorageDriveReadFrame"},{"kind":"ref","name":"StorageDriveCommitFrame"},{"kind":"ref","name":"StorageDriveShareFrame"},{"kind":"ref","name":"StorageS3PutFrame"},{"kind":"ref","name":"StorageS3GetFrame"},{"kind":"ref","name":"StorageS3ListFrame"},{"kind":"ref","name":"StorageS3DeleteFrame"},{"kind":"ref","name":"StoragePublishFrame"},{"kind":"ref","name":"StorageResolveFrame"},{"kind":"ref","name":"StorageKeysExportFrame"},{"kind":"ref","name":"StorageKeysImportFrame"},{"kind":"ref","name":"IdentityAccountFrame"},{"kind":"ref","name":"IdentityProfileReadFrame"},{"kind":"ref","name":"IdentityProfileDiscloseFrame"},{"kind":"ref","name":"IdentityHumanityStatusFrame"},{"kind":"ref","name":"IdentityHumanityProveFrame"},{"kind":"ref","name":"IdentitySubjectDeriveFrame"},{"kind":"ref","name":"IdentityEntitlementsReadFrame"},{"kind":"ref","name":"TransactionSignFrame"}]},"StorageBucketCreateFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1000}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageBucketCreateRequest"}}]},"StorageBucketGetFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1001}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageBucketGetRequest"}}]},"StorageBucketGrantFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1002}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageBucketGrantRequest"}}]},"StorageBucketRevokeFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1003}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageBucketRevokeRequest"}}]},"StorageObjectPutFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1010}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageObjectPutRequest"}}]},"StorageObjectGetFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1011}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageObjectGetRequest"}}]},"StorageObjectRangeFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1012}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageObjectRangeRequest"}}]},"StorageObjectDeleteFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1013}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageObjectDeleteRequest"}}]},"StorageObjectStatusFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1014}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageObjectStatusRequest"}}]},"StorageCheckpointStatusFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1020}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageCheckpointStatusRequest"}}]},"StorageCheckpointSubscribeFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1021}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageCheckpointSubscribeRequest"}}]},"StorageReplicaStatusFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1022}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageReplicaStatusRequest"}}]},"StorageReplicaSubscribeFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1023}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageReplicaSubscribeRequest"}}]},"StorageDeletionStatusFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1024}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageDeletionStatusRequest"}}]},"StorageDeletionSubscribeFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1025}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageDeletionSubscribeRequest"}}]},"StorageDriveReadFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1030}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageDriveReadRequest"}}]},"StorageDriveCommitFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1031}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageDriveCommitRequest"}}]},"StorageDriveShareFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1032}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageDriveShareRequest"}}]},"StorageS3PutFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1040}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageS3PutRequest"}}]},"StorageS3GetFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1041}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageS3GetRequest"}}]},"StorageS3ListFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1042}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageS3ListRequest"}}]},"StorageS3DeleteFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1043}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageS3DeleteRequest"}}]},"StoragePublishFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1050}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StoragePublishRequest"}}]},"StorageResolveFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1051}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageResolveRequest"}}]},"StorageKeysExportFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1060}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageKeysExportRequest"}}]},"StorageKeysImportFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1061}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"StorageKeysImportRequest"}}]},"IdentityAccountFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1100}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityAccountRequest"}}]},"IdentityProfileReadFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1101}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityProfileReadRequest"}}]},"IdentityProfileDiscloseFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1102}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityProfileDiscloseRequest"}}]},"IdentityHumanityStatusFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1103}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityHumanityStatusRequest"}}]},"IdentityHumanityProveFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1104}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityHumanityProveRequest"}}]},"IdentitySubjectDeriveFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1105}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentitySubjectDeriveRequest"}}]},"IdentityEntitlementsReadFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1106}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"IdentityEntitlementsReadRequest"}}]},"TransactionSignFrame":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"const","value":1200}},{"key":4,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":false,"schema":{"kind":"bytes","min":1,"max":64}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"TransactionSignRequest"}}]},"ErrorV2":{"kind":"union","variants":[{"kind":"ref","name":"Error100"},{"kind":"ref","name":"Error101"},{"kind":"ref","name":"Error102"},{"kind":"ref","name":"Error103"},{"kind":"ref","name":"Error104"},{"kind":"ref","name":"Error105"},{"kind":"ref","name":"Error106"},{"kind":"ref","name":"Error107"},{"kind":"ref","name":"Error108"},{"kind":"ref","name":"Error109"},{"kind":"ref","name":"Error110"},{"kind":"ref","name":"Error111"},{"kind":"ref","name":"Error112"},{"kind":"ref","name":"Error113"},{"kind":"ref","name":"Error114"},{"kind":"ref","name":"Error115"},{"kind":"ref","name":"Error116"},{"kind":"ref","name":"Error200"},{"kind":"ref","name":"Error201"},{"kind":"ref","name":"Error202"},{"kind":"ref","name":"Error203"},{"kind":"ref","name":"Error204"},{"kind":"ref","name":"Error205"},{"kind":"ref","name":"Error206"},{"kind":"ref","name":"Error207"},{"kind":"ref","name":"Error208"},{"kind":"ref","name":"Error209"},{"kind":"ref","name":"Error210"},{"kind":"ref","name":"Error211"},{"kind":"ref","name":"Error220"},{"kind":"ref","name":"Error221"},{"kind":"ref","name":"Error222"},{"kind":"ref","name":"Error223"},{"kind":"ref","name":"Error224"},{"kind":"ref","name":"Error225"},{"kind":"ref","name":"Error226"},{"kind":"ref","name":"Error227"},{"kind":"ref","name":"Error228"},{"kind":"ref","name":"Error229"},{"kind":"ref","name":"Error230"},{"kind":"ref","name":"Error231"},{"kind":"ref","name":"Error232"},{"kind":"ref","name":"Error233"},{"kind":"ref","name":"Error234"},{"kind":"ref","name":"Error235"},{"kind":"ref","name":"Error236"},{"kind":"ref","name":"Error237"},{"kind":"ref","name":"Error238"},{"kind":"ref","name":"Error239"},{"kind":"ref","name":"Error240"},{"kind":"ref","name":"Error241"},{"kind":"ref","name":"Error250"},{"kind":"ref","name":"Error251"},{"kind":"ref","name":"Error252"},{"kind":"ref","name":"Error253"},{"kind":"ref","name":"Error254"},{"kind":"ref","name":"Error255"},{"kind":"ref","name":"Error256"},{"kind":"ref","name":"Error257"},{"kind":"ref","name":"Error258"},{"kind":"ref","name":"Error259"},{"kind":"ref","name":"Error260"},{"kind":"ref","name":"Error261"},{"kind":"ref","name":"Error300"},{"kind":"ref","name":"Error301"},{"kind":"ref","name":"Error302"},{"kind":"ref","name":"Error303"},{"kind":"ref","name":"Error304"},{"kind":"ref","name":"Error305"},{"kind":"ref","name":"Error306"},{"kind":"ref","name":"Error307"},{"kind":"ref","name":"Error320"},{"kind":"ref","name":"Error321"},{"kind":"ref","name":"Error322"},{"kind":"ref","name":"Error323"},{"kind":"ref","name":"Error324"},{"kind":"ref","name":"Error325"},{"kind":"ref","name":"Error400"},{"kind":"ref","name":"Error401"},{"kind":"ref","name":"Error402"},{"kind":"ref","name":"Error403"},{"kind":"ref","name":"Error404"},{"kind":"ref","name":"Error405"},{"kind":"ref","name":"Error406"},{"kind":"ref","name":"Error407"},{"kind":"ref","name":"Error408"},{"kind":"ref","name":"Error409"},{"kind":"ref","name":"Error410"},{"kind":"ref","name":"Error411"},{"kind":"ref","name":"Error412"},{"kind":"ref","name":"Error413"}]},"Error100":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":100}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_SCHEMA_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error101":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":101}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_NON_CANONICAL"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error102":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":102}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_VERSION_MISMATCH"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error103":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":103}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_GENESIS_MISMATCH"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error104":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":104}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_DESCRIPTOR_MISMATCH"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error105":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":105}},{"key":1,"required":true,"schema":{"kind":"const","value":"WIRE_SEQUENCE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error106":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":106}},{"key":1,"required":true,"schema":{"kind":"const","value":"REQUEST_DEADLINE_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error107":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":107}},{"key":1,"required":true,"schema":{"kind":"const","value":"REQUEST_CANCELLED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error108":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":108}},{"key":1,"required":true,"schema":{"kind":"const","value":"REQUEST_NOT_FOUND"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error109":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":109}},{"key":1,"required":true,"schema":{"kind":"const","value":"GRANT_REQUIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error110":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":110}},{"key":1,"required":true,"schema":{"kind":"const","value":"GRANT_SCOPE_DENIED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error111":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":111}},{"key":1,"required":true,"schema":{"kind":"const","value":"GRANT_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error112":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":112}},{"key":1,"required":true,"schema":{"kind":"const","value":"GRANT_REVOKED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error113":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":113}},{"key":1,"required":true,"schema":{"kind":"const","value":"HOST_OUTBOX_UNAVAILABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error114":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":114}},{"key":1,"required":true,"schema":{"kind":"const","value":"HOST_OUTBOX_FULL"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error115":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":115}},{"key":1,"required":true,"schema":{"kind":"const","value":"HOST_OUTBOX_CORRUPT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error116":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":116}},{"key":1,"required":true,"schema":{"kind":"const","value":"HOST_OUTBOX_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error200":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":200}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHUNK_OUT_OF_ORDER"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error201":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":201}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHUNK_TOO_LARGE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error202":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":202}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHUNK_MISSING"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error203":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":203}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_LENGTH_MISMATCH"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error204":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":204}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CID_MISMATCH"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error205":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":205}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_OBJECT_TOO_LARGE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error206":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":206}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_IDEMPOTENCY_CONFLICT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error207":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":207}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_RANGE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error208":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":208}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_INTEGRITY_FAILED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error209":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":209}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_NOT_PUBLISHABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error210":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":210}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_NOT_FOUND"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error211":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":211}},{"key":1,"required":true,"schema":{"kind":"const","value":"ENCRYPTION_NONCE_REUSE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error220":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":220}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_WRONG_DOMAIN"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error221":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":221}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_WRONG_VERSION"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error222":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":222}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_WRONG_BUCKET"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error223":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":223}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_WRONG_KEY"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error224":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":224}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_STALE_NONCE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error225":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":225}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_WRONG_WINDOW"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error226":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":226}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_SIGNATURE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error227":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":227}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_AUDIENCE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error228":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":228}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_CONTENT_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error229":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":229}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_NONCE_REPLAY"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error230":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":230}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error231":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":231}},{"key":1,"required":true,"schema":{"kind":"const","value":"CAPABILITY_ISSUER_REVOKED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error232":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":232}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_SIGNATURE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error233":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":233}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_AUDIENCE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error234":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":234}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_REPLAY"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error235":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":235}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error236":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":236}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_REVOKED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error237":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":237}},{"key":1,"required":true,"schema":{"kind":"const","value":"RESUME_CURSOR_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error238":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":238}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_RECOVERY_TABLE_FULL"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error239":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":239}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error240":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":240}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_SEQUENCE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error241":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":241}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CHECKPOINT_EQUIVOCATION"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error250":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":250}},{"key":1,"required":true,"schema":{"kind":"const","value":"BUCKET_NOT_FOUND"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error251":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":251}},{"key":1,"required":true,"schema":{"kind":"const","value":"BUCKET_VERSION_CONFLICT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error252":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":252}},{"key":1,"required":true,"schema":{"kind":"const","value":"BUCKET_MEMBER_LIMIT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error253":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":253}},{"key":1,"required":true,"schema":{"kind":"const","value":"AGREEMENT_INVALID_STATE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error254":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":254}},{"key":1,"required":true,"schema":{"kind":"const","value":"AGREEMENT_CAPACITY_EXCEEDED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error255":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":255}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_INELIGIBLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error256":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":256}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_ORG_UNKNOWN"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error257":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":257}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_ATTESTATION_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error258":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":258}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_ATTESTATION_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error259":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":259}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_SLA_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error260":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":260}},{"key":1,"required":true,"schema":{"kind":"const","value":"PROVIDER_SERVICE_KEY_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error261":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":261}},{"key":1,"required":true,"schema":{"kind":"const","value":"STORAGE_CURSOR_STALE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error300":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":300}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_NAME_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error301":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":301}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_PATH_TOO_LONG"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error302":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":302}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_DEPTH_EXCEEDED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error303":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":303}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_CHILD_LIMIT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error304":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":304}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_METADATA_LIMIT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error305":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":305}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_ORDER_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error306":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":306}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_VERSION_CONFLICT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error307":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":307}},{"key":1,"required":true,"schema":{"kind":"const","value":"DRIVE_REFERENCE_UNPUBLISHABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error320":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":320}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_BUCKET_NAME_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error321":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":321}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_KEY_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error322":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":322}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_METADATA_LIMIT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error323":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":323}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_PRECONDITION_FAILED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error324":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":324}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_NOT_FOUND"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error325":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":325}},{"key":1,"required":true,"schema":{"kind":"const","value":"S3_HISTORY_LIMIT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error400":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":400}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_AUDIENCE_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error401":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":401}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_CHALLENGE_REPLAY"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error402":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":402}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_PROOF_EXPIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error403":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":403}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_EPOCH_INVALID"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error404":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":404}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_DISCLOSURE_DENIED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error405":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":405}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_HUMANITY_UNAVAILABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error406":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":406}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_ENTITLEMENT_UNAVAILABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error407":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":407}},{"key":1,"required":true,"schema":{"kind":"const","value":"SIGNING_CONSENT_REQUIRED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error408":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":408}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_RECOVERY_ENTROPY_FAILED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error409":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":409}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_RECOVERY_INSTALL_FAILED"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error410":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":410}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_OLD_INCARNATION"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error411":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":411}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_RETIRED_SET_FULL"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error412":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":412}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_AUTHORITY_UNAVAILABLE"}},{"key":2,"required":true,"schema":{"kind":"const","value":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Error413":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":413}},{"key":1,"required":true,"schema":{"kind":"const","value":"IDENTITY_EFFECT_CONFLICT"}},{"key":2,"required":true,"schema":{"kind":"const","value":false}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ErrorDetails"}}]},"Empty":{"kind":"map","fields":[]},"Fin":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"AcceptedState":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"uint","min":"0","max":"4"}}]},"ProgressState":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":false,"schema":{"kind":"ref","name":"U64"}}]},"ResultState":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"BucketV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"AccountId32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":4,"required":true,"schema":{"kind":"array","min":2,"max":4,"items":{"kind":"ref","name":"ProviderId"}}}]},"ProviderReceiptV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"CheckpointV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U32"}}]},"SubscriptionAckV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"IdentityReceiptV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"Fin"}}]},"StorageBucketCreateRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":1,"required":true,"schema":{"kind":"array","min":2,"max":4,"items":{"kind":"ref","name":"ProviderId"}}},{"key":2,"required":true,"schema":{"kind":"uint","min":"0","max":"1"}}]},"StorageBucketCreateAccepted":{"kind":"ref","name":"AcceptedState"},"StorageBucketCreateProgress":{"kind":"ref","name":"ProgressState"},"StorageBucketCreateResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageBucketCreateError":{"kind":"ref","name":"ErrorV2"},"StorageBucketGetRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageBucketGetAccepted":{"kind":"ref","name":"AcceptedState"},"StorageBucketGetProgress":{"kind":"ref","name":"ProgressState"},"StorageBucketGetResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketV1"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageBucketGetError":{"kind":"ref","name":"ErrorV2"},"StorageBucketGrantRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":2,"required":true,"schema":{"kind":"uint","min":"0","max":"2"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageBucketGrantAccepted":{"kind":"ref","name":"AcceptedState"},"StorageBucketGrantProgress":{"kind":"ref","name":"ProgressState"},"StorageBucketGrantResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageBucketGrantError":{"kind":"ref","name":"ErrorV2"},"StorageBucketRevokeRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageBucketRevokeAccepted":{"kind":"ref","name":"AcceptedState"},"StorageBucketRevokeProgress":{"kind":"ref","name":"ProgressState"},"StorageBucketRevokeResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageBucketRevokeError":{"kind":"ref","name":"ErrorV2"},"StorageObjectPutRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"uint","min":"0","max":"1"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"OperationId"}}]},"StorageObjectPutAccepted":{"kind":"ref","name":"AcceptedState"},"StorageObjectPutProgress":{"kind":"ref","name":"ProgressState"},"StorageObjectPutResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"ProviderReceiptV1"}},{"key":1,"required":true,"schema":{"kind":"bool"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageObjectPutError":{"kind":"ref","name":"ErrorV2"},"StorageObjectGetRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}}]},"StorageObjectGetAccepted":{"kind":"ref","name":"AcceptedState"},"StorageObjectGetProgress":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Bytes4MiB"}}]},"StorageObjectGetResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}}]},"StorageObjectGetError":{"kind":"ref","name":"ErrorV2"},"StorageObjectRangeRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageObjectRangeAccepted":{"kind":"ref","name":"AcceptedState"},"StorageObjectRangeProgress":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Bytes4MiB"}}]},"StorageObjectRangeResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}}]},"StorageObjectRangeError":{"kind":"ref","name":"ErrorV2"},"StorageObjectDeleteRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageObjectDeleteAccepted":{"kind":"ref","name":"AcceptedState"},"StorageObjectDeleteProgress":{"kind":"ref","name":"ProgressState"},"StorageObjectDeleteResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageObjectDeleteError":{"kind":"ref","name":"ErrorV2"},"StorageObjectStatusRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}}]},"StorageObjectStatusAccepted":{"kind":"ref","name":"AcceptedState"},"StorageObjectStatusProgress":{"kind":"ref","name":"ProgressState"},"StorageObjectStatusResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"uint","min":"0","max":"4"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"ProviderReceiptV1"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"CheckpointV2"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":4,"required":true,"schema":{"kind":"bool"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageObjectStatusError":{"kind":"ref","name":"ErrorV2"},"StorageCheckpointStatusRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageCheckpointStatusAccepted":{"kind":"ref","name":"AcceptedState"},"StorageCheckpointStatusProgress":{"kind":"ref","name":"ProgressState"},"StorageCheckpointStatusResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageCheckpointStatusError":{"kind":"ref","name":"ErrorV2"},"StorageCheckpointSubscribeRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageCheckpointSubscribeAccepted":{"kind":"ref","name":"AcceptedState"},"StorageCheckpointSubscribeProgress":{"kind":"ref","name":"ProgressState"},"StorageCheckpointSubscribeResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"SubscriptionAckV1"}}]},"StorageCheckpointSubscribeError":{"kind":"ref","name":"ErrorV2"},"StorageReplicaStatusRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}}]},"StorageReplicaStatusAccepted":{"kind":"ref","name":"AcceptedState"},"StorageReplicaStatusProgress":{"kind":"ref","name":"ProgressState"},"StorageReplicaStatusResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":1,"required":true,"schema":{"kind":"array","min":2,"max":4,"items":{"kind":"ref","name":"ProviderId"}}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageReplicaStatusError":{"kind":"ref","name":"ErrorV2"},"StorageReplicaSubscribeRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageReplicaSubscribeAccepted":{"kind":"ref","name":"AcceptedState"},"StorageReplicaSubscribeProgress":{"kind":"ref","name":"ProgressState"},"StorageReplicaSubscribeResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"SubscriptionAckV1"}}]},"StorageReplicaSubscribeError":{"kind":"ref","name":"ErrorV2"},"StorageDeletionStatusRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}}]},"StorageDeletionStatusAccepted":{"kind":"ref","name":"AcceptedState"},"StorageDeletionStatusProgress":{"kind":"ref","name":"ProgressState"},"StorageDeletionStatusResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageDeletionStatusError":{"kind":"ref","name":"ErrorV2"},"StorageDeletionSubscribeRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageDeletionSubscribeAccepted":{"kind":"ref","name":"AcceptedState"},"StorageDeletionSubscribeProgress":{"kind":"ref","name":"ProgressState"},"StorageDeletionSubscribeResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"SubscriptionAckV1"}}]},"StorageDeletionSubscribeError":{"kind":"ref","name":"ErrorV2"},"StorageDriveReadRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"text","min":1,"max":4096,"nfc":true}},{"key":2,"required":false,"schema":{"kind":"ref","name":"Cid"}}]},"StorageDriveReadAccepted":{"kind":"ref","name":"AcceptedState"},"StorageDriveReadProgress":{"kind":"ref","name":"ProgressState"},"StorageDriveReadResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageDriveReadError":{"kind":"ref","name":"ErrorV2"},"StorageDriveCommitRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"bytes","min":1,"max":4194304}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"uint","min":"0","max":"2"}}]},"StorageDriveCommitAccepted":{"kind":"ref","name":"AcceptedState"},"StorageDriveCommitProgress":{"kind":"ref","name":"ProgressState"},"StorageDriveCommitResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageDriveCommitError":{"kind":"ref","name":"ErrorV2"},"StorageDriveShareRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":2,"required":true,"schema":{"kind":"uint","min":"0","max":"2"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"StorageDriveShareAccepted":{"kind":"ref","name":"AcceptedState"},"StorageDriveShareProgress":{"kind":"ref","name":"ProgressState"},"StorageDriveShareResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageDriveShareError":{"kind":"ref","name":"ErrorV2"},"StorageS3PutRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText128"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":1,"max":1024}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":3,"required":true,"schema":{"kind":"bytes","min":0,"max":32768}},{"key":4,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":5,"required":false,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":6,"required":true,"schema":{"kind":"ref","name":"OperationId"}}]},"StorageS3PutAccepted":{"kind":"ref","name":"AcceptedState"},"StorageS3PutProgress":{"kind":"ref","name":"ProgressState"},"StorageS3PutResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageS3PutError":{"kind":"ref","name":"ErrorV2"},"StorageS3GetRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText128"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":1,"max":1024}},{"key":2,"required":false,"schema":{"kind":"ref","name":"U64"}}]},"StorageS3GetAccepted":{"kind":"ref","name":"AcceptedState"},"StorageS3GetProgress":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Bytes4MiB"}}]},"StorageS3GetResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageS3GetError":{"kind":"ref","name":"ErrorV2"},"StorageS3ListRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText128"}},{"key":1,"required":false,"schema":{"kind":"bytes","min":0,"max":1024}},{"key":2,"required":false,"schema":{"kind":"bytes","min":1,"max":2048}},{"key":3,"required":true,"schema":{"kind":"uint","min":"1","max":"100"}}]},"StorageS3ListAccepted":{"kind":"ref","name":"AcceptedState"},"StorageS3ListProgress":{"kind":"ref","name":"ProgressState"},"StorageS3ListResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"array","min":0,"max":100,"items":{"kind":"ref","name":"Cid"}}},{"key":1,"required":false,"schema":{"kind":"bytes","min":1,"max":2048}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageS3ListError":{"kind":"ref","name":"ErrorV2"},"StorageS3DeleteRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText128"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":1,"max":1024}},{"key":2,"required":false,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":3,"required":true,"schema":{"kind":"ref","name":"OperationId"}}]},"StorageS3DeleteAccepted":{"kind":"ref","name":"AcceptedState"},"StorageS3DeleteProgress":{"kind":"ref","name":"ProgressState"},"StorageS3DeleteResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageS3DeleteError":{"kind":"ref","name":"ErrorV2"},"StoragePublishRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"U64"}}]},"StoragePublishAccepted":{"kind":"ref","name":"AcceptedState"},"StoragePublishProgress":{"kind":"ref","name":"ProgressState"},"StoragePublishResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StoragePublishError":{"kind":"ref","name":"ErrorV2"},"StorageResolveRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageResolveAccepted":{"kind":"ref","name":"AcceptedState"},"StorageResolveProgress":{"kind":"ref","name":"ProgressState"},"StorageResolveResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"StorageResolveError":{"kind":"ref","name":"ErrorV2"},"StorageKeysExportRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":2,"required":true,"schema":{"kind":"bytes","min":32,"max":256}}]},"StorageKeysExportAccepted":{"kind":"ref","name":"AcceptedState"},"StorageKeysExportProgress":{"kind":"ref","name":"ProgressState"},"StorageKeysExportResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"bytes","min":32,"max":1024}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U16"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}}]},"StorageKeysExportError":{"kind":"ref","name":"ErrorV2"},"StorageKeysImportRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":32,"max":1024}},{"key":2,"required":true,"schema":{"kind":"uint","min":"0","max":"1"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U32"}}]},"StorageKeysImportAccepted":{"kind":"ref","name":"AcceptedState"},"StorageKeysImportProgress":{"kind":"ref","name":"ProgressState"},"StorageKeysImportResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"KeyId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U32"}}]},"StorageKeysImportError":{"kind":"ref","name":"ErrorV2"},"IdentityAccountRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText128"}}]},"IdentityAccountAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityAccountProgress":{"kind":"ref","name":"ProgressState"},"IdentityAccountResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"AccountId32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"IdentityAccountError":{"kind":"ref","name":"ErrorV2"},"IdentityProfileReadRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":1,"required":true,"schema":{"kind":"array","min":1,"max":64,"items":{"kind":"ref","name":"NfcText128"}}},{"key":2,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"IdentityProfileReadAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityProfileReadProgress":{"kind":"ref","name":"ProgressState"},"IdentityProfileReadResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"IdentityReceiptV2"}}]},"IdentityProfileReadError":{"kind":"ref","name":"ErrorV2"},"IdentityProfileDiscloseRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":1,"required":true,"schema":{"kind":"array","min":1,"max":64,"items":{"kind":"ref","name":"NfcText128"}}},{"key":2,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"IdentityProfileDiscloseAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityProfileDiscloseProgress":{"kind":"ref","name":"ProgressState"},"IdentityProfileDiscloseResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"IdentityReceiptV2"}}]},"IdentityProfileDiscloseError":{"kind":"ref","name":"ErrorV2"},"IdentityHumanityStatusRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":1,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"IdentityHumanityStatusAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityHumanityStatusProgress":{"kind":"ref","name":"ProgressState"},"IdentityHumanityStatusResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"U16"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"IdentityHumanityStatusError":{"kind":"ref","name":"ErrorV2"},"IdentityHumanityProveRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":16,"max":64}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"array","min":0,"max":64,"items":{"kind":"ref","name":"NfcText128"}}}]},"IdentityHumanityProveAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityHumanityProveProgress":{"kind":"ref","name":"ProgressState"},"IdentityHumanityProveResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"bytes","min":64,"max":4096}},{"key":1,"required":true,"schema":{"kind":"bytes","min":32,"max":32}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"bool"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"IdentityHumanityProveError":{"kind":"ref","name":"ErrorV2"},"IdentitySubjectDeriveRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":3,"required":false,"schema":{"kind":"ref","name":"U32"}}]},"IdentitySubjectDeriveAccepted":{"kind":"ref","name":"AcceptedState"},"IdentitySubjectDeriveProgress":{"kind":"ref","name":"ProgressState"},"IdentitySubjectDeriveResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":32,"max":32}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":4,"required":true,"schema":{"kind":"bool"}}]},"IdentitySubjectDeriveError":{"kind":"ref","name":"ErrorV2"},"IdentityEntitlementsReadRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":2,"required":false,"schema":{"kind":"ref","name":"Hash32"}}]},"IdentityEntitlementsReadAccepted":{"kind":"ref","name":"AcceptedState"},"IdentityEntitlementsReadProgress":{"kind":"ref","name":"ProgressState"},"IdentityEntitlementsReadResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"bool"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"IdentityEntitlementsReadError":{"kind":"ref","name":"ErrorV2"},"TransactionSignRequest":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"TransactionSignAccepted":{"kind":"ref","name":"AcceptedState"},"TransactionSignProgress":{"kind":"ref","name":"ProgressState"},"TransactionSignResult":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Fin"}}]},"TransactionSignError":{"kind":"ref","name":"ErrorV2"},"OperationRequest":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateRequest"},{"kind":"ref","name":"StorageBucketGetRequest"},{"kind":"ref","name":"StorageBucketGrantRequest"},{"kind":"ref","name":"StorageBucketRevokeRequest"},{"kind":"ref","name":"StorageObjectPutRequest"},{"kind":"ref","name":"StorageObjectGetRequest"},{"kind":"ref","name":"StorageObjectRangeRequest"},{"kind":"ref","name":"StorageObjectDeleteRequest"},{"kind":"ref","name":"StorageObjectStatusRequest"},{"kind":"ref","name":"StorageCheckpointStatusRequest"},{"kind":"ref","name":"StorageCheckpointSubscribeRequest"},{"kind":"ref","name":"StorageReplicaStatusRequest"},{"kind":"ref","name":"StorageReplicaSubscribeRequest"},{"kind":"ref","name":"StorageDeletionStatusRequest"},{"kind":"ref","name":"StorageDeletionSubscribeRequest"},{"kind":"ref","name":"StorageDriveReadRequest"},{"kind":"ref","name":"StorageDriveCommitRequest"},{"kind":"ref","name":"StorageDriveShareRequest"},{"kind":"ref","name":"StorageS3PutRequest"},{"kind":"ref","name":"StorageS3GetRequest"},{"kind":"ref","name":"StorageS3ListRequest"},{"kind":"ref","name":"StorageS3DeleteRequest"},{"kind":"ref","name":"StoragePublishRequest"},{"kind":"ref","name":"StorageResolveRequest"},{"kind":"ref","name":"StorageKeysExportRequest"},{"kind":"ref","name":"StorageKeysImportRequest"},{"kind":"ref","name":"IdentityAccountRequest"},{"kind":"ref","name":"IdentityProfileReadRequest"},{"kind":"ref","name":"IdentityProfileDiscloseRequest"},{"kind":"ref","name":"IdentityHumanityStatusRequest"},{"kind":"ref","name":"IdentityHumanityProveRequest"},{"kind":"ref","name":"IdentitySubjectDeriveRequest"},{"kind":"ref","name":"IdentityEntitlementsReadRequest"},{"kind":"ref","name":"TransactionSignRequest"}]},"AcceptedEventV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"const","value":0}},{"key":4,"required":true,"schema":{"kind":"ref","name":"AllAccepted"}}]},"ProgressEventV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"const","value":1}},{"key":4,"required":true,"schema":{"kind":"ref","name":"AllProgress"}}]},"ResultEventV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"const","value":2}},{"key":4,"required":true,"schema":{"kind":"ref","name":"AllResult"}}]},"ErrorEventV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"const","value":3}},{"key":4,"required":true,"schema":{"kind":"ref","name":"AllError"}}]},"CancelledEventV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"const","value":4}},{"key":4,"required":true,"schema":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":107}}]}}]},"StorageCheckpointSubscribeSubscriptionEvent":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"CheckpointV2"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageCheckpointSubscribeUnsubscribeAck":{"kind":"ref","name":"SubscriptionAckV1"},"StorageReplicaSubscribeSubscriptionEvent":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":4,"required":true,"schema":{"kind":"array","min":2,"max":4,"items":{"kind":"ref","name":"ProviderId"}}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageReplicaSubscribeUnsubscribeAck":{"kind":"ref","name":"SubscriptionAckV1"},"StorageDeletionSubscribeSubscriptionEvent":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Fin"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"StorageDeletionSubscribeUnsubscribeAck":{"kind":"ref","name":"SubscriptionAckV1"},"AllAccepted":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateAccepted"},{"kind":"ref","name":"StorageBucketGetAccepted"},{"kind":"ref","name":"StorageBucketGrantAccepted"},{"kind":"ref","name":"StorageBucketRevokeAccepted"},{"kind":"ref","name":"StorageObjectPutAccepted"},{"kind":"ref","name":"StorageObjectGetAccepted"},{"kind":"ref","name":"StorageObjectRangeAccepted"},{"kind":"ref","name":"StorageObjectDeleteAccepted"},{"kind":"ref","name":"StorageObjectStatusAccepted"},{"kind":"ref","name":"StorageCheckpointStatusAccepted"},{"kind":"ref","name":"StorageCheckpointSubscribeAccepted"},{"kind":"ref","name":"StorageReplicaStatusAccepted"},{"kind":"ref","name":"StorageReplicaSubscribeAccepted"},{"kind":"ref","name":"StorageDeletionStatusAccepted"},{"kind":"ref","name":"StorageDeletionSubscribeAccepted"},{"kind":"ref","name":"StorageDriveReadAccepted"},{"kind":"ref","name":"StorageDriveCommitAccepted"},{"kind":"ref","name":"StorageDriveShareAccepted"},{"kind":"ref","name":"StorageS3PutAccepted"},{"kind":"ref","name":"StorageS3GetAccepted"},{"kind":"ref","name":"StorageS3ListAccepted"},{"kind":"ref","name":"StorageS3DeleteAccepted"},{"kind":"ref","name":"StoragePublishAccepted"},{"kind":"ref","name":"StorageResolveAccepted"},{"kind":"ref","name":"StorageKeysExportAccepted"},{"kind":"ref","name":"StorageKeysImportAccepted"},{"kind":"ref","name":"IdentityAccountAccepted"},{"kind":"ref","name":"IdentityProfileReadAccepted"},{"kind":"ref","name":"IdentityProfileDiscloseAccepted"},{"kind":"ref","name":"IdentityHumanityStatusAccepted"},{"kind":"ref","name":"IdentityHumanityProveAccepted"},{"kind":"ref","name":"IdentitySubjectDeriveAccepted"},{"kind":"ref","name":"IdentityEntitlementsReadAccepted"},{"kind":"ref","name":"TransactionSignAccepted"}]},"AllProgress":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateProgress"},{"kind":"ref","name":"StorageBucketGetProgress"},{"kind":"ref","name":"StorageBucketGrantProgress"},{"kind":"ref","name":"StorageBucketRevokeProgress"},{"kind":"ref","name":"StorageObjectPutProgress"},{"kind":"ref","name":"StorageObjectGetProgress"},{"kind":"ref","name":"StorageObjectRangeProgress"},{"kind":"ref","name":"StorageObjectDeleteProgress"},{"kind":"ref","name":"StorageObjectStatusProgress"},{"kind":"ref","name":"StorageCheckpointStatusProgress"},{"kind":"ref","name":"StorageCheckpointSubscribeProgress"},{"kind":"ref","name":"StorageReplicaStatusProgress"},{"kind":"ref","name":"StorageReplicaSubscribeProgress"},{"kind":"ref","name":"StorageDeletionStatusProgress"},{"kind":"ref","name":"StorageDeletionSubscribeProgress"},{"kind":"ref","name":"StorageDriveReadProgress"},{"kind":"ref","name":"StorageDriveCommitProgress"},{"kind":"ref","name":"StorageDriveShareProgress"},{"kind":"ref","name":"StorageS3PutProgress"},{"kind":"ref","name":"StorageS3GetProgress"},{"kind":"ref","name":"StorageS3ListProgress"},{"kind":"ref","name":"StorageS3DeleteProgress"},{"kind":"ref","name":"StoragePublishProgress"},{"kind":"ref","name":"StorageResolveProgress"},{"kind":"ref","name":"StorageKeysExportProgress"},{"kind":"ref","name":"StorageKeysImportProgress"},{"kind":"ref","name":"IdentityAccountProgress"},{"kind":"ref","name":"IdentityProfileReadProgress"},{"kind":"ref","name":"IdentityProfileDiscloseProgress"},{"kind":"ref","name":"IdentityHumanityStatusProgress"},{"kind":"ref","name":"IdentityHumanityProveProgress"},{"kind":"ref","name":"IdentitySubjectDeriveProgress"},{"kind":"ref","name":"IdentityEntitlementsReadProgress"},{"kind":"ref","name":"TransactionSignProgress"}]},"AllResult":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateResult"},{"kind":"ref","name":"StorageBucketGetResult"},{"kind":"ref","name":"StorageBucketGrantResult"},{"kind":"ref","name":"StorageBucketRevokeResult"},{"kind":"ref","name":"StorageObjectPutResult"},{"kind":"ref","name":"StorageObjectGetResult"},{"kind":"ref","name":"StorageObjectRangeResult"},{"kind":"ref","name":"StorageObjectDeleteResult"},{"kind":"ref","name":"StorageObjectStatusResult"},{"kind":"ref","name":"StorageCheckpointStatusResult"},{"kind":"ref","name":"StorageCheckpointSubscribeResult"},{"kind":"ref","name":"StorageReplicaStatusResult"},{"kind":"ref","name":"StorageReplicaSubscribeResult"},{"kind":"ref","name":"StorageDeletionStatusResult"},{"kind":"ref","name":"StorageDeletionSubscribeResult"},{"kind":"ref","name":"StorageDriveReadResult"},{"kind":"ref","name":"StorageDriveCommitResult"},{"kind":"ref","name":"StorageDriveShareResult"},{"kind":"ref","name":"StorageS3PutResult"},{"kind":"ref","name":"StorageS3GetResult"},{"kind":"ref","name":"StorageS3ListResult"},{"kind":"ref","name":"StorageS3DeleteResult"},{"kind":"ref","name":"StoragePublishResult"},{"kind":"ref","name":"StorageResolveResult"},{"kind":"ref","name":"StorageKeysExportResult"},{"kind":"ref","name":"StorageKeysImportResult"},{"kind":"ref","name":"IdentityAccountResult"},{"kind":"ref","name":"IdentityProfileReadResult"},{"kind":"ref","name":"IdentityProfileDiscloseResult"},{"kind":"ref","name":"IdentityHumanityStatusResult"},{"kind":"ref","name":"IdentityHumanityProveResult"},{"kind":"ref","name":"IdentitySubjectDeriveResult"},{"kind":"ref","name":"IdentityEntitlementsReadResult"},{"kind":"ref","name":"TransactionSignResult"}]},"AllError":{"kind":"union","variants":[{"kind":"ref","name":"StorageBucketCreateError"},{"kind":"ref","name":"StorageBucketGetError"},{"kind":"ref","name":"StorageBucketGrantError"},{"kind":"ref","name":"StorageBucketRevokeError"},{"kind":"ref","name":"StorageObjectPutError"},{"kind":"ref","name":"StorageObjectGetError"},{"kind":"ref","name":"StorageObjectRangeError"},{"kind":"ref","name":"StorageObjectDeleteError"},{"kind":"ref","name":"StorageObjectStatusError"},{"kind":"ref","name":"StorageCheckpointStatusError"},{"kind":"ref","name":"StorageCheckpointSubscribeError"},{"kind":"ref","name":"StorageReplicaStatusError"},{"kind":"ref","name":"StorageReplicaSubscribeError"},{"kind":"ref","name":"StorageDeletionStatusError"},{"kind":"ref","name":"StorageDeletionSubscribeError"},{"kind":"ref","name":"StorageDriveReadError"},{"kind":"ref","name":"StorageDriveCommitError"},{"kind":"ref","name":"StorageDriveShareError"},{"kind":"ref","name":"StorageS3PutError"},{"kind":"ref","name":"StorageS3GetError"},{"kind":"ref","name":"StorageS3ListError"},{"kind":"ref","name":"StorageS3DeleteError"},{"kind":"ref","name":"StoragePublishError"},{"kind":"ref","name":"StorageResolveError"},{"kind":"ref","name":"StorageKeysExportError"},{"kind":"ref","name":"StorageKeysImportError"},{"kind":"ref","name":"IdentityAccountError"},{"kind":"ref","name":"IdentityProfileReadError"},{"kind":"ref","name":"IdentityProfileDiscloseError"},{"kind":"ref","name":"IdentityHumanityStatusError"},{"kind":"ref","name":"IdentityHumanityProveError"},{"kind":"ref","name":"IdentitySubjectDeriveError"},{"kind":"ref","name":"IdentityEntitlementsReadError"},{"kind":"ref","name":"TransactionSignError"}]},"SubjectContextV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":7,"required":true,"schema":{"kind":"bool"}}]},"SubjectProofV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"NfcText256"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":7,"required":true,"schema":{"kind":"bool"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"Subject"}},{"key":9,"required":true,"schema":{"kind":"bytes","min":32,"max":32}},{"key":10,"required":true,"schema":{"kind":"bytes","min":16,"max":64}},{"key":11,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":12,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":13,"required":true,"schema":{"kind":"ref","name":"Nonce"}}]},"ProviderCapabilityV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"GrantId"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"KeyId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"ProductId"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":7,"required":false,"schema":{"kind":"ref","name":"AgreementId"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":9,"required":true,"schema":{"kind":"array","min":1,"max":64,"items":{"kind":"ref","name":"U16"}}},{"key":10,"required":false,"schema":{"kind":"ref","name":"Cid"}},{"key":11,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":12,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":13,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":14,"required":true,"schema":{"kind":"ref","name":"Nonce"}},{"key":15,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"ResumeTokenV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"KeyId"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":9,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":10,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":11,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":12,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":13,"required":true,"schema":{"kind":"ref","name":"Nonce"}},{"key":14,"required":true,"schema":{"kind":"bool"}},{"key":15,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"ResponseAckV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"HostOutboxEntryV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"uint","min":"0","max":"5"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Bytes4MiB"}},{"key":4,"required":true,"schema":{"kind":"bytes","min":1,"max":4096}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":9,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":10,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":11,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":12,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":13,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":14,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":15,"required":true,"schema":{"kind":"ref","name":"U16"}},{"key":16,"required":false,"schema":{"kind":"ref","name":"Hash32"}},{"key":17,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":18,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":19,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":20,"required":true,"schema":{"kind":"ref","name":"U32"}}]},"RecoveryEntryV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"RequestId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"Nonce"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"Bytes4MiB"}},{"key":8,"required":false,"schema":{"kind":"bytes","min":1,"max":4096}},{"key":9,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":10,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":11,"required":true,"schema":{"kind":"bool"}},{"key":12,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"CheckpointSubmissionV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":7,"required":true,"schema":{"kind":"bytes","min":64,"max":64}},{"key":8,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":9,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":10,"required":true,"schema":{"kind":"array","min":0,"max":4,"items":{"kind":"ref","name":"ProviderId"}}},{"key":11,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"CheckpointResultV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"ProviderTransferRequestV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":8,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"ProviderTransferChunkV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":3,"required":true,"schema":{"kind":"bytes","min":0,"max":262144}},{"key":4,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"ProviderTransferReceiptV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U32"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"ProviderId"}},{"key":6,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":7,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"SubjectProofEnvelopeV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"SubjectProofV2"}},{"key":1,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"RecoveryInstallV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"bytes","min":32,"max":32}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":4,"required":true,"schema":{"kind":"const","value":0}},{"key":5,"required":true,"schema":{"kind":"const","value":false}},{"key":6,"required":false,"schema":{"kind":"ref","name":"Hash32"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}}]},"RecoveryReceiptV2":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":2}},{"key":1,"required":true,"schema":{"kind":"ref","name":"OperationId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":4,"required":true,"schema":{"kind":"const","value":0}},{"key":5,"required":true,"schema":{"kind":"const","value":false}},{"key":6,"required":false,"schema":{"kind":"ref","name":"Hash32"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":8,"required":true,"schema":{"kind":"bytes","min":64,"max":64}}]},"DriveFileManifestV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"array","min":0,"max":256,"items":{"kind":"ref","name":"Cid"}}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"text","min":1,"max":128,"nfc":true}},{"key":5,"required":true,"schema":{"kind":"bytes","min":0,"max":512}}]},"DriveManifestV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"array","min":0,"max":1024,"items":{"kind":"ref","name":"Cid"}}},{"key":4,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"DriveChangedEventV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"ref","name":"BucketId"}},{"key":1,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"S3ObjectVersionV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"text","min":3,"max":63,"nfc":true}},{"key":2,"required":true,"schema":{"kind":"bytes","min":1,"max":1024}},{"key":3,"required":true,"schema":{"kind":"ref","name":"Cid"}},{"key":4,"required":true,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":5,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":6,"required":true,"schema":{"kind":"bool"}},{"key":7,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"S3ChangedEventV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"text","min":3,"max":63,"nfc":true}},{"key":1,"required":true,"schema":{"kind":"bytes","min":1,"max":1024}},{"key":2,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":3,"required":true,"schema":{"kind":"uint","min":"0","max":"1"}},{"key":4,"required":true,"schema":{"kind":"text","min":64,"max":64,"nfc":true}},{"key":5,"required":true,"schema":{"kind":"ref","name":"Hash32"}}]},"DurableStateV1":{"kind":"map","fields":[{"key":0,"required":true,"schema":{"kind":"const","value":1}},{"key":1,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":2,"required":true,"schema":{"kind":"ref","name":"Hash32"}},{"key":3,"required":true,"schema":{"kind":"ref","name":"U64"}},{"key":4,"required":true,"schema":{"kind":"ref","name":"U32"}}]}} as const;
export const HOST_V2_CROSS_FIELD_RULES=[{"production":"StorageObjectRangeRequest","id":"range-length-positive","kind":"uint-positive","key":3},{"production":"StorageBucketGrantRequest","id":"grant-expiry-after-issued","kind":"uint-greater","left":4,"right":3},{"production":"StorageDriveShareRequest","id":"share-expiry-after-issued","kind":"uint-greater","left":4,"right":3},{"production":"ProviderCapabilityV1","id":"capability-expiry-after-issued","kind":"uint-greater","left":13,"right":12},{"production":"ProviderCapabilityV1","id":"capability-validity-at-most-128","kind":"uint-delta-max","left":13,"right":12,"max":"128"},{"production":"ResumeTokenV1","id":"resume-expiry-after-issued","kind":"uint-greater","left":12,"right":11},{"production":"ResumeTokenV1","id":"resume-validity-at-most-128","kind":"uint-delta-max","left":12,"right":11,"max":"128"},{"production":"RecoveryInstallV2","id":"recovery-seed-nonzero","kind":"bytes-nonzero","key":2},{"production":"DriveManifestV1","id":"drive-cids-sorted-unique","kind":"bytes-array-sorted-unique","key":3},{"production":"S3ObjectVersionV1","id":"s3-key-no-nul","kind":"bytes-no-nul","key":2}] as const;
export const HOST_V2_OPERATION_BINDINGS={
  "storage.bucket.create": {
    "code": 1000,
    "featureId": "storage.control",
    "grantScope": "storage.bucket.admin",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageBucketCreateFrame",
    "request": "StorageBucketCreateRequest",
    "accepted": "StorageBucketCreateAccepted",
    "progress": "StorageBucketCreateProgress",
    "result": "StorageBucketCreateResult",
    "error": "StorageBucketCreateError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.bucket.get": {
    "code": 1001,
    "featureId": "storage.control",
    "grantScope": "storage.bucket.read",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageBucketGetFrame",
    "request": "StorageBucketGetRequest",
    "accepted": "StorageBucketGetAccepted",
    "progress": "StorageBucketGetProgress",
    "result": "StorageBucketGetResult",
    "error": "StorageBucketGetError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.bucket.grant": {
    "code": 1002,
    "featureId": "storage.control",
    "grantScope": "storage.bucket.admin",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageBucketGrantFrame",
    "request": "StorageBucketGrantRequest",
    "accepted": "StorageBucketGrantAccepted",
    "progress": "StorageBucketGrantProgress",
    "result": "StorageBucketGrantResult",
    "error": "StorageBucketGrantError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.bucket.revoke": {
    "code": 1003,
    "featureId": "storage.control",
    "grantScope": "storage.bucket.admin",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageBucketRevokeFrame",
    "request": "StorageBucketRevokeRequest",
    "accepted": "StorageBucketRevokeAccepted",
    "progress": "StorageBucketRevokeProgress",
    "result": "StorageBucketRevokeResult",
    "error": "StorageBucketRevokeError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.object.put": {
    "code": 1010,
    "featureId": "storage.content",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageObjectPutFrame",
    "request": "StorageObjectPutRequest",
    "accepted": "StorageObjectPutAccepted",
    "progress": "StorageObjectPutProgress",
    "result": "StorageObjectPutResult",
    "error": "StorageObjectPutError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211,
      220,
      221,
      222,
      223,
      224,
      225,
      226,
      227,
      228,
      229,
      230,
      231,
      232,
      233,
      234,
      235,
      236,
      237,
      238,
      239,
      240,
      241
    ]
  },
  "storage.object.get": {
    "code": 1011,
    "featureId": "storage.content",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageObjectGetFrame",
    "request": "StorageObjectGetRequest",
    "accepted": "StorageObjectGetAccepted",
    "progress": "StorageObjectGetProgress",
    "result": "StorageObjectGetResult",
    "error": "StorageObjectGetError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.object.range": {
    "code": 1012,
    "featureId": "storage.content",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageObjectRangeFrame",
    "request": "StorageObjectRangeRequest",
    "accepted": "StorageObjectRangeAccepted",
    "progress": "StorageObjectRangeProgress",
    "result": "StorageObjectRangeResult",
    "error": "StorageObjectRangeError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.object.delete": {
    "code": 1013,
    "featureId": "storage.deletion",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageObjectDeleteFrame",
    "request": "StorageObjectDeleteRequest",
    "accepted": "StorageObjectDeleteAccepted",
    "progress": "StorageObjectDeleteProgress",
    "result": "StorageObjectDeleteResult",
    "error": "StorageObjectDeleteError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.object.status": {
    "code": 1014,
    "featureId": "storage.content",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageObjectStatusFrame",
    "request": "StorageObjectStatusRequest",
    "accepted": "StorageObjectStatusAccepted",
    "progress": "StorageObjectStatusProgress",
    "result": "StorageObjectStatusResult",
    "error": "StorageObjectStatusError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.checkpoint.status": {
    "code": 1020,
    "featureId": "storage.proof",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageCheckpointStatusFrame",
    "request": "StorageCheckpointStatusRequest",
    "accepted": "StorageCheckpointStatusAccepted",
    "progress": "StorageCheckpointStatusProgress",
    "result": "StorageCheckpointStatusResult",
    "error": "StorageCheckpointStatusError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      220,
      221,
      222,
      223,
      224,
      225,
      226,
      227,
      228,
      229,
      230,
      231,
      232,
      233,
      234,
      235,
      236,
      237,
      238,
      239,
      240,
      241
    ]
  },
  "storage.checkpoint.subscribe": {
    "code": 1021,
    "featureId": "storage.proof",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageCheckpointSubscribeFrame",
    "request": "StorageCheckpointSubscribeRequest",
    "accepted": "StorageCheckpointSubscribeAccepted",
    "progress": "StorageCheckpointSubscribeProgress",
    "result": "StorageCheckpointSubscribeResult",
    "error": "StorageCheckpointSubscribeError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      220,
      221,
      222,
      223,
      224,
      225,
      226,
      227,
      228,
      229,
      230,
      231,
      232,
      233,
      234,
      235,
      236,
      237,
      238,
      239,
      240,
      241
    ]
  },
  "storage.replica.status": {
    "code": 1022,
    "featureId": "storage.replica",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageReplicaStatusFrame",
    "request": "StorageReplicaStatusRequest",
    "accepted": "StorageReplicaStatusAccepted",
    "progress": "StorageReplicaStatusProgress",
    "result": "StorageReplicaStatusResult",
    "error": "StorageReplicaStatusError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.replica.subscribe": {
    "code": 1023,
    "featureId": "storage.replica",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageReplicaSubscribeFrame",
    "request": "StorageReplicaSubscribeRequest",
    "accepted": "StorageReplicaSubscribeAccepted",
    "progress": "StorageReplicaSubscribeProgress",
    "result": "StorageReplicaSubscribeResult",
    "error": "StorageReplicaSubscribeError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.deletion.status": {
    "code": 1024,
    "featureId": "storage.deletion",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageDeletionStatusFrame",
    "request": "StorageDeletionStatusRequest",
    "accepted": "StorageDeletionStatusAccepted",
    "progress": "StorageDeletionStatusProgress",
    "result": "StorageDeletionStatusResult",
    "error": "StorageDeletionStatusError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.deletion.subscribe": {
    "code": 1025,
    "featureId": "storage.deletion",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageDeletionSubscribeFrame",
    "request": "StorageDeletionSubscribeRequest",
    "accepted": "StorageDeletionSubscribeAccepted",
    "progress": "StorageDeletionSubscribeProgress",
    "result": "StorageDeletionSubscribeResult",
    "error": "StorageDeletionSubscribeError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.drive.read": {
    "code": 1030,
    "featureId": "storage.drive",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageDriveReadFrame",
    "request": "StorageDriveReadRequest",
    "accepted": "StorageDriveReadAccepted",
    "progress": "StorageDriveReadProgress",
    "result": "StorageDriveReadResult",
    "error": "StorageDriveReadError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.drive.commit": {
    "code": 1031,
    "featureId": "storage.drive",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageDriveCommitFrame",
    "request": "StorageDriveCommitRequest",
    "accepted": "StorageDriveCommitAccepted",
    "progress": "StorageDriveCommitProgress",
    "result": "StorageDriveCommitResult",
    "error": "StorageDriveCommitError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.drive.share": {
    "code": 1032,
    "featureId": "storage.drive",
    "grantScope": "storage.bucket.admin",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageDriveShareFrame",
    "request": "StorageDriveShareRequest",
    "accepted": "StorageDriveShareAccepted",
    "progress": "StorageDriveShareProgress",
    "result": "StorageDriveShareResult",
    "error": "StorageDriveShareError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.s3.put": {
    "code": 1040,
    "featureId": "storage.s3",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageS3PutFrame",
    "request": "StorageS3PutRequest",
    "accepted": "StorageS3PutAccepted",
    "progress": "StorageS3PutProgress",
    "result": "StorageS3PutResult",
    "error": "StorageS3PutError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.s3.get": {
    "code": 1041,
    "featureId": "storage.s3",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageS3GetFrame",
    "request": "StorageS3GetRequest",
    "accepted": "StorageS3GetAccepted",
    "progress": "StorageS3GetProgress",
    "result": "StorageS3GetResult",
    "error": "StorageS3GetError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.s3.list": {
    "code": 1042,
    "featureId": "storage.s3",
    "grantScope": "storage.bucket.reader",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageS3ListFrame",
    "request": "StorageS3ListRequest",
    "accepted": "StorageS3ListAccepted",
    "progress": "StorageS3ListProgress",
    "result": "StorageS3ListResult",
    "error": "StorageS3ListError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.s3.delete": {
    "code": 1043,
    "featureId": "storage.s3",
    "grantScope": "storage.bucket.writer",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageS3DeleteFrame",
    "request": "StorageS3DeleteRequest",
    "accepted": "StorageS3DeleteAccepted",
    "progress": "StorageS3DeleteProgress",
    "result": "StorageS3DeleteResult",
    "error": "StorageS3DeleteError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      300,
      301,
      302,
      303,
      304,
      305,
      306,
      307,
      320,
      321,
      322,
      323,
      324,
      325
    ]
  },
  "storage.publish": {
    "code": 1050,
    "featureId": "storage.publish",
    "grantScope": "storage.publish",
    "consentMode": "grant",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StoragePublishFrame",
    "request": "StoragePublishRequest",
    "accepted": "StoragePublishAccepted",
    "progress": "StoragePublishProgress",
    "result": "StoragePublishResult",
    "error": "StoragePublishError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211,
      250,
      251,
      252,
      253,
      254,
      255,
      256,
      257,
      258,
      259,
      260,
      261
    ]
  },
  "storage.resolve": {
    "code": 1051,
    "featureId": "storage.publish",
    "grantScope": "public",
    "consentMode": "none",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "StorageResolveFrame",
    "request": "StorageResolveRequest",
    "accepted": "StorageResolveAccepted",
    "progress": "StorageResolveProgress",
    "result": "StorageResolveResult",
    "error": "StorageResolveError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      200,
      201,
      202,
      203,
      204,
      205,
      206,
      207,
      208,
      209,
      210,
      211
    ]
  },
  "storage.keys.export": {
    "code": 1060,
    "featureId": "storage.encryption",
    "grantScope": "storage.keys.export",
    "consentMode": "fresh-user-consent",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageKeysExportFrame",
    "request": "StorageKeysExportRequest",
    "accepted": "StorageKeysExportAccepted",
    "progress": "StorageKeysExportProgress",
    "result": "StorageKeysExportResult",
    "error": "StorageKeysExportError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116
    ]
  },
  "storage.keys.import": {
    "code": 1061,
    "featureId": "storage.encryption",
    "grantScope": "storage.keys.import",
    "consentMode": "fresh-user-consent",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "StorageKeysImportFrame",
    "request": "StorageKeysImportRequest",
    "accepted": "StorageKeysImportAccepted",
    "progress": "StorageKeysImportProgress",
    "result": "StorageKeysImportResult",
    "error": "StorageKeysImportError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116
    ]
  },
  "identity.account": {
    "code": 1100,
    "featureId": "identity.account",
    "grantScope": "identity.account",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "IdentityAccountFrame",
    "request": "IdentityAccountRequest",
    "accepted": "IdentityAccountAccepted",
    "progress": "IdentityAccountProgress",
    "result": "IdentityAccountResult",
    "error": "IdentityAccountError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.profile.read": {
    "code": 1101,
    "featureId": "identity.profile",
    "grantScope": "identity.profile.read",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "IdentityProfileReadFrame",
    "request": "IdentityProfileReadRequest",
    "accepted": "IdentityProfileReadAccepted",
    "progress": "IdentityProfileReadProgress",
    "result": "IdentityProfileReadResult",
    "error": "IdentityProfileReadError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.profile.disclose": {
    "code": 1102,
    "featureId": "identity.profile",
    "grantScope": "identity.profile.disclose",
    "consentMode": "fresh-user-consent",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "IdentityProfileDiscloseFrame",
    "request": "IdentityProfileDiscloseRequest",
    "accepted": "IdentityProfileDiscloseAccepted",
    "progress": "IdentityProfileDiscloseProgress",
    "result": "IdentityProfileDiscloseResult",
    "error": "IdentityProfileDiscloseError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.humanity.status": {
    "code": 1103,
    "featureId": "identity.humanity",
    "grantScope": "identity.humanity.status",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "IdentityHumanityStatusFrame",
    "request": "IdentityHumanityStatusRequest",
    "accepted": "IdentityHumanityStatusAccepted",
    "progress": "IdentityHumanityStatusProgress",
    "result": "IdentityHumanityStatusResult",
    "error": "IdentityHumanityStatusError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.humanity.prove": {
    "code": 1104,
    "featureId": "identity.humanity",
    "grantScope": "identity.humanity.prove",
    "consentMode": "fresh-user-consent",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "IdentityHumanityProveFrame",
    "request": "IdentityHumanityProveRequest",
    "accepted": "IdentityHumanityProveAccepted",
    "progress": "IdentityHumanityProveProgress",
    "result": "IdentityHumanityProveResult",
    "error": "IdentityHumanityProveError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.subject.derive": {
    "code": 1105,
    "featureId": "identity.subject",
    "grantScope": "identity.subject.derive",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "IdentitySubjectDeriveFrame",
    "request": "IdentitySubjectDeriveRequest",
    "accepted": "IdentitySubjectDeriveAccepted",
    "progress": "IdentitySubjectDeriveProgress",
    "result": "IdentitySubjectDeriveResult",
    "error": "IdentitySubjectDeriveError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "identity.entitlements.read": {
    "code": 1106,
    "featureId": "identity.entitlements",
    "grantScope": "identity.entitlements.read",
    "consentMode": "grant",
    "stateChanging": false,
    "operationIdRequired": false,
    "frame": "IdentityEntitlementsReadFrame",
    "request": "IdentityEntitlementsReadRequest",
    "accepted": "IdentityEntitlementsReadAccepted",
    "progress": "IdentityEntitlementsReadProgress",
    "result": "IdentityEntitlementsReadResult",
    "error": "IdentityEntitlementsReadError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  },
  "transaction.sign": {
    "code": 1200,
    "featureId": "transaction.sign",
    "grantScope": "transaction.sign",
    "consentMode": "fresh-user-consent",
    "stateChanging": true,
    "operationIdRequired": true,
    "frame": "TransactionSignFrame",
    "request": "TransactionSignRequest",
    "accepted": "TransactionSignAccepted",
    "progress": "TransactionSignProgress",
    "result": "TransactionSignResult",
    "error": "TransactionSignError",
    "allowedErrors": [
      100,
      101,
      102,
      103,
      104,
      105,
      106,
      107,
      108,
      109,
      110,
      111,
      112,
      113,
      114,
      115,
      116,
      400,
      401,
      402,
      403,
      404,
      405,
      406,
      407,
      408,
      409,
      410,
      411,
      412,
      413
    ]
  }
} as const;
export const HOST_V2_ERROR_BINDINGS={
  "100": {
    "name": "WIRE_SCHEMA_INVALID",
    "retryable": false
  },
  "101": {
    "name": "WIRE_NON_CANONICAL",
    "retryable": false
  },
  "102": {
    "name": "WIRE_VERSION_MISMATCH",
    "retryable": false
  },
  "103": {
    "name": "WIRE_GENESIS_MISMATCH",
    "retryable": false
  },
  "104": {
    "name": "WIRE_DESCRIPTOR_MISMATCH",
    "retryable": false
  },
  "105": {
    "name": "WIRE_SEQUENCE_INVALID",
    "retryable": false
  },
  "106": {
    "name": "REQUEST_DEADLINE_EXPIRED",
    "retryable": false
  },
  "107": {
    "name": "REQUEST_CANCELLED",
    "retryable": false
  },
  "108": {
    "name": "REQUEST_NOT_FOUND",
    "retryable": false
  },
  "109": {
    "name": "GRANT_REQUIRED",
    "retryable": false
  },
  "110": {
    "name": "GRANT_SCOPE_DENIED",
    "retryable": false
  },
  "111": {
    "name": "GRANT_EXPIRED",
    "retryable": false
  },
  "112": {
    "name": "GRANT_REVOKED",
    "retryable": false
  },
  "113": {
    "name": "HOST_OUTBOX_UNAVAILABLE",
    "retryable": false
  },
  "114": {
    "name": "HOST_OUTBOX_FULL",
    "retryable": true
  },
  "115": {
    "name": "HOST_OUTBOX_CORRUPT",
    "retryable": false
  },
  "116": {
    "name": "HOST_OUTBOX_EXPIRED",
    "retryable": false
  },
  "200": {
    "name": "STORAGE_CHUNK_OUT_OF_ORDER",
    "retryable": false
  },
  "201": {
    "name": "STORAGE_CHUNK_TOO_LARGE",
    "retryable": false
  },
  "202": {
    "name": "STORAGE_CHUNK_MISSING",
    "retryable": false
  },
  "203": {
    "name": "STORAGE_LENGTH_MISMATCH",
    "retryable": false
  },
  "204": {
    "name": "STORAGE_CID_MISMATCH",
    "retryable": false
  },
  "205": {
    "name": "STORAGE_OBJECT_TOO_LARGE",
    "retryable": false
  },
  "206": {
    "name": "STORAGE_IDEMPOTENCY_CONFLICT",
    "retryable": false
  },
  "207": {
    "name": "STORAGE_RANGE_INVALID",
    "retryable": false
  },
  "208": {
    "name": "STORAGE_INTEGRITY_FAILED",
    "retryable": false
  },
  "209": {
    "name": "STORAGE_NOT_PUBLISHABLE",
    "retryable": true
  },
  "210": {
    "name": "STORAGE_NOT_FOUND",
    "retryable": false
  },
  "211": {
    "name": "ENCRYPTION_NONCE_REUSE",
    "retryable": false
  },
  "220": {
    "name": "STORAGE_CHECKPOINT_WRONG_DOMAIN",
    "retryable": false
  },
  "221": {
    "name": "STORAGE_CHECKPOINT_WRONG_VERSION",
    "retryable": false
  },
  "222": {
    "name": "STORAGE_CHECKPOINT_WRONG_BUCKET",
    "retryable": false
  },
  "223": {
    "name": "STORAGE_CHECKPOINT_WRONG_KEY",
    "retryable": false
  },
  "224": {
    "name": "STORAGE_CHECKPOINT_STALE_NONCE",
    "retryable": true
  },
  "225": {
    "name": "STORAGE_CHECKPOINT_WRONG_WINDOW",
    "retryable": false
  },
  "226": {
    "name": "CAPABILITY_SIGNATURE_INVALID",
    "retryable": false
  },
  "227": {
    "name": "CAPABILITY_AUDIENCE_INVALID",
    "retryable": false
  },
  "228": {
    "name": "CAPABILITY_CONTENT_INVALID",
    "retryable": false
  },
  "229": {
    "name": "CAPABILITY_NONCE_REPLAY",
    "retryable": false
  },
  "230": {
    "name": "CAPABILITY_EXPIRED",
    "retryable": false
  },
  "231": {
    "name": "CAPABILITY_ISSUER_REVOKED",
    "retryable": false
  },
  "232": {
    "name": "RESUME_SIGNATURE_INVALID",
    "retryable": false
  },
  "233": {
    "name": "RESUME_AUDIENCE_INVALID",
    "retryable": false
  },
  "234": {
    "name": "RESUME_REPLAY",
    "retryable": false
  },
  "235": {
    "name": "RESUME_EXPIRED",
    "retryable": false
  },
  "236": {
    "name": "RESUME_REVOKED",
    "retryable": false
  },
  "237": {
    "name": "RESUME_CURSOR_INVALID",
    "retryable": false
  },
  "238": {
    "name": "PROVIDER_RECOVERY_TABLE_FULL",
    "retryable": false
  },
  "239": {
    "name": "STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM",
    "retryable": false
  },
  "240": {
    "name": "STORAGE_CHECKPOINT_SEQUENCE_INVALID",
    "retryable": false
  },
  "241": {
    "name": "STORAGE_CHECKPOINT_EQUIVOCATION",
    "retryable": false
  },
  "250": {
    "name": "BUCKET_NOT_FOUND",
    "retryable": false
  },
  "251": {
    "name": "BUCKET_VERSION_CONFLICT",
    "retryable": false
  },
  "252": {
    "name": "BUCKET_MEMBER_LIMIT",
    "retryable": false
  },
  "253": {
    "name": "AGREEMENT_INVALID_STATE",
    "retryable": false
  },
  "254": {
    "name": "AGREEMENT_CAPACITY_EXCEEDED",
    "retryable": false
  },
  "255": {
    "name": "PROVIDER_INELIGIBLE",
    "retryable": true
  },
  "256": {
    "name": "PROVIDER_ORG_UNKNOWN",
    "retryable": false
  },
  "257": {
    "name": "PROVIDER_ATTESTATION_INVALID",
    "retryable": false
  },
  "258": {
    "name": "PROVIDER_ATTESTATION_EXPIRED",
    "retryable": false
  },
  "259": {
    "name": "PROVIDER_SLA_INVALID",
    "retryable": false
  },
  "260": {
    "name": "PROVIDER_SERVICE_KEY_INVALID",
    "retryable": false
  },
  "261": {
    "name": "STORAGE_CURSOR_STALE",
    "retryable": true
  },
  "300": {
    "name": "DRIVE_NAME_INVALID",
    "retryable": false
  },
  "301": {
    "name": "DRIVE_PATH_TOO_LONG",
    "retryable": false
  },
  "302": {
    "name": "DRIVE_DEPTH_EXCEEDED",
    "retryable": false
  },
  "303": {
    "name": "DRIVE_CHILD_LIMIT",
    "retryable": false
  },
  "304": {
    "name": "DRIVE_METADATA_LIMIT",
    "retryable": false
  },
  "305": {
    "name": "DRIVE_ORDER_INVALID",
    "retryable": false
  },
  "306": {
    "name": "DRIVE_VERSION_CONFLICT",
    "retryable": false
  },
  "307": {
    "name": "DRIVE_REFERENCE_UNPUBLISHABLE",
    "retryable": false
  },
  "320": {
    "name": "S3_BUCKET_NAME_INVALID",
    "retryable": false
  },
  "321": {
    "name": "S3_KEY_INVALID",
    "retryable": false
  },
  "322": {
    "name": "S3_METADATA_LIMIT",
    "retryable": false
  },
  "323": {
    "name": "S3_PRECONDITION_FAILED",
    "retryable": false
  },
  "324": {
    "name": "S3_NOT_FOUND",
    "retryable": false
  },
  "325": {
    "name": "S3_HISTORY_LIMIT",
    "retryable": false
  },
  "400": {
    "name": "IDENTITY_AUDIENCE_INVALID",
    "retryable": false
  },
  "401": {
    "name": "IDENTITY_CHALLENGE_REPLAY",
    "retryable": false
  },
  "402": {
    "name": "IDENTITY_PROOF_EXPIRED",
    "retryable": false
  },
  "403": {
    "name": "IDENTITY_EPOCH_INVALID",
    "retryable": false
  },
  "404": {
    "name": "IDENTITY_DISCLOSURE_DENIED",
    "retryable": false
  },
  "405": {
    "name": "IDENTITY_HUMANITY_UNAVAILABLE",
    "retryable": true
  },
  "406": {
    "name": "IDENTITY_ENTITLEMENT_UNAVAILABLE",
    "retryable": true
  },
  "407": {
    "name": "SIGNING_CONSENT_REQUIRED",
    "retryable": false
  },
  "408": {
    "name": "IDENTITY_RECOVERY_ENTROPY_FAILED",
    "retryable": false
  },
  "409": {
    "name": "IDENTITY_RECOVERY_INSTALL_FAILED",
    "retryable": false
  },
  "410": {
    "name": "IDENTITY_OLD_INCARNATION",
    "retryable": false
  },
  "411": {
    "name": "IDENTITY_RETIRED_SET_FULL",
    "retryable": false
  },
  "412": {
    "name": "IDENTITY_AUTHORITY_UNAVAILABLE",
    "retryable": true
  },
  "413": {
    "name": "IDENTITY_EFFECT_CONFLICT",
    "retryable": false
  }
} as const;
export interface HostV2OperationTypeMap {
  readonly "storage.bucket.create": { readonly code: 1000; readonly frame: StorageBucketCreateFrame; readonly request: StorageBucketCreateRequest; readonly accepted: StorageBucketCreateAccepted; readonly progress: StorageBucketCreateProgress; readonly result: StorageBucketCreateResult; readonly error: StorageBucketCreateError };
  readonly "storage.bucket.get": { readonly code: 1001; readonly frame: StorageBucketGetFrame; readonly request: StorageBucketGetRequest; readonly accepted: StorageBucketGetAccepted; readonly progress: StorageBucketGetProgress; readonly result: StorageBucketGetResult; readonly error: StorageBucketGetError };
  readonly "storage.bucket.grant": { readonly code: 1002; readonly frame: StorageBucketGrantFrame; readonly request: StorageBucketGrantRequest; readonly accepted: StorageBucketGrantAccepted; readonly progress: StorageBucketGrantProgress; readonly result: StorageBucketGrantResult; readonly error: StorageBucketGrantError };
  readonly "storage.bucket.revoke": { readonly code: 1003; readonly frame: StorageBucketRevokeFrame; readonly request: StorageBucketRevokeRequest; readonly accepted: StorageBucketRevokeAccepted; readonly progress: StorageBucketRevokeProgress; readonly result: StorageBucketRevokeResult; readonly error: StorageBucketRevokeError };
  readonly "storage.object.put": { readonly code: 1010; readonly frame: StorageObjectPutFrame; readonly request: StorageObjectPutRequest; readonly accepted: StorageObjectPutAccepted; readonly progress: StorageObjectPutProgress; readonly result: StorageObjectPutResult; readonly error: StorageObjectPutError };
  readonly "storage.object.get": { readonly code: 1011; readonly frame: StorageObjectGetFrame; readonly request: StorageObjectGetRequest; readonly accepted: StorageObjectGetAccepted; readonly progress: StorageObjectGetProgress; readonly result: StorageObjectGetResult; readonly error: StorageObjectGetError };
  readonly "storage.object.range": { readonly code: 1012; readonly frame: StorageObjectRangeFrame; readonly request: StorageObjectRangeRequest; readonly accepted: StorageObjectRangeAccepted; readonly progress: StorageObjectRangeProgress; readonly result: StorageObjectRangeResult; readonly error: StorageObjectRangeError };
  readonly "storage.object.delete": { readonly code: 1013; readonly frame: StorageObjectDeleteFrame; readonly request: StorageObjectDeleteRequest; readonly accepted: StorageObjectDeleteAccepted; readonly progress: StorageObjectDeleteProgress; readonly result: StorageObjectDeleteResult; readonly error: StorageObjectDeleteError };
  readonly "storage.object.status": { readonly code: 1014; readonly frame: StorageObjectStatusFrame; readonly request: StorageObjectStatusRequest; readonly accepted: StorageObjectStatusAccepted; readonly progress: StorageObjectStatusProgress; readonly result: StorageObjectStatusResult; readonly error: StorageObjectStatusError };
  readonly "storage.checkpoint.status": { readonly code: 1020; readonly frame: StorageCheckpointStatusFrame; readonly request: StorageCheckpointStatusRequest; readonly accepted: StorageCheckpointStatusAccepted; readonly progress: StorageCheckpointStatusProgress; readonly result: StorageCheckpointStatusResult; readonly error: StorageCheckpointStatusError };
  readonly "storage.checkpoint.subscribe": { readonly code: 1021; readonly frame: StorageCheckpointSubscribeFrame; readonly request: StorageCheckpointSubscribeRequest; readonly accepted: StorageCheckpointSubscribeAccepted; readonly progress: StorageCheckpointSubscribeProgress; readonly result: StorageCheckpointSubscribeResult; readonly error: StorageCheckpointSubscribeError };
  readonly "storage.replica.status": { readonly code: 1022; readonly frame: StorageReplicaStatusFrame; readonly request: StorageReplicaStatusRequest; readonly accepted: StorageReplicaStatusAccepted; readonly progress: StorageReplicaStatusProgress; readonly result: StorageReplicaStatusResult; readonly error: StorageReplicaStatusError };
  readonly "storage.replica.subscribe": { readonly code: 1023; readonly frame: StorageReplicaSubscribeFrame; readonly request: StorageReplicaSubscribeRequest; readonly accepted: StorageReplicaSubscribeAccepted; readonly progress: StorageReplicaSubscribeProgress; readonly result: StorageReplicaSubscribeResult; readonly error: StorageReplicaSubscribeError };
  readonly "storage.deletion.status": { readonly code: 1024; readonly frame: StorageDeletionStatusFrame; readonly request: StorageDeletionStatusRequest; readonly accepted: StorageDeletionStatusAccepted; readonly progress: StorageDeletionStatusProgress; readonly result: StorageDeletionStatusResult; readonly error: StorageDeletionStatusError };
  readonly "storage.deletion.subscribe": { readonly code: 1025; readonly frame: StorageDeletionSubscribeFrame; readonly request: StorageDeletionSubscribeRequest; readonly accepted: StorageDeletionSubscribeAccepted; readonly progress: StorageDeletionSubscribeProgress; readonly result: StorageDeletionSubscribeResult; readonly error: StorageDeletionSubscribeError };
  readonly "storage.drive.read": { readonly code: 1030; readonly frame: StorageDriveReadFrame; readonly request: StorageDriveReadRequest; readonly accepted: StorageDriveReadAccepted; readonly progress: StorageDriveReadProgress; readonly result: StorageDriveReadResult; readonly error: StorageDriveReadError };
  readonly "storage.drive.commit": { readonly code: 1031; readonly frame: StorageDriveCommitFrame; readonly request: StorageDriveCommitRequest; readonly accepted: StorageDriveCommitAccepted; readonly progress: StorageDriveCommitProgress; readonly result: StorageDriveCommitResult; readonly error: StorageDriveCommitError };
  readonly "storage.drive.share": { readonly code: 1032; readonly frame: StorageDriveShareFrame; readonly request: StorageDriveShareRequest; readonly accepted: StorageDriveShareAccepted; readonly progress: StorageDriveShareProgress; readonly result: StorageDriveShareResult; readonly error: StorageDriveShareError };
  readonly "storage.s3.put": { readonly code: 1040; readonly frame: StorageS3PutFrame; readonly request: StorageS3PutRequest; readonly accepted: StorageS3PutAccepted; readonly progress: StorageS3PutProgress; readonly result: StorageS3PutResult; readonly error: StorageS3PutError };
  readonly "storage.s3.get": { readonly code: 1041; readonly frame: StorageS3GetFrame; readonly request: StorageS3GetRequest; readonly accepted: StorageS3GetAccepted; readonly progress: StorageS3GetProgress; readonly result: StorageS3GetResult; readonly error: StorageS3GetError };
  readonly "storage.s3.list": { readonly code: 1042; readonly frame: StorageS3ListFrame; readonly request: StorageS3ListRequest; readonly accepted: StorageS3ListAccepted; readonly progress: StorageS3ListProgress; readonly result: StorageS3ListResult; readonly error: StorageS3ListError };
  readonly "storage.s3.delete": { readonly code: 1043; readonly frame: StorageS3DeleteFrame; readonly request: StorageS3DeleteRequest; readonly accepted: StorageS3DeleteAccepted; readonly progress: StorageS3DeleteProgress; readonly result: StorageS3DeleteResult; readonly error: StorageS3DeleteError };
  readonly "storage.publish": { readonly code: 1050; readonly frame: StoragePublishFrame; readonly request: StoragePublishRequest; readonly accepted: StoragePublishAccepted; readonly progress: StoragePublishProgress; readonly result: StoragePublishResult; readonly error: StoragePublishError };
  readonly "storage.resolve": { readonly code: 1051; readonly frame: StorageResolveFrame; readonly request: StorageResolveRequest; readonly accepted: StorageResolveAccepted; readonly progress: StorageResolveProgress; readonly result: StorageResolveResult; readonly error: StorageResolveError };
  readonly "storage.keys.export": { readonly code: 1060; readonly frame: StorageKeysExportFrame; readonly request: StorageKeysExportRequest; readonly accepted: StorageKeysExportAccepted; readonly progress: StorageKeysExportProgress; readonly result: StorageKeysExportResult; readonly error: StorageKeysExportError };
  readonly "storage.keys.import": { readonly code: 1061; readonly frame: StorageKeysImportFrame; readonly request: StorageKeysImportRequest; readonly accepted: StorageKeysImportAccepted; readonly progress: StorageKeysImportProgress; readonly result: StorageKeysImportResult; readonly error: StorageKeysImportError };
  readonly "identity.account": { readonly code: 1100; readonly frame: IdentityAccountFrame; readonly request: IdentityAccountRequest; readonly accepted: IdentityAccountAccepted; readonly progress: IdentityAccountProgress; readonly result: IdentityAccountResult; readonly error: IdentityAccountError };
  readonly "identity.profile.read": { readonly code: 1101; readonly frame: IdentityProfileReadFrame; readonly request: IdentityProfileReadRequest; readonly accepted: IdentityProfileReadAccepted; readonly progress: IdentityProfileReadProgress; readonly result: IdentityProfileReadResult; readonly error: IdentityProfileReadError };
  readonly "identity.profile.disclose": { readonly code: 1102; readonly frame: IdentityProfileDiscloseFrame; readonly request: IdentityProfileDiscloseRequest; readonly accepted: IdentityProfileDiscloseAccepted; readonly progress: IdentityProfileDiscloseProgress; readonly result: IdentityProfileDiscloseResult; readonly error: IdentityProfileDiscloseError };
  readonly "identity.humanity.status": { readonly code: 1103; readonly frame: IdentityHumanityStatusFrame; readonly request: IdentityHumanityStatusRequest; readonly accepted: IdentityHumanityStatusAccepted; readonly progress: IdentityHumanityStatusProgress; readonly result: IdentityHumanityStatusResult; readonly error: IdentityHumanityStatusError };
  readonly "identity.humanity.prove": { readonly code: 1104; readonly frame: IdentityHumanityProveFrame; readonly request: IdentityHumanityProveRequest; readonly accepted: IdentityHumanityProveAccepted; readonly progress: IdentityHumanityProveProgress; readonly result: IdentityHumanityProveResult; readonly error: IdentityHumanityProveError };
  readonly "identity.subject.derive": { readonly code: 1105; readonly frame: IdentitySubjectDeriveFrame; readonly request: IdentitySubjectDeriveRequest; readonly accepted: IdentitySubjectDeriveAccepted; readonly progress: IdentitySubjectDeriveProgress; readonly result: IdentitySubjectDeriveResult; readonly error: IdentitySubjectDeriveError };
  readonly "identity.entitlements.read": { readonly code: 1106; readonly frame: IdentityEntitlementsReadFrame; readonly request: IdentityEntitlementsReadRequest; readonly accepted: IdentityEntitlementsReadAccepted; readonly progress: IdentityEntitlementsReadProgress; readonly result: IdentityEntitlementsReadResult; readonly error: IdentityEntitlementsReadError };
  readonly "transaction.sign": { readonly code: 1200; readonly frame: TransactionSignFrame; readonly request: TransactionSignRequest; readonly accepted: TransactionSignAccepted; readonly progress: TransactionSignProgress; readonly result: TransactionSignResult; readonly error: TransactionSignError };
}
