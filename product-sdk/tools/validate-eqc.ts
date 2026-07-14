import { readFileSync } from "node:fs"; import { dirname,resolve } from "node:path"; import { fileURLToPath } from "node:url";
import { contractDigest,sha256File,validateEqcResult,validateResultSchemaContract,validateServiceSchemaContract,validateSloManifest } from "../packages/eqc/src/validate.ts";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"../.."),load=(p:string)=>JSON.parse(readFileSync(resolve(root,p),"utf8"));
const mp=resolve(root,"docs/evidence/performance/service-slo-manifest.json"),slo=load("docs/evidence/performance/service-slo-manifest.json");
if(slo.ratification?.canonical_envelope!=="docs/evidence/verification/p5/sdk-freeze-ratification-envelope.json")throw Error("E/Q/C must bind the P5 SDK-freeze envelope; P0 is historical only");
const env=load(slo.ratification.canonical_envelope);
const status=validateSloManifest(slo,env);validateServiceSchemaContract(load("docs/evidence/performance/service-slo-manifest.schema.json"));validateResultSchemaContract(load("docs/evidence/performance/benchmark-result.schema.json"));const hash=sha256File(mp),descriptor=contractDigest("descriptor",load("product-sdk/packages/descriptors/generated/orbis-descriptor.json"));
for(const k of ["E","Q","C"]){const f=load(`product-sdk/tests/eqc/fixtures/${k}.schema-only.json`);f.manifest_sha256=hash;f.runtime.descriptor_contract_sha256=descriptor;validateEqcResult(f,hash,slo,status.ratified)}
process.stdout.write(`PASS E/Q/C strict schema/client harness; p5_sdk_freeze_ratified=${status.ratified}; p0_historical_only=true; performance_claim=false\n`);
