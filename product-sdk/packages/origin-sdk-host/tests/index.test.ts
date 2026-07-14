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

import assert from "node:assert/strict"; import test from "node:test";
import { createHostClient } from "../src/index.ts"; import { createFakeHost } from "../src/testing.ts";
test("host client keeps permission and signing approval host-owned",async()=>{const fake=createFakeHost({accounts:[{address:"5Test"}]});const host=createHostClient(fake.bridge,{id:"festival.app",name:"Festival"});assert.equal((await host.accounts()).success,false);fake.grant("festival.app","accounts");const accounts=await host.accounts();assert.deepEqual(accounts.success&&accounts.value,[{address:"5Test"}]);fake.grant("festival.app","signing");const signed=await host.sign({account:"5Test",payload:new Uint8Array([1,2]),purpose:"check-in"});assert.equal(signed.success,true);fake.revoke("festival.app","signing");assert.equal((await host.sign({account:"5Test",payload:new Uint8Array([1]),purpose:"x"})).success,false)});
test("local storage is namespaced by product identity",async()=>{const fake=createFakeHost();fake.grant("a.app","local-storage");fake.grant("b.app","local-storage");const a=createHostClient(fake.bridge,{id:"a.app",name:"A"}),b=createHostClient(fake.bridge,{id:"b.app",name:"B"});await a.setLocal("key",new Uint8Array([7]));const stored=await a.getLocal("key"),missing=await b.getLocal("key");assert.deepEqual(stored.success&&stored.value,new Uint8Array([7]));assert.equal(missing.success&&missing.value,undefined)});
