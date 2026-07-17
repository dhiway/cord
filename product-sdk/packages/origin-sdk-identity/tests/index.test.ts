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
  IDENTITY_V2_OPERATION_CODES,
  accountId,
  hash32,
} from "../src/index.ts";

test("root exports only the unified seven-operation Identity projection plus signing", () => {
  const operations = Object.keys(IDENTITY_V2_OPERATION_CODES);
  assert.deepEqual(operations.filter((operation) => operation.startsWith("identity.")), [
    "identity.account",
    "identity.profile.read",
    "identity.profile.disclose",
    "identity.humanity.status",
    "identity.humanity.prove",
    "identity.subject.derive",
    "identity.entitlements.read",
  ]);
  assert.equal(operations.filter((operation) => operation === "transaction.sign").length, 1);
});

test("shared account and hash primitives remain closed and reusable", () => {
  assert.equal(accountId("5Identity"), "5Identity");
  assert.throws(() => accountId(""), TypeError);
  assert.equal(hash32(`0x${"11".repeat(32)}`), `0x${"11".repeat(32)}`);
  assert.throws(() => hash32(`0x${"AA".repeat(32)}`), TypeError);
});
