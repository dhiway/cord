#!/usr/bin/env python3
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Fail-closed P0 validator for the Origin/Orbis runtime-alignment freeze."""
import csv, hashlib, json, re, subprocess, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
errors=[]
def require(ok,msg):
 if not ok: errors.append(msg)
def text(path): return (ROOT/path).read_text()
def git(*args): return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
# Exact source baseline.
require(git('branch','--show-current') == 'sm-update-sub-0x63','wrong branch')
require(git('rev-parse','HEAD') == '439a1b62da11175129ad5390186230a64d569810','HEAD drift')
lock=text('Cargo.lock')
revs=set(re.findall(r'git\+https://github.com/dhiway/sdk\?branch=release-v1\.24\.0#([0-9a-f]{40})',lock))
require(revs == {'cc190ea83c590b6a14a6b9771ab02c81618dc118'},f'SDK lock mismatch: {sorted(revs)}')
origin=text('origin/base/runtime/src/lib.rs'); orbis=text('origin/orbis/runtime/src/lib.rs')
for src,wants,label in [(origin,['spec_name: alloc::borrow::Cow::Borrowed("origin")','spec_version: 9901','transaction_version: 2'],'Origin'),(orbis,['spec_name: Cow::Borrowed("orbis")','spec_version: 29','transaction_version: 8'],'Orbis')]:
 for want in wants: require(want in src,f'{label} version missing: {want}')
require('pub const ORBIS_ID: u32 = 1006;' in text('origin/base/runtime/constants/src/lib.rs'),'Orbis para id drift')
for want in ['BLOCK_PROCESSING_VELOCITY: u32 = 3','RELAY_PARENT_OFFSET: u32 = 1','UNINCLUDED_SEGMENT_CAPACITY: u32 = (3 + RELAY_PARENT_OFFSET) * BLOCK_PROCESSING_VELOCITY','RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6_000']:
 require(want in orbis,f'authoring baseline drift: {want}')
# Manifest truth records the independently reviewed Slice-2 closure. Parse only the four
# frozen scalar fields needed by this validator so repository Python 3.9 needs neither tomllib nor
# a third-party TOML package.  Requiring exactly one match fails closed on missing/duplicate keys.
manifest=text('docs/orbis-completion-manifest.toml')
def manifest_scalar(key):
 matches=re.findall(r'^'+re.escape(key)+r'\s*=\s*(?:"([^"]*)"|(\d+))\s*$',manifest,re.M)
 require(len(matches)==1,f'manifest field {key} must occur exactly once')
 if len(matches)!=1: return None
 string,integer=matches[0]
 return string if string else int(integer)
require(manifest_scalar('manifest_version') == 5,'manifest version is not 5')
require(manifest_scalar('evidence_phase') == 'closed','manifest evidence phase is not closed')
require(manifest_scalar('evidence_architect_status') == 'clear','architect evidence closure drift')
require(manifest_scalar('evidence_critic_status') == 'clear','critic evidence closure drift')
require('id = "GATE-6-SLICE2-EVIDENCE"' in manifest and 'status = "present"' in manifest[manifest.index('id = "GATE-6-SLICE2-EVIDENCE"'):], 'Gate 6 closure is not present')
# CSV contract.
p=ROOT/'docs/architecture/origin-orbis-runtime-alignment.csv'
with p.open(newline='') as f: rows=list(csv.DictReader(f)); fields=f.seek(0) or []
required={'id','layer','capability','owner','status','index_or_version','implementation','evidence','disposition','blocker'}
require(rows and required <= set(rows[0]),'alignment CSV columns incomplete')
ids=[r['id'] for r in rows]
require(len(ids)==len(set(ids)),'duplicate alignment ids')
require(set(r['status'] for r in rows) <= {'present','gap','excluded','blocked','frozen'},'unknown alignment status')
byid={r['id']:r for r in rows}
for id in ['BASE-BRANCH','BASE-HEAD','BASE-SDK','ORIGIN-VERSION','ORBIS-VERSION','ORBIS-PARA','ORBIS-AUTHORING','MANIFEST-V5']:
 require(id in byid,f'missing baseline row {id}')
require(byid.get('MANIFEST-V5',{}).get('status') == 'present','closed manifest evidence not represented')
# Evidence index hashes must bind every listed input/artifact (the index does not self-hash).
idx=json.loads(text('docs/evidence/p0/index.json'))
require(idx.get('head') == git('rev-parse','HEAD'),'evidence index HEAD drift')
require(idx.get('alignment_rows') == len(rows),'evidence index row count drift')
for artifact in idx.get('artifacts',[]):
 ap=ROOT/artifact['path']
 require(ap.is_file(),f"indexed artifact missing: {artifact['path']}")
 if ap.is_file(): require(hashlib.sha256(ap.read_bytes()).hexdigest() == artifact['sha256'],f"indexed artifact hash drift: {artifact['path']}")
# Exact runtime pallet inventories must be complete and index-unique per runtime.
def pallets(src):
 marker='construct_runtime! {' if 'construct_runtime! {' in src else 'construct_runtime!('; start=src.index(marker); b=src[start:src.index('\n);',start)]
 return {(m.group(1),int(m.group(3))) for m in re.finditer(r'^\s*([A-Za-z][A-Za-z0-9]*):\s*([A-Za-z0-9_:<>]+)\s*=\s*(\d+),',b,re.M)}
for owner,prefix,src in [('Origin','ORIGIN-PAL',origin),('Orbis','ORBIS-PAL',orbis)]:
 expected=pallets(src); actual={(r['capability'],int(r['index_or_version'])) for r in rows if r['owner']==owner and r['layer']=='pallet'}
 require(actual==expected,f'{owner} pallet inventory mismatch: missing={sorted(expected-actual)} extra={sorted(actual-expected)}')
 indexes=[i for _,i in actual]; require(len(indexes)==len(set(indexes)),f'{owner} duplicate pallet index')
if errors:
 print('\n'.join('ERROR: '+e for e in errors)); sys.exit(1)
print(f'PASS: branch/head/SDK/runtime/manifest and {len(rows)} alignment rows are exact')
