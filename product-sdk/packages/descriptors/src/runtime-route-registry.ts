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

import { attestationHostRoutes } from "./attestation-host-routes.ts";
import { namesHostRoutes } from "./names-host-routes.ts";
import { driveHostRoutes, providerHostRoutes, s3HostRoutes } from "./storage-host-routes.ts";




import { identityHostRoutes, personhoodHostRoutes } from "./identity-host-routes.ts";
import { sponsoredTransaction } from "../../../src/sponsored-transaction.ts";

/** Direct references to every real exported SDK route callable. */
export const NATIVE_RUNTIME_ROUTE_REGISTRY = {
  "identity:identity_status": identityHostRoutes.identityStatus,
  "identity:personhood_status": personhoodHostRoutes.personhoodStatus,
  "identity:attestation_allowance": personhoodHostRoutes.attestationAllowance,
  "identity:set_identity": identityHostRoutes.setIdentity,
  "identity:clear_identity": identityHostRoutes.clearIdentity,
  "identity:request_judgement": identityHostRoutes.requestJudgement,
  "identity:cancel_judgement_request": identityHostRoutes.cancelJudgementRequest,
  "identity:provide_judgement": identityHostRoutes.provideJudgement,
  "identity:attest_lite_person": personhoodHostRoutes.attestLitePerson,
  "attestation:schema_by_id": attestationHostRoutes.schemaById,
  "attestation:attestation_by_id": attestationHostRoutes.attestationById,
  "attestation:attestation_live_status": attestationHostRoutes.liveStatus,
  "attestation:creator_schemas": attestationHostRoutes.creatorSchemas,
  "attestation:issuer_attestations": attestationHostRoutes.issuerAttestations,
  "attestation:subject_schema_attestations": attestationHostRoutes.subjectSchemaAttestations,
  "attestation:next_delegated_nonce": attestationHostRoutes.nextDelegatedNonce,
  "attestation:schema_count": attestationHostRoutes.schemaCount,
  "attestation:attestation_count": attestationHostRoutes.attestationCount,
  "attestation:next_issuance_nonce": attestationHostRoutes.nextIssuanceNonce,
  "attestation:external_status": attestationHostRoutes.externalStatus,
  "attestation:create_schema": attestationHostRoutes.createSchema,
  "attestation:set_schema_status": attestationHostRoutes.setSchemaStatus,
  "attestation:issue": attestationHostRoutes.issue,
  "attestation:issue_delegated": attestationHostRoutes.issueDelegated,
  "attestation:issue_batch": attestationHostRoutes.issueBatch,
  "attestation:revoke": attestationHostRoutes.revoke,
  "attestation:set_emergency_pause": attestationHostRoutes.setEmergencyPause,
  "attestation:force_schema_status": attestationHostRoutes.forceSchemaStatus,
  "attestation:force_revoke": attestationHostRoutes.forceRevoke,
  "attestation:revoke_delegated": attestationHostRoutes.revokeDelegated,
  "attestation:issue_delegated_batch": attestationHostRoutes.issueDelegatedBatch,
  "attestation:revoke_batch": attestationHostRoutes.revokeBatch,
  "attestation:revoke_delegated_batch": attestationHostRoutes.revokeDelegatedBatch,
  "attestation:revoke_external_status": attestationHostRoutes.revokeExternalStatus,
  "attestation:revoke_external_status_batch": attestationHostRoutes.revokeExternalStatusBatch,
  "names:label_policy_version": namesHostRoutes.labelPolicyVersion,
  "names:name_by_id": namesHostRoutes.nameById,
  "names:root_name_by_normalized_label": namesHostRoutes.rootNameByNormalizedLabel,
  "names:owner_names": namesHostRoutes.ownerNames,
  "names:controllers": namesHostRoutes.controllers,
  "names:resolve_address": namesHostRoutes.resolveAddress,
  "names:resolve_subject": namesHostRoutes.resolveSubject,
  "names:resolve_attestation": namesHostRoutes.resolveAttestation,
  "names:resolve_content_publication": namesHostRoutes.resolveContentPublication,
  "names:resolve_text": namesHostRoutes.resolveText,
  "names:primary_name": namesHostRoutes.primaryName,
  "names:name_status": namesHostRoutes.nameStatus,
  "names:commit": namesHostRoutes.commit,
  "names:cancel_commitment": namesHostRoutes.cancelCommitment,
  "names:prune_expired_commitment": namesHostRoutes.pruneExpiredCommitment,
  "names:register": namesHostRoutes.register,
  "names:renew": namesHostRoutes.renew,
  "names:transfer": namesHostRoutes.transfer,
  "names:add_controller": namesHostRoutes.addController,
  "names:remove_controller": namesHostRoutes.removeController,
  "names:set_address": namesHostRoutes.setAddress,
  "names:set_subject": namesHostRoutes.setSubject,
  "names:set_attestation": namesHostRoutes.setAttestation,
  "names:publish_content": namesHostRoutes.publishContent,
  "names:set_text": namesHostRoutes.setText,
  "names:set_primary_name": namesHostRoutes.setPrimaryName,
  "names:release": namesHostRoutes.release,
  "names:remove_expired_name": namesHostRoutes.removeExpiredName,
  "names:reserve_name": namesHostRoutes.reserveName,
  "names:clear_reservation": namesHostRoutes.clearReservation,
  "names:set_label_protection": namesHostRoutes.setLabelProtection,
  "names:set_paused": namesHostRoutes.setPaused,
  "names:force_transfer": namesHostRoutes.forceTransfer,
  "names:force_revoke": namesHostRoutes.forceRevoke,
  "names:set_registrar": namesHostRoutes.setRegistrar,
  "storage:provider_by_id": providerHostRoutes.providerById,
  "storage:providers": providerHostRoutes.providers,
  "storage:agreement_by_id": providerHostRoutes.agreementById,
  "storage:provider_agreements": providerHostRoutes.providerAgreements,
  "storage:agreement_nonce": providerHostRoutes.agreementNonce,
  "storage:challenge_by_id": providerHostRoutes.challengeById,
  "storage:challenges_at": providerHostRoutes.challengesAt,
  "storage:can_accept_capacity": providerHostRoutes.canAcceptCapacity,
  "storage:bucket_checkpoint": providerHostRoutes.bucketCheckpoint,
  "storage:register_provider": providerHostRoutes.registerProvider,
  "storage:update_provider": providerHostRoutes.updateProvider,
  "storage:set_provider_status": providerHostRoutes.setProviderStatus,
  "storage:remove_provider": providerHostRoutes.removeProvider,
  "storage:heartbeat": providerHostRoutes.heartbeat,
  "storage:propose_agreement": providerHostRoutes.proposeAgreement,
  "storage:accept_agreement": providerHostRoutes.acceptAgreement,
  "storage:cancel_agreement": providerHostRoutes.cancelAgreement,
  "storage:issue_challenge": providerHostRoutes.issueChallenge,
  "storage:timeout_challenge": providerHostRoutes.timeoutChallenge,
  "storage:request_renewal": providerHostRoutes.requestRenewal,
  "storage:acknowledge_manifest_deletion": providerHostRoutes.acknowledgeManifestDeletion,
  "storage:accept_renewal": providerHostRoutes.acceptRenewal,
  "storage:expire_agreement": providerHostRoutes.expireAgreement,
  "storage:prune_agreement": providerHostRoutes.pruneAgreement,
  "storage:drive_by_id": driveHostRoutes.driveById,
  "storage:owner_drives": driveHostRoutes.ownerDrives,
  "storage:drive_controllers": driveHostRoutes.driveControllers,
  "storage:next_drive_nonce": driveHostRoutes.nextDriveNonce,
  "storage:create_drive": driveHostRoutes.createDrive,
  "storage:update_root": driveHostRoutes.updateRoot,
  "storage:drive.set_controller": driveHostRoutes.setController,
  "storage:transfer_drive": driveHostRoutes.transferDrive,
  "storage:archive_drive": driveHostRoutes.archiveDrive,
  "storage:bucket_by_id": s3HostRoutes.bucketById,
  "storage:bucket_by_name": s3HostRoutes.bucketByName,
  "storage:owner_buckets": s3HostRoutes.ownerBuckets,
  "storage:bucket_object_keys": s3HostRoutes.bucketObjectKeys,
  "storage:object_by_key": s3HostRoutes.objectByKey,
  "storage:object_history": s3HostRoutes.objectHistory,
  "storage:object_id": s3HostRoutes.objectId,
  "storage:create_bucket": s3HostRoutes.createBucket,
  "storage:s3.set_controller": s3HostRoutes.setController,
  "storage:transfer_bucket": s3HostRoutes.transferBucket,
  "storage:set_archived": s3HostRoutes.setArchived,
  "storage:set_versioning": s3HostRoutes.setVersioning,
  "storage:put_object": s3HostRoutes.putObject,
  "storage:delete_object": s3HostRoutes.deleteObject,
  "storage:delete_bucket": s3HostRoutes.deleteBucket,
  "transaction:prepare_sponsored_intent": sponsoredTransaction.prepareSponsoredIntent,
  "transaction:submit_sponsored_intent": sponsoredTransaction.submitSponsoredIntent,
} as const;
