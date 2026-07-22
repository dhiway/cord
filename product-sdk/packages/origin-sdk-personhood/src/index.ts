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

import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import type { AccountId, Hash32, Versioned } from "@cord-network/origin-sdk-identity";
import { accountId, hash32 } from "@cord-network/origin-sdk-identity";
import { err, ok } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

export interface PersonhoodStatusView {
  readonly full_personal_id: string | null;
  readonly full_recognized: boolean;
  readonly lite_recognized: boolean;
}

export interface AttestationAllowanceView {
  readonly remaining: number;
}

export type CandidateSignature =
  | { readonly scheme: "sr25519" | "ed25519"; readonly bytes: string }
  | { readonly scheme: "ecdsa"; readonly bytes: string };

declare const personhoodType: unique symbol;
export type RingVrfKey = Hash32 & { readonly [personhoodType]: "RingVrfKey" };
export type RingVrfProof = string & { readonly [personhoodType]: "RingVrfProof" };

export interface LitePersonAttestation {
  readonly candidate: AccountId;
  readonly candidate_signature: CandidateSignature;
  readonly ring_vrf_key: RingVrfKey;
  readonly proof_of_ownership: RingVrfProof;
}

export interface PersonhoodProfile {
  readonly finalized_hash: `0x${string}`;
  readonly finalized_number: bigint;
  readonly status: Versioned<PersonhoodStatusView>;
  readonly attestation_allowance: Versioned<AttestationAllowanceView>;
}

export interface PersonhoodReadAdapter {
  personhoodStatus(at: `0x${string}`, account: AccountId, signal?: AbortSignal): Promise<Versioned<PersonhoodStatusView>>;
  attestationAllowance(at: `0x${string}`, account: AccountId, signal?: AbortSignal): Promise<Versioned<AttestationAllowanceView>>;
}

export interface PersonhoodRuntimeAdapter extends PersonhoodReadAdapter {
  attestLitePerson(at: `0x${string}`, input: LitePersonAttestation, signal?: AbortSignal): Promise<PreparedTransaction>;
}

export interface PersonhoodClient {
  status(account: AccountId, signal?: AbortSignal): Promise<SdkResult<Versioned<PersonhoodStatusView>>>;
  attestationAllowance(account: AccountId, signal?: AbortSignal): Promise<SdkResult<Versioned<AttestationAllowanceView>>>;
  profile(account: AccountId, signal?: AbortSignal): Promise<SdkResult<PersonhoodProfile>>;
  prepareAttestLitePerson(input: LitePersonAttestation, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
}

export const PERSONHOOD_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  reads: {
    status: "IdentityPersonhoodApi.personhood_status",
    attestationAllowance: "IdentityPersonhoodApi.attestation_allowance",
  },
  transactions: { attestLitePerson: "PeopleLite.attest" },
} as const;

export const PERSONHOOD_ADMIN_EXCLUSIONS = [
  { target: "Personhood.force_recognize_personhood", reason: "forced runtime administration" },
  { target: "Personhood.create_people_collection", reason: "collection administration" },
  { target: "Personhood.clean_up_stale_aliases", reason: "maintenance" },
  { target: "PeopleLite.increase_attestation_allowance", reason: "allowance administration" },
  { target: "PeopleLite.clear_attestation_allowance", reason: "allowance administration" },
  { target: "PeopleLite.dispatch_as_signer", reason: "unbounded dispatch is not an application API" },
] as const;

const invalid = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "personhood",
  domain: "input",
  code: "invalid_input",
  message,
  retryable: false,
}));

function validateAttestation(input: LitePersonAttestation): void {
  accountId(input.candidate);
  hash32(input.ring_vrf_key);
  const bytes = input.candidate_signature.scheme === "ecdsa" ? 130 : 128;
  if (!new RegExp(`^0x[0-9a-f]{${bytes}}$`).test(input.candidate_signature.bytes))
    throw new TypeError(`${input.candidate_signature.scheme} signature has an invalid length`);
  if (!/^0x[0-9a-f]{128}$/.test(input.proof_of_ownership))
    throw new TypeError("proof_of_ownership must be a lowercase 64-byte 0x-prefixed value");
}

export function createPersonhoodClient(chain: CommonsChainClient, runtime: PersonhoodRuntimeAdapter): PersonhoodClient {
  return {
    async status(account, signal) {
      try { accountId(account); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid account"); }
      return chain.readFinalized((at) => runtime.personhoodStatus(at, account, signal), signal);
    },
    async attestationAllowance(account, signal) {
      try { accountId(account); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid account"); }
      return chain.readFinalized((at) => runtime.attestationAllowance(at, account, signal), signal);
    },
    async profile(account, signal) {
      try { accountId(account); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid account"); }
      const snapshot = await chain.finalizedSnapshot(signal);
      if (!snapshot.success) return snapshot;
      const [status, allowance] = await Promise.all([
        snapshot.value.read((at) => runtime.personhoodStatus(at, account, signal), signal),
        snapshot.value.read((at) => runtime.attestationAllowance(at, account, signal), signal),
      ]);
      if (!status.success) return status;
      if (!allowance.success) return allowance;
      return ok({
        finalized_hash: snapshot.value.block.hash,
        finalized_number: snapshot.value.block.number,
        status: status.value,
        attestation_allowance: allowance.value,
      });
    },
    async prepareAttestLitePerson(input, signal) {
      try { validateAttestation(input); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid attestation"); }
      const normalized = {
        ...input,
        candidate_signature: { ...input.candidate_signature },
      } as LitePersonAttestation;
      return prepareAtFinalized(chain, ({ block }) => runtime.attestLitePerson(block.hash, normalized, signal), signal);
    },
  };
}
