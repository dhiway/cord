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

import {
  page,
  type AccountId,
  type AttestationId,
  type BlockNumber,
  type ContentCommitment,
  type NameId,
  type NamesAddress,
  type NormalizedLabel,
  type PageInput,
  type RegistrationCommitment,
  type RegistrationSalt,
  type SubjectId,
  type TextKey,
  type TextValue,
} from "@cord-network/origin-sdk-names";
import { finalizedRead, submitAndFinalize, type RequestContext } from "../../../src/host.ts";

export const namesHostRoutes = {
	labelPolicyVersion(context: RequestContext) {
		return finalizedRead("names", context, "names", "label_policy_version", {});
	},

  nameById(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "name_by_id", { name });
  },

  rootNameByNormalizedLabel(context: RequestContext, label: NormalizedLabel) {
    return finalizedRead("names", context, "names", "root_name_by_normalized_label", { label });
  },

  ownerNames(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("names", context, "names", "owner_names", { owner, ...page(input) });
  },

  controllers(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "controllers", { name });
  },

  resolveAddress(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "resolve_address", { name });
  },

  resolveSubject(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "resolve_subject", { name });
  },

  resolveAttestation(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "resolve_attestation", { name });
  },

  resolveContentPublication(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "resolve_content_publication", { name });
  },

  resolveText(context: RequestContext, name: NameId, key: TextKey) {
    return finalizedRead("names", context, "names", "resolve_text", { name, key });
  },

  primaryName(context: RequestContext, owner: AccountId) {
    return finalizedRead("names", context, "names", "primary_name", { owner });
  },

  nameStatus(context: RequestContext, name: NameId) {
    return finalizedRead("names", context, "names", "name_status", { name });
  },

  commit(context: RequestContext, commitment: RegistrationCommitment) {
    return submitAndFinalize("names", context, "names", "commit", { commitment });
  },

  cancelCommitment(context: RequestContext, commitment: RegistrationCommitment) {
    return submitAndFinalize("names", context, "names", "cancel_commitment", { commitment });
  },

  pruneExpiredCommitment(
    context: RequestContext,
    owner: AccountId,
    commitment: RegistrationCommitment,
  ) {
    return submitAndFinalize("names", context, "names", "prune_expired_commitment", {
      owner,
      commitment,
    });
  },

  register(
    context: RequestContext,
    parent: NameId | null,
    label: NormalizedLabel,
    salt: RegistrationSalt,
  ) {
    return submitAndFinalize("names", context, "names", "register", { parent, label, salt });
  },

  renew(context: RequestContext, name: NameId, additional_period: BlockNumber) {
    return submitAndFinalize("names", context, "names", "renew", { name, additional_period });
  },

  transfer(context: RequestContext, name: NameId, new_owner: AccountId) {
    return submitAndFinalize("names", context, "names", "transfer", { name, new_owner });
  },

  addController(context: RequestContext, name: NameId, controller: AccountId) {
    return submitAndFinalize("names", context, "names", "add_controller", { name, controller });
  },

  removeController(context: RequestContext, name: NameId, controller: AccountId) {
    return submitAndFinalize("names", context, "names", "remove_controller", { name, controller });
  },

  setAddress(context: RequestContext, name: NameId, address: NamesAddress | null) {
    return submitAndFinalize("names", context, "names", "set_address", { name, address });
  },

  setSubject(context: RequestContext, name: NameId, subject: SubjectId | null) {
    return submitAndFinalize("names", context, "names", "set_subject", { name, subject });
  },

  setAttestation(context: RequestContext, name: NameId, attestation: AttestationId | null) {
    return submitAndFinalize("names", context, "names", "set_attestation", { name, attestation });
  },

  publishContent(context: RequestContext, name: NameId, content: ContentCommitment | null, expectedRevision: string, operationId: string) {
    return submitAndFinalize("names", context, "names", "publish_content", { name, content, expected_revision: expectedRevision, operation_id: operationId });
  },

  setText(context: RequestContext, name: NameId, key: TextKey, value: TextValue | null) {
    return submitAndFinalize("names", context, "names", "set_text", { name, key, value });
  },

  setPrimaryName(context: RequestContext, name: NameId | null) {
    return submitAndFinalize("names", context, "names", "set_primary_name", { name });
  },

  release(context: RequestContext, name: NameId) {
    return submitAndFinalize("names", context, "names", "release", { name });
  },

  removeExpiredName(context: RequestContext, name: NameId) {
    return submitAndFinalize("names", context, "names", "remove_expired_name", { name });
  },

  reserveName(
    context: RequestContext,
    parent: NameId | null,
    label: NormalizedLabel,
    beneficiary: AccountId | null,
    expires_at: BlockNumber | null,
  ) {
    return submitAndFinalize("names", context, "names", "reserve_name", {
      parent,
      label,
      beneficiary,
      expires_at,
    });
  },

  clearReservation(context: RequestContext, name: NameId) {
    return submitAndFinalize("names", context, "names", "clear_reservation", { name });
  },

  setLabelProtection(context: RequestContext, label: NormalizedLabel, protected_label: boolean) {
    return submitAndFinalize("names", context, "names", "set_label_protection", {
      label,
      protected: protected_label,
    });
  },

  setPaused(context: RequestContext, paused: boolean) {
    return submitAndFinalize("names", context, "names", "set_paused", { paused });
  },

  forceTransfer(context: RequestContext, name: NameId, new_owner: AccountId) {
    return submitAndFinalize("names", context, "names", "force_transfer", { name, new_owner });
  },

  forceRevoke(context: RequestContext, name: NameId) {
    return submitAndFinalize("names", context, "names", "force_revoke", { name });
  },

  setRegistrar(context: RequestContext, registrar: AccountId, enabled: boolean) {
    return submitAndFinalize("names", context, "names", "set_registrar", { registrar, enabled });
  },
} as const;
