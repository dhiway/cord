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
import { createFakeSigner } from "../src/testing.ts";

test("fake signer records, rejects, reconfigures, and resets", async () => {
  const signer = createFakeSigner({
    accounts: [{ address: "5Alice", name: "Alice" }],
    signature: new Uint8Array([1, 2, 3]),
  });
  const request = { account: "5Alice", payload: new Uint8Array([9]), purpose: "check-in" };
  const signed = await signer.sign(request);
  assert.equal(signed.success, true);
  assert.deepEqual(signed.success && signed.value, new Uint8Array([1, 2, 3]));
  assert.deepEqual(signer.requests, [request]);

  signer.rejectNextSignature();
  const rejected = await signer.sign(request);
  assert.equal(!rejected.success && rejected.error.code, "signing_rejected");

  signer.setAccounts([]);
  const missing = await signer.sign(request);
  assert.equal(!missing.success && missing.error.code, "account_unavailable");
  signer.reset();
  assert.equal((await signer.accounts()).success, true);
  assert.equal(signer.requests.length, 0);
});
