#!/usr/bin/env python3
from pathlib import Path
import subprocess,tempfile
ROOT=Path(__file__).resolve().parents[1]; BASE=(ROOT/'docs/orbis-completion-manifest.toml').read_text(); VERIFY=ROOT/'scripts/verify-orbis-completion-v4.py'
def run(text):
 with tempfile.NamedTemporaryFile('w',suffix='.toml',delete=False) as f: f.write(text); name=f.name
 try: return subprocess.run([str(VERIFY),'--static','--manifest',name],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 finally: Path(name).unlink()
def reject(name,text,needle):
 r=run(text)
 if r.returncode==0 or needle not in r.stdout: raise SystemExit(f'{name} did not reject as {needle}: {r.stdout}')
if run(BASE).returncode: raise SystemExit('positive manifest failed')
reject('zero-sha',BASE.replace('artifact_sha256 = "','artifact_sha256 = "'+'0'*64+'#',1),'artifact')
reject('missing-file',BASE.replace('artifact_path = "docs/evidence/orbis-v4/','artifact_path = "docs/evidence/orbis-v4/MISSING-',1),'artifact')
reject('bad-commit',BASE.replace('source_commit = "f88aa3faa6573582ca690fa3cace58b7f670aa88"','source_commit = "'+'f'*40+'"',1),'ancestor')
reject('bad-path',BASE.replace('source_paths = "origin/orbis/runtime/src/lib.rs','source_paths = "missing/runtime.rs',1),'historical path')
reject('bad-symbol',BASE.replace('source_symbol = "IntentPreimageV7"','source_symbol = "DefinitelyAbsentSymbol"',1),'historical symbol')
reject('broad-command',BASE.replace('tests::sponsored_meta_tx_preserves_actor_and_rejects_replay_and_forgery -- --exact','remediation',1),'broad non-mapping')
reject('contradiction',BASE.replace('status = "present"\nowner = "slice-1"','status = "planned"\nowner = "slice-1"',1),'contradiction')
reject('planned-fake',BASE.replace('source_paths = ""','source_paths = "docs/orbis-completion-manifest.toml"',1),'planned row has nonblank')
reject('unchecked',BASE.replace('state = "excluded"','state = "mystery"',1),'unchecked rows')
reject('output-hash',BASE.replace('output_sha256 = "','output_sha256 = "'+'f'*64+'#',1),'output hash')
print('10 negative verifier cases and positive control passed')
