#!/usr/bin/env python3
"""Adversarial mutation tests for the Orbis v5 pending verifier."""
from pathlib import Path
import json,os,subprocess,sys,tempfile
ROOT=Path(__file__).resolve().parents[1]; verifier=ROOT/'scripts/verify-orbis-completion-v5.py'; source=(ROOT/'docs/orbis-completion-manifest.toml').read_text()
env=os.environ.copy();env['ORBIS_V5_SKIP_V4']='1'
def rejected(label,text):
 with tempfile.NamedTemporaryFile('w',suffix='.toml',delete=False) as f:f.write(text);name=f.name
 try:r=subprocess.run([sys.executable,str(verifier),'--static','--manifest',name],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 finally:Path(name).unlink(missing_ok=True)
 if r.returncode==0:raise SystemExit('mutation accepted: '+label)
 print('rejected='+label)
def one(old,new):
 if source.count(old)!=1:raise SystemExit('mutation anchor not unique: '+old)
 return source.replace(old,new,1)
rejected('unrelated-row-body-status',one('id = "PAL-000"\nname = "system"\nindex = 0\npackage = "frame-system"','id = "PAL-000"\nname = "system-mutated"\nindex = 0\npackage = "frame-system"'))
rejected('local-upstream-package-identity',source.replace('package = "pallet-orbis-score"\nupstream_package = "indiv-pallet-score"','package = "indiv-pallet-score"\nupstream_package = "pallet-orbis-score"'))
rejected('migration-substitution',one('id = "MIG-pallet-orbis-score"','id = "MIG-indiv-pallet-score"'))
rejected('finite-count-addition',source+'\n[[slice2_evidence]]\nid = "S2-UNAPPROVED"\nstatus = "present"\n')
rejected('staged-gate6',one('id = "GATE-6-SLICE2-EVIDENCE"\norder = 6\nvalue = "independent evidence review of the bounded Slice 2 v5 transition"\nstatus = "pending"','id = "GATE-6-SLICE2-EVIDENCE"\norder = 6\nvalue = "independent evidence review of the bounded Slice 2 v5 transition"\nstatus = "present"'))
rejected('source-binding',source.replace('source_commit = "ce60319f85a13b29e143a4d15189616f38abe777"','source_commit = "317a3a5b3dda0964958e08b94c683b1cc1b8e387"',1))
rejected('output-binding',one('output_sha256 = "061418fe43d05df35b9ad4f83f931f574e76009407ef3e7d52139d9ebf37558c"','output_sha256 = "'+'0'*64+'"'))
rejected('human-contract',one('value = "Concrete direct and paid Meta Score/Honour surfaces preserve actor, nonce, payment, quota, business state, and rejection invariants."','value = "mutated contract"'))
rejected('contract-hash',one('assertion_sha256 = "b4c8410c960d0ecb7148d29871877a53efbcc9f46a59cbfd086dd01142d4ea84"','assertion_sha256 = "'+'0'*64+'"'))
rejected('benchmark-number',one('benchmark_measured_ref_time = "25000000"','benchmark_measured_ref_time = "25000001"'))
rejected('benchmark-supplemental-hash',one('benchmark_watch_json_sha256 = "af7fefee097c870a397dfceeb98d417eb64257af625a8cfed0afff77a1089255"','benchmark_watch_json_sha256 = "'+'0'*64+'"'))
rejected('benchmark-wasm-identity',one('benchmark_compiled_wasm_sha256 = "7bb2b93783280dc512373b124ed1c9ed66eb3dbf434a1be9b4e9b2234df333ac"','benchmark_compiled_wasm_sha256 = "'+'0'*64+'"'))
def mutate_file(label,path,transform):
 original=path.read_bytes()
 try:
  path.write_bytes(transform(original))
  rejected(label,source)
 finally:path.write_bytes(original)
marker_path=ROOT/'origin/orbis/evidence_markers_v5.rs'
mutate_file('missing-marker',marker_path,lambda b:b.replace(b'"slice2-surfaces"',b'"removed-surfaces"',1))
mutate_file('duplicate-marker',marker_path,lambda b:b.replace(b'pub const EVIDENCE_MARKERS_V5:',b'pub const EVIDENCE_MARKERS_V5_DUPLICATE: &[(&str, &str, &str, &str)] = EVIDENCE_MARKERS_V5;\npub const EVIDENCE_MARKERS_V5:',1))
mutate_file('coherent-artifact',ROOT/'docs/evidence/orbis-v5/S2-SURFACES-01.json',lambda b:b.replace(b'Concrete direct',b'Coherently mutated'))
surface_output=ROOT/'docs/evidence/orbis-v5/S2-SURFACES-01.out'
marker_line=b'assertion=S2-SURFACES-01:b4c8410c960d0ecb7148d29871877a53efbcc9f46a59cbfd086dd01142d4ea84\n'
mutate_file('missing-raw-marker',surface_output,lambda b:b.replace(marker_line,b'',1))
mutate_file('duplicate-raw-marker',surface_output,lambda b:b.replace(marker_line,marker_line+marker_line,1))
mutate_file('extra-raw-marker',surface_output,lambda b:b+marker_line.replace(b'S2-SURFACES-01',b'S2-EXTRA-01'))
proof=ROOT/'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json';original=proof.read_bytes()
try:
 data=json.loads(original);data['poison_control_removed']=False;proof.write_text(json.dumps(data,sort_keys=True,indent=2)+'\n')
 rejected('cold-poison-control',source)
finally:proof.write_bytes(original)
# Clean-tree enforcement is tested only when the caller starts from a clean tracked tree.
status=subprocess.run(['git','status','--porcelain','--untracked-files=all'],cwd=ROOT,text=True,stdout=subprocess.PIPE).stdout.splitlines()
status=[line for line in status if ' .omx/' not in line]
if not status:
 dirty=ROOT/'docs/evidence/orbis-v5/UNTRACKED-POISON';dirty.write_text('poison')
 try:
  r=subprocess.run([sys.executable,str(verifier),'--static'],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
  if r.returncode==0:raise SystemExit('dirty/untracked evidence accepted')
  print('rejected=dirty-untracked')
 finally:dirty.unlink()
print('orbis-v5-adversarial=ok')
