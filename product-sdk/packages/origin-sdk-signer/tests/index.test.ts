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
import { createHostClient } from "@cord-network/origin-sdk-host";
import { createFakeHost } from "@cord-network/origin-sdk-host/testing";
import {
  createHostSigner,
  selectHostSigner,
} from "../src/index.ts";

const product = { id: "signer.app", name: "Signer" } as const;

test("host signer requires a live grant and per-call host approval", async () => {
  const fake = createFakeHost({ accounts: [{ address: "5Signer" }] });
  const signer = createHostSigner(createHostClient(fake.bridge, product));
  assert.equal((await signer.sign({
    account: "5Signer", payload: new Uint8Array([1]), purpose: "tx",
  })).success, false);
  fake.grant(product.id, "signing");
  assert.equal((await signer.sign({
    account: "5Signer", payload: new Uint8Array([1]), purpose: "tx",
  })).success, true);
  fake.revoke(product.id, "signing");
  assert.equal((await signer.sign({
    account: "5Signer", payload: new Uint8Array([1]), purpose: "tx",
  })).success, false);
});

test("selected signer binds every request to the selected live host account", async () => {
  const fake = createFakeHost({ accounts: [
    { address: "5First", name: "First" },
    { address: "5Second", name: "Second" },
  ] });
  fake.grant(product.id, "accounts");
  fake.grant(product.id, "signing");
  const selected = await selectHostSigner(
    createHostClient(fake.bridge, product), "5Second",
  );
  assert.equal(selected.success, true);
  if (!selected.success) return;
  assert.equal(selected.value.account.address, "5Second");
  const mismatch = await selected.value.sign({
    account: "5First", payload: new Uint8Array([1]), purpose: "tx",
  });
  assert.equal(!mismatch.success && mismatch.error.code, "account_mismatch");
  assert.equal((await selected.value.sign({
    account: "5Second", payload: new Uint8Array([1]), purpose: "tx",
  })).success, true);
});

test("account selection fails closed when no requested account exists", async () => {
  const fake = createFakeHost({ accounts: [] });
  fake.grant(product.id, "accounts");
  const selected = await selectHostSigner(createHostClient(fake.bridge, product));
  assert.equal(!selected.success && selected.error.code, "no_accounts");
});
