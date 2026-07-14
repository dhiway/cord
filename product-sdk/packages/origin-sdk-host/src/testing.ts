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

import { createHash } from "node:crypto";
import { OriginSdkError } from "@cord-network/origin-sdk-errors";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { HostAccount, HostCapability, OriginHostBridge } from "./index.ts";
export function createFakeHost(options:{accounts?:readonly HostAccount[]}={}){const grants=new Set<string>(),storage=new Map<string,Uint8Array>();const key=(p:string,c:string)=>`${p}:${c}`;const bridge:OriginHostBridge={async authorize(product,capability){return grants.has(key(product.id,capability))?ok({productId:product.id,capability}):err(new OriginSdkError({source:"fake-host",domain:"permission",code:"permission_denied",message:"Permission denied"}))},async accounts(){return ok(options.accounts??[])},async approveAndSign(product,request){if(!grants.has(key(product.id,"signing")))return err(new OriginSdkError({source:"fake-host",domain:"signing",code:"signing_rejected",message:"Signing rejected"}));return ok(new Uint8Array(createHash("sha256").update(request.payload).digest()))},async getStorage(product,k){return ok(storage.get(`${product.id}:${k}`)?.slice())},async setStorage(product,k,value){storage.set(`${product.id}:${k}`,value.slice());return ok(undefined)}};return{bridge,grant(productId:string,capability:HostCapability){grants.add(key(productId,capability))},revoke(productId:string,capability:HostCapability){grants.delete(key(productId,capability))}}}
