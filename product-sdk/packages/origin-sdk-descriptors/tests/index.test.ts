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
import {
  COMMONS_CANDIDATE_NETWORK_BINDING,
  COMMONS_NETWORK_BINDING,
  validateCommonsRuntime,
} from "../src/index.ts";

const identity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

test("Commons descriptor accepts only its exact runtime identity", () => {
  assert.deepEqual(validateCommonsRuntime(identity), { success: true, value: COMMONS_NETWORK_BINDING });
  const mismatch = validateCommonsRuntime({ ...identity, spec_version: identity.spec_version + 1 });
  assert.equal(mismatch.success, false);
  if (!mismatch.success) assert.equal(mismatch.error.code, "runtime_identity_mismatch");
  assert.equal(COMMONS_CANDIDATE_NETWORK_BINDING.access_mode, "candidate");
  assert.equal(COMMONS_NETWORK_BINDING.production_activation_ready, false);
});
