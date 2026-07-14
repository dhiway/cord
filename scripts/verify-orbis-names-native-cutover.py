#!/usr/bin/env python3
"""Prove the CORD product/runtime tree has no executable legacy Orbis Names contract survivor."""
import json,re
from datetime import datetime,timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
REPORT=ROOT/'docs/evidence/verification/p3/names-native-cutover.json'
SCAN=('origin/orbis','origin-rs','product-sdk','node','runtimes','pallets')
BINARY_SUFFIX={'.sol','.abi','.bin'}
# Only Orbis Names-qualified legacy adapter/deployment vocabulary is prohibited. Generic Revive support is unrelated.
FORBIDDEN=(
 re.compile(r'(?i)names.{0,60}(?:contractaddress|contract_address|abi_json|bytecode|eth_call|deployproxy|proxyaddress)'),
 re.compile(r'(?i)(?:contractaddress|contract_address|abi_json|eth_call).{0,60}names'),
)
NAMESPACE_SCAN=(
 ROOT/'origin/orbis/pallets/individuality-support/src',
 ROOT/'origin/orbis/pallets/resources/src',
 ROOT/'origin/orbis/pallets/people-lite/src',
 ROOT/'origin/orbis/runtime/src',
)
FORBIDDEN_SECONDARY_NAMESPACE=(
 'UsernameOwnerOf','UsernameReservationQueue','ReservationOf','UsernameReservationDuration',
 'remove_expired_username_reservation','set_username_reservation_duration',
 'InvalidExpiredUsernameReservationRemoval','is_lite_person_label',
 'names-gateway','names_gateway',
)
REMOVED_LABEL_HELPER=ROOT/'origin/orbis/pallets/individuality-support/src/labels.rs'
def ignored(p:Path)->bool:
 parts=set(p.parts)
 return bool(parts & {'.git','target','node_modules','dist','build','.omx'})
def main():
 survivors=[]; namespace_violations=[]; files=0
 for root_name in SCAN:
  root=ROOT/root_name
  if not root.exists(): continue
  for p in root.rglob('*'):
   if not p.is_file() or ignored(p): continue
   files+=1
   rel=p.relative_to(ROOT).as_posix()
   if p.suffix.lower() in BINARY_SUFFIX and 'names' in rel.lower():
    survivors.append({'path':rel,'reason':'legacy Orbis Names contract artifact'})
    continue
   if p.suffix.lower() not in {'.rs','.ts','.tsx','.js','.json','.toml','.md'}: continue
   try: text=p.read_text(errors='strict')
   except (UnicodeError,OSError): continue
   for pattern in FORBIDDEN:
    match=pattern.search(text)
    if match:
     survivors.append({'path':rel,'reason':'legacy Orbis Names ABI/address/deployment adapter','match':match.group(0)[:160]})
     break
 for root in NAMESPACE_SCAN:
  for p in root.rglob('*'):
   if not p.is_file() or ignored(p) or p.suffix.lower() not in {'.rs','.toml','.md'}: continue
   text=p.read_text(errors='strict')
   for symbol in FORBIDDEN_SECONDARY_NAMESPACE:
    if symbol in text:
     namespace_violations.append({
      'path':p.relative_to(ROOT).as_posix(),
      'reason':'secondary DNS/name-registry authority surface',
      'symbol':symbol,
     })
 if REMOVED_LABEL_HELPER.exists():
  namespace_violations.append({
   'path':REMOVED_LABEL_HELPER.relative_to(ROOT).as_posix(),
   'reason':'removed username-label helper module still exists',
  })
 runtime=(ROOT/'origin/orbis/runtime/src/lib.rs').read_text()
 if not re.search(r'\bNames\s*:\s*pallet_orbis_names\b',runtime):
  namespace_violations.append({
   'path':'origin/orbis/runtime/src/lib.rs',
   'reason':'canonical native Orbis Names pallet is not wired into the runtime',
  })
 report={
  'schema_version':1,'generated_at':datetime.now(timezone.utc).isoformat(),
  'scope':'CORD repository executable runtime/node/Rust SDK/TypeScript SDK sources only',
  'clean_genesis':True,'backward_compatibility':False,'data_migration':False,
  'scanned_roots':list(SCAN),'files_scanned':files,
  'provenance_only_allowlist':['docs/evidence/p0-contract-native/**','docs/sdk/contract-to-native-map.json','docs/adr/**'],
  'legacy_names_contract_survivors':survivors,'survivor_count':len(survivors),
  'secondary_namespace_authority_violations':namespace_violations,
  'names_is_sole_dns_name_registry_authority':not namespace_violations,
  'status':'PASS' if not survivors and not namespace_violations else 'FAIL',
 }
 REPORT.parent.mkdir(parents=True,exist_ok=True); REPORT.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
 failures=survivors+namespace_violations
 if failures: raise SystemExit('\n'.join(f"{x['path']}: {x['reason']}" for x in failures))
 print(f"PASS: scanned {files} product/runtime files; zero executable legacy Orbis Names contract survivors; Orbis Names is the sole DNS/name-registry authority")
if __name__=='__main__': main()
