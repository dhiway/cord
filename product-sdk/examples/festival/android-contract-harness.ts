import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { validatePlatformContractHarness } from "./mobile-contract-harness.ts";

const report = await validatePlatformContractHarness("android");
if (process.argv.includes("--write")) {
  const path = resolve(
    import.meta.dirname,
    "../../../docs/evidence/verification/p6/festival-android-contract-parity.report.json",
  );
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${path}\n`);
} else {
  process.stdout.write(`${JSON.stringify(report)}\n`);
}
