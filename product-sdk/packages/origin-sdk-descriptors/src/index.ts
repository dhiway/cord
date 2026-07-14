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

import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { COMMONS_CANDIDATE_NETWORK_BINDING, COMMONS_NETWORK_BINDING } from "./network-binding.ts";

export { COMMONS_CANDIDATE_NETWORK_BINDING, COMMONS_NETWORK_BINDING } from "./network-binding.ts";

export type CommonsNetworkBinding = typeof COMMONS_NETWORK_BINDING;
export type CommonsCandidateNetworkBinding = typeof COMMONS_CANDIDATE_NETWORK_BINDING;

export interface RuntimeIdentity {
  genesis_hash: string;
  spec_version: number;
  transaction_version: number;
  metadata_hash: string;
  descriptor_contract_sha256: string;
  chain_spec_source_sha256: string;
}

export function validateCommonsRuntime(identity: RuntimeIdentity): SdkResult<CommonsNetworkBinding> {
  const expected = COMMONS_NETWORK_BINDING;
  const mismatch = (Object.keys(identity) as (keyof RuntimeIdentity)[])
    .find((key) => identity[key] !== expected[key]);
  if (mismatch !== undefined) {
    return {
      success: false,
      error: new OriginSdkError({
        source: "descriptors",
        domain: "runtime",
        code: "runtime_identity_mismatch",
        message: `Commons runtime ${mismatch} does not match the generated descriptor`,
        retryable: false,
        details: { field: mismatch, expected: expected[mismatch], actual: identity[mismatch] },
      }),
    };
  }
  return { success: true, value: expected };
}
