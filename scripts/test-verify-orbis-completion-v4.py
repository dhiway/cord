#!/usr/bin/env python3
from pathlib import Path
import os,subprocess,tempfile,re
ROOT=Path(__file__).resolve().parents[1]; BASE=(ROOT/'docs/orbis-completion-manifest.toml').read_text(); VERIFY=ROOT/'scripts/verify-orbis-completion-v4.py'
def run(text,contaminate=False):
 with tempfile.NamedTemporaryFile('w',suffix='.toml',delete=False) as f: f.write(text); name=f.name
 env=os.environ.copy()
 if contaminate: env['RUNTIME_METADATA_HASH']='contaminated-parent-value'
 try: return subprocess.run([str(VERIFY),'--static','--manifest',name],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 finally: Path(name).unlink()
def reject(name,text,needle):
 r=run(text)
 if r.returncode==0 or needle not in r.stdout: raise SystemExit(f'{name} did not reject as {needle}: {r.stdout}')
if run(BASE,True).returncode: raise SystemExit('positive manifest failed under contaminated parent env')
reject('zero-sha',BASE.replace('artifact_sha256 = "','artifact_sha256 = "'+'0'*64+'#',1),'artifact')
reject('missing-file',BASE.replace('artifact_path = "docs/evidence/orbis-v4/','artifact_path = "docs/evidence/orbis-v4/MISSING-',1),'artifact')
reject('bad-commit',BASE.replace('source_commit = "f88aa3faa6573582ca690fa3cace58b7f670aa88"','source_commit = "'+'f'*40+'"',1),'ancestor')
reject('bad-path',BASE.replace('source_paths = "origin/orbis/runtime/src/lib.rs','source_paths = "missing/runtime.rs',1),'historical path')
reject('bad-symbol',BASE.replace('source_symbol = "IntentPreimageV7"','source_symbol = "DefinitelyAbsentSymbol"',1),'historical symbol')
reject('broad-command',BASE.replace('meta_v6_fixtures::checked_in_meta_v7_fixtures_decode_all_recompute_and_match_hashes -- --exact','remediation',1),'broad non-mapping')
reject('contradiction',BASE.replace('status = "present"\nowner = "slice-1"','status = "planned"\nowner = "slice-1"',1),'contradiction')
reject('planned-fake',BASE.replace('source_paths = ""','source_paths = "docs/orbis-completion-manifest.toml"',1),'planned row has nonblank')
reject('unchecked',BASE.replace('state = "excluded"','state = "mystery"',1),'unchecked rows')
reject('assertion-mismatch',BASE.replace('expected_assertion = "exact canonical MetaTxExtension tuple order"','expected_assertion = "injected unmatched assertion"',1),'assertion mismatch')
reject('migration-conflation',BASE.replace('id = "PMIG-Bulletin-V6-to-V7"\npallet = "BulletinTransactionStorage"\nfrom_version = 6\nto_version = 7\nowner = "slice-1"','id = "PMIG-Bulletin-V6-to-V7"\npallet = "BulletinTransactionStorage"\nfrom_version = 6\nto_version = 7\nowner = "slice-10"',1),'migration conflated')
reject('missing-migration-id',re.sub(r'\[\[protocol_migration\]\]\nid = "PMIG-Bulletin-V7-to-V8".*?(?=\[\[)', '',BASE,count=1,flags=re.S),'identity enumeration')
reject('duplicate-id',BASE.replace('id = "PMIG-Bulletin-V7-to-V8"','id = "PMIG-Bulletin-V6-to-V7"',1),'duplicate normalized identity')
reject('output-drift',BASE.replace('output_sha256 = "','output_sha256 = "'+'f'*64+'#',1),'artifact binding mismatch')
fifth='''\n[[meta_contract]]\nid = "META-EVIDENCE-FIFTH"\nkind = "metadata-evidence"\nvalue = "forbidden fifth mode"\nsource_paths = ""\nsource_symbol = ""\ntest_or_command = ""\nexpected_assertion = ""\nartifact_path = ""\nartifact_sha256 = ""\nsource_commit = ""\nplanned_slice = "none"\ndependency_ids = "none"\nstatus = "planned"\n'''
reject('fifth-metadata-mode',BASE+fifth,'metadata evidence ID set mismatch')
print('15 adversarial verifier cases plus contaminated-parent positive control passed')
