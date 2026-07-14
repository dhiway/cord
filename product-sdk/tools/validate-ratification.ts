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

import { readFileSync } from "node:fs"; import { resolve } from "node:path"; import { ratificationStatus } from "../packages/core/src/ratification.ts";
for(const [phase,file] of [["p0","ratification-envelope.json"],["p5","sdk-freeze-ratification-envelope.json"]]){const path=resolve(import.meta.dirname,`../../docs/evidence/verification/${phase}/${file}`),envelope=JSON.parse(readFileSync(path,"utf8")),status=ratificationStatus(envelope);process.stdout.write(`PASS ${phase} canonical Ed25519 ratification envelope; targets_ratified=${status.p0TargetsRatified}; production_activation_ready=${status.productionActivationReady}; verified_roles=${status.verifiedRoles.join(",")||"none"}; performance_claim=${envelope.payload.performance_claim}; payload_sha256=${status.payloadSha256}\n`)}
