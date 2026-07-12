#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,os,subprocess,tempfile,re
ROOT=Path(__file__).resolve().parents[1]; BASE=(ROOT/'docs/orbis-completion-manifest.toml').read_text(); VERIFY=ROOT/'scripts/verify-orbis-completion-v4.py'
def run(text,contaminate=False,static=True):
 with tempfile.NamedTemporaryFile('w',suffix='.toml',delete=False) as f: f.write(text); name=f.name
 env=os.environ.copy()
 if contaminate: env['RUNTIME_METADATA_HASH']='contaminated-parent-value'
 try:
  args=[str(VERIFY),'--manifest',name]
  if static: args.insert(1,'--static')
  return subprocess.run(args,cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 finally: Path(name).unlink()
def reject(name,text,needle):
 r=run(text)
 if r.returncode==0 or needle not in r.stdout: raise SystemExit(f'{name} did not reject as {needle}: {r.stdout}')
positive=run(BASE,True,False)
if positive.returncode: raise SystemExit('full positive manifest failed under contaminated parent env: '+positive.stdout)
reject('architect-clear-regression',BASE.replace('architect_status = "clear"','architect_status = "pending"',1),'Architect and Critic CLEAR evidence must be exact')
reject('critic-clear-regression',BASE.replace('critic_status = "clear"','critic_status = "pending"',1),'Architect and Critic CLEAR evidence must be exact')
gate_regression=re.sub(r'(\[\[remediation_gate\]\]\nid = "GATE-5-EVIDENCE".*?status = ")present(" )',r'\1planned\2',BASE,count=1,flags=re.S)
if gate_regression==BASE:
 gate_regression=re.sub(r'(\[\[remediation_gate\]\]\nid = "GATE-5-EVIDENCE".*?status = ")present("\n)',r'\1planned\2',BASE,count=1,flags=re.S)
reject('gate5-present-regression',gate_regression,'planned row has nonblank')
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
def coherent_marker_forgery():
 pattern=r'(\[\[meta_contract\]\]\nid = "META-ALIAS-VERIFY-CONSUME"\n.*?)(?=\[\[)'
 block=re.search(pattern,BASE,re.S).group(1)
 fields=dict(re.findall(r'^([A-Za-z0-9_]+) = "([^"]*)"$',block,re.M))
 forged_expected='coherently forged assertion absent from source registry'
 forged_digest=hashlib.sha256(forged_expected.encode()).hexdigest()
 artifact=json.loads((ROOT/fields['artifact_path']).read_text())
 original_log=(ROOT/artifact['output_artifact']).read_text()
 forged_log=original_log.replace('assertion='+fields['id']+':'+fields['assertion_sha256'],'assertion='+fields['id']+':'+forged_digest)
 log=tempfile.NamedTemporaryFile('w',suffix='.log',delete=False); log.write(forged_log); log.close()
 output_hash=hashlib.sha256(forged_log.encode()).hexdigest()
 artifact.update(expected=forged_expected,assertion_sha256=forged_digest,output_sha256=output_hash,output_artifact=log.name,output_artifact_sha256=output_hash)
 art=tempfile.NamedTemporaryFile('w',suffix='.json',delete=False); art.write(json.dumps(artifact,sort_keys=True,indent=2)+'\n'); art.close()
 artifact_hash=hashlib.sha256(Path(art.name).read_bytes()).hexdigest()
 updates={'expected_assertion':forged_expected,'assertion_sha256':forged_digest,'output_sha256':output_hash,'artifact_path':art.name,'artifact_sha256':artifact_hash}
 forged_block=block
 for key,value in updates.items(): forged_block=re.sub(r'^'+key+r' = "[^"]*"$',key+' = "'+value+'"',forged_block,flags=re.M)
 try:
  result=run(re.sub(pattern,lambda _:forged_block,BASE,count=1,flags=re.S))
  if result.returncode==0 or 'source marker registry mismatch' not in result.stdout: raise SystemExit('coherent marker forgery was not rejected by source registry: '+result.stdout)
 finally:
  Path(log.name).unlink(); Path(art.name).unlink()
coherent_marker_forgery()
reject('migration-conflation',BASE.replace('id = "PMIG-Bulletin-V6-to-V7"\npallet = "BulletinTransactionStorage"\nfrom_version = 6\nto_version = 7\nowner = "slice-1"','id = "PMIG-Bulletin-V6-to-V7"\npallet = "BulletinTransactionStorage"\nfrom_version = 6\nto_version = 7\nowner = "slice-10"',1),'migration conflated')
reject('missing-migration-id',re.sub(r'\[\[protocol_migration\]\]\nid = "PMIG-Bulletin-V7-to-V8".*?(?=\[\[)', '',BASE,count=1,flags=re.S),'manifest inventory')
reject('duplicate-id',BASE.replace('id = "PMIG-Bulletin-V7-to-V8"','id = "PMIG-Bulletin-V6-to-V7"',1),'duplicate normalized identity')
reject('output-drift',BASE.replace('output_sha256 = "','output_sha256 = "'+'f'*64+'#',1),'artifact binding mismatch')
fifth='''\n[[meta_contract]]\nid = "META-EVIDENCE-FIFTH"\nkind = "metadata-evidence"\nvalue = "forbidden fifth mode"\nsource_paths = ""\nsource_symbol = ""\ntest_or_command = ""\nexpected_assertion = ""\nartifact_path = ""\nartifact_sha256 = ""\nsource_commit = ""\nplanned_slice = "none"\ndependency_ids = "none"\nstatus = "planned"\n'''
reject('fifth-metadata-mode',BASE+fifth,'manifest inventory')
# Inventory coverage spans ordinary present/planned/excluded rows and independent table families.
for name,pattern in [
 ('delete-present-call',r'\[\[protocol_call\]\]\nid = "CALL-Resources-12".*?(?=\[\[)'),
 ('delete-planned-provider',r'\[\[provider_v8_contract\]\]\nid = "BUL-V8-FIELD".*?(?=\[\[)'),
 ('delete-excluded-capability',r'\[\[exclusion\]\]\nid = "EXC-staking".*?(?=\[\[)'),
 ('delete-stateless-source',r'\[\[source\]\]\nid = "dhiway-sdk".*?(?=\[\[)'),
]:
 mutated,n=re.subn(pattern,'',BASE,count=1,flags=re.S)
 if n!=1: raise SystemExit(name+' fixture did not match')
 reject(name,mutated,'manifest inventory')
added='''\n[[protocol_call]]\nid = "CALL-FORGED-ADDITION"\nstate = "planned"\n'''
reject('inventory-addition',BASE+added,'manifest inventory')
reject('inventory-rename',BASE.replace('id = "CALL-Resources-12"','id = "CALL-Resources-12-RENAMED"',1),'manifest inventory')

proof_target=Path('/tmp/orbis-v4-equivalence-target'); proof_target.mkdir(parents=True,exist_ok=True)
(proof_target/'STALE-POISON').write_text('must be removed before both cold builds\n')
proof_check=subprocess.run([str(ROOT/'scripts/prove-orbis-marker-nonruntime.sh'),'--check'],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
if proof_check.returncode: raise SystemExit('cold Wasm proof --check or stale-target cleanup failed: '+proof_check.stdout)

def production_cleanliness_cases():
 worktree=Path(tempfile.mkdtemp(prefix='orbis-v4-production-')); worktree.rmdir()
 subprocess.run(['git','worktree','add','--detach',str(worktree),'HEAD'],cwd=ROOT,check=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 try:
  verifier=worktree/'scripts/verify-orbis-completion-v4.py'
  before=subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=worktree,text=True)
  clean=subprocess.run([str(verifier),'--static'],cwd=worktree,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  after=subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=worktree,text=True)
  if clean.returncode or before!=after: raise SystemExit('clean production verifier failed or mutated tracked worktree: '+clean.stdout)
  output_log=next((worktree/'docs/evidence/orbis-v4/outputs').glob('*.log'))
  dirty_matrix=[
   worktree/'docs/orbis-completion-manifest.toml',
   worktree/'origin/orbis/runtime/src/meta_v6.rs',
   worktree/'origin/orbis/runtime/src/remediation_v3.rs',
   worktree/'scripts/prove-orbis-marker-nonruntime.sh',
   worktree/'docs/evidence/orbis-v4/marker-nonruntime-equivalence.json',
   worktree/'origin/orbis/evidence_markers_v4.rs',
   worktree/'origin/orbis/evidence_inventory_v4.rs',
   worktree/'docs/evidence/orbis-v4/critic-review-block-1.md',
   output_log,
   worktree/'docs/evidence/orbis-v4/verification-report.json',
   worktree/'docs/evidence/orbis-v4/verification-report.sha256',
   worktree/'docs/evidence/orbis-v4/META-ALIAS-VERIFY-CONSUME.json',
  ]
  for path in dirty_matrix:
   original=path.read_bytes(); path.write_bytes(original+b'\n')
   dirty=subprocess.run([str(verifier),'--static'],cwd=worktree,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
   path.write_bytes(original)
   if dirty.returncode==0: raise SystemExit('dirty production path was accepted: '+str(path))
  stray=worktree/'docs/evidence/orbis-v4/coherent-untracked-evidence.json'; stray.write_text('{"schema":"coherent-forgery"}\n')
  untracked=subprocess.run([str(verifier),'--static'],cwd=worktree,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  if untracked.returncode==0 or 'contains untracked files' not in untracked.stdout: raise SystemExit('untracked production evidence was not rejected: '+untracked.stdout)
  stray.unlink()
  replacement=worktree/'docs/evidence/orbis-v4/critic-review-block-1.md'
  subprocess.run(['git','rm','--cached','--',str(replacement.relative_to(worktree))],cwd=worktree,check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  replaced=subprocess.run([str(verifier),'--static'],cwd=worktree,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  subprocess.run(['git','reset','HEAD','--',str(replacement.relative_to(worktree))],cwd=worktree,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  if replaced.returncode==0: raise SystemExit('untracked replacement production evidence was accepted')
 finally:
  subprocess.run(['git','worktree','remove','--force',str(worktree)],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
production_cleanliness_cases()
print('expanded adversarial verifier cases, production cleanliness controls, and full contaminated-parent positive passed')
