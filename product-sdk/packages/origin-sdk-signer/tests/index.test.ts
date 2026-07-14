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

import assert from "node:assert/strict";import test from "node:test";import { createHostClient } from "@cord-network/origin-sdk-host";import { createFakeHost } from "@cord-network/origin-sdk-host/testing";import { createHostSigner } from "../src/index.ts";
test("host signer requires a live grant and per-call host approval",async()=>{const fake=createFakeHost({accounts:[{address:"5Signer"}]});const signer=createHostSigner(createHostClient(fake.bridge,{id:"signer.app",name:"Signer"}));assert.equal((await signer.sign({account:"5Signer",payload:new Uint8Array([1]),purpose:"tx"})).success,false);fake.grant("signer.app","signing");assert.equal((await signer.sign({account:"5Signer",payload:new Uint8Array([1]),purpose:"tx"})).success,true);fake.revoke("signer.app","signing");assert.equal((await signer.sign({account:"5Signer",payload:new Uint8Array([1]),purpose:"tx"})).success,false)});
