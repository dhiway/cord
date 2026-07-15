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


import type { PreparedTransaction, TransactionStatus } from "@cord-network/origin-sdk-tx";
import type { ReviveDryRunResult, ReviveRuntimeAdapter } from "./index.ts";

export function createFakeReviveRuntime(options:{readonly query?:ReviveDryRunResult;readonly statuses?:readonly TransactionStatus[]}={}){
  const calls:Array<{readonly operation:string;readonly request:unknown}>=[];
  const transaction:PreparedTransaction={async *signSubmitAndWatch(){for(const status of options.statuses??[])yield status}};
  const runtime:ReviveRuntimeAdapter={
    async dryRun(_at,request){calls.push({operation:"query",request});return options.query??{reverted:false,data:new Uint8Array(),gasRequired:0n,storageDeposit:0n}},
    async prepareCall(_at,request){calls.push({operation:"call",request});return transaction},
    async prepareInstantiate(_at,request){calls.push({operation:"instantiate",request});return transaction},
  };
  return {runtime,get calls(){return [...calls]},reset(){calls.length=0}};
}
