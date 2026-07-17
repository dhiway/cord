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

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const root = process.cwd();
const generatedTsDir = path.join(root, 'product-sdk/tools/origin-host-conformance/generated');
const generatedRustDir = path.join(root, 'origin-rs/tests/generated');
const runtimeTsDir = path.join(root, 'product-sdk/packages/origin-sdk-host/src/internal/v2');
const runtimeRustDir = path.join(root, 'origin-rs/src/product_sdk/host_v2');
fs.mkdirSync(generatedTsDir, { recursive: true });
fs.mkdirSync(generatedRustDir, { recursive: true });
fs.mkdirSync(runtimeTsDir, { recursive: true });
fs.mkdirSync(runtimeRustDir, { recursive: true });
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const cddl = read('docs/specs/origin-host-registry-v2.cddl');
const projectionText = read('docs/specs/origin-host-registry-v2.schema.json');
const projection = JSON.parse(projectionText);
const operationRegistry = JSON.parse(read('docs/specs/origin-host-registry-v2.operations.json'));
const ops = operationRegistry.operations;
const errors = JSON.parse(read('docs/specs/origin-host-registry-v2.errors.json')).errors;
const header = read('HEADER-GPL3').trimEnd();
const id = s => s.replace(/[^A-Za-z0-9]+(.)?/g, (_, x) => x ? x.toUpperCase() : '').replace(/^[0-9]/, '_$&');

function splitTop(source, separator) {
  const out = []; let start = 0; let depth = 0; let quote = false; let escaped = false;
  for (let i = 0; i < source.length; i++) {
    const c = source[i];
    if (quote) { if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === '"') quote = false; continue; }
    if (c === '"') quote = true; else if ('([{'.includes(c)) depth++; else if (')]}'.includes(c)) depth--;
    else if (depth === 0 && source.startsWith(separator, i)) { out.push(source.slice(start, i).trim()); start = i + separator.length; i += separator.length - 1; }
  }
  out.push(source.slice(start).trim()); return out.filter(Boolean);
}
const definitions = Object.fromEntries([...cddl.matchAll(/^([A-Za-z][A-Za-z0-9]*)\s*=\s*(.+)$/gm)].map(m => [m[1], m[2].trim()]));
const dec = x => String(BigInt(x));
function parseCddl(expr) {
  const alts = splitTop(expr, ' / '); if (alts.length > 1) return { kind: 'union', variants: alts.map(parseCddl) };
  const t = expr.trim();
  if (t.startsWith('{') && t.endsWith('}')) {
    const fields = splitTop(t.slice(1, -1), ',').map(member => { const m = member.match(/^(\?)?(\d+):\s*(.+)$/); if (!m) throw Error('unsupported CDDL map member '+member); return { key: Number(m[2]), required: !m[1], schema: parseCddl(m[3]) }; });
    return { kind: 'map', fields };
  }
  if (t.startsWith('[') && t.endsWith(']')) { const m = t.slice(1,-1).trim().match(/^(\d+)\*(\d+)\s+(.+)$/); if (!m) throw Error('unsupported CDDL array '+t); return { kind:'array', min:Number(m[1]), max:Number(m[2]), items:parseCddl(m[3]) }; }
  if (t === 'bool') return { kind:'bool' }; if (t === 'true' || t === 'false') return { kind:'const', value:t === 'true' };
  if (/^"/.test(t)) return { kind:'const', value:JSON.parse(t) }; if (/^\d+$/.test(t)) return { kind:'const', value:Number(t) };
  let m = t.match(/^(\d+)\.\.(\d+)$/); if (m) return { kind:'uint', min:dec(m[1]), max:dec(m[2]) };
  m = t.match(/^bytes(?:\s+\.size\s*(?:\((\d+)\.\.(\d+)\)|(\d+)))?$/); if (m) { const lo=m[1]??m[3]??'0', hi=m[2]??m[3]??'18446744073709551615'; return { kind:'bytes', min:Number(lo), max:Number(hi) }; }
  m = t.match(/^tstr(?:\s+\.size\s*(?:\((\d+)\.\.(\d+)\)|(\d+)))?$/); if (m) { const lo=m[1]??m[3]??'0', hi=m[2]??m[3]??'18446744073709551615'; return { kind:'text', min:Number(lo), max:Number(hi), nfc:true }; }
  if (definitions[t]) return { kind:'ref', name:t };
  throw Error('unsupported CDDL expression '+t);
}
function projectionNode(node, production) {
  if (node.$ref) return { kind:'ref', name:node.$ref.replace('#/$defs/','') };
  if (node.oneOf) return { kind:'union', variants:node.oneOf.map(x => projectionNode(x, production)) };
  if (node.enum && node['x-cord-cbor-discriminants']) { const values=Object.values(node['x-cord-cbor-discriminants']).map(Number).sort((a,b)=>a-b); if(values.length===1)return {kind:'const',value:values[0]}; if(values.every((x,i)=>x===values[0]+i))return {kind:'uint',min:String(values[0]),max:String(values.at(-1))}; return {kind:'union',variants:values.map(value=>({kind:'const',value}))}; }
  if (Object.hasOwn(node,'const')) return { kind:'const', value:node.const };
  if (node.type === 'object') { const required=new Set(node.required||[]); const fields=Object.entries(node.properties||{}).map(([key,schema])=>({key:Number(key),required:required.has(key),schema:projectionNode(schema,production)})).sort((a,b)=>a.key-b.key); if(node.additionalProperties!==false)throw Error('open JSON projection '+production); return {kind:'map',fields}; }
  if (node.type === 'array') return {kind:'array',min:node.minItems??0,max:node.maxItems??Number.MAX_SAFE_INTEGER,items:projectionNode(node.items,production)};
  if (node.type === 'boolean') return {kind:'bool'};
  if (node.type === 'integer') return {kind:'uint',min:dec(node.minimum??0),max:dec(node.maximum??'18446744073709551615')};
  if (node.type === 'string' && node.contentEncoding === 'base64url') return {kind:'bytes',min:node['x-cord-decoded-minBytes']??0,max:node['x-cord-decoded-maxBytes']??Number.MAX_SAFE_INTEGER};
  if (node.type === 'string' && node['x-cord-max']) return {kind:'uint',min:'0',max:dec(node['x-cord-max'])};
  if (node.type === 'string') return {kind:'text',min:node['x-cord-utf8-minBytes']??0,max:node['x-cord-utf8-maxBytes']??Number.MAX_SAFE_INTEGER,nfc:node['x-cord-nfc']===true};
  throw Error('unsupported JSON projection node for '+production+': '+JSON.stringify(node));
}
const cddlSchemas = Object.fromEntries(Object.entries(definitions).map(([name,expr])=>[name,parseCddl(expr)]));
const jsonSchemas = Object.fromEntries(Object.entries(projection.$defs).map(([name,node])=>[name,projectionNode(node,name)]));
const semanticMismatch=[];
for(const name of new Set([...Object.keys(cddlSchemas),...Object.keys(jsonSchemas)])) if(JSON.stringify(cddlSchemas[name])!==JSON.stringify(jsonSchemas[name])) semanticMismatch.push({name,cddl:cddlSchemas[name],projection:jsonSchemas[name]});
if(semanticMismatch.length) { console.error(JSON.stringify({semantic_mismatch:semanticMismatch.slice(0,5)},null,2)); process.exit(1); }
const requiredDriftProbe=structuredClone(projection.$defs.StorageBucketCreateFrame);
requiredDriftProbe.required=requiredDriftProbe.required.filter(key=>key!=='8');
if(JSON.stringify(projectionNode(requiredDriftProbe,'StorageBucketCreateFrame'))===JSON.stringify(cddlSchemas.StorageBucketCreateFrame))throw Error('hostile JSON required-field drift was not rejected');
const crossFieldRules = [
 {production:'StorageObjectRangeRequest',id:'range-length-positive',kind:'uint-positive',key:3},
 {production:'StorageBucketGrantRequest',id:'grant-expiry-after-issued',kind:'uint-greater',left:4,right:3},
 {production:'StorageDriveShareRequest',id:'share-expiry-after-issued',kind:'uint-greater',left:4,right:3},
 {production:'ProviderCapabilityV1',id:'capability-expiry-after-issued',kind:'uint-greater',left:13,right:12},
 {production:'ProviderCapabilityV1',id:'capability-validity-at-most-128',kind:'uint-delta-max',left:13,right:12,max:'128'},
 {production:'ResumeTokenV1',id:'resume-expiry-after-issued',kind:'uint-greater',left:12,right:11},
 {production:'ResumeTokenV1',id:'resume-validity-at-most-128',kind:'uint-delta-max',left:12,right:11,max:'128'},
 {production:'RecoveryInstallV2',id:'recovery-seed-nonzero',kind:'bytes-nonzero',key:2},
 {production:'DriveManifestV1',id:'drive-cids-sorted-unique',kind:'bytes-array-sorted-unique',key:3},
 {production:'S3ObjectVersionV1',id:'s3-key-no-nul',kind:'bytes-no-nul',key:2}
];
const semanticTable={schema_version:2,cddl_sha256:sha(cddl),json_projection_sha256:sha(projectionText),schemas:cddlSchemas,cross_field_rules:crossFieldRules};
const semanticJson=JSON.stringify(semanticTable);
const semanticHash=sha(semanticJson);
const rustVariant=s=>{const v=id(s);return v[0].toUpperCase()+v.slice(1)};
const defs=Object.entries(definitions).map(([name,cddl])=>({name,cddl}));
const tsNode = node => {
  if (node.kind === 'ref') return node.name;
  if (node.kind === 'union') return node.variants.map(tsNode).join(' | ');
  if (node.kind === 'map') return `{ ${node.fields.map(field => `readonly ${field.key}${field.required ? '' : '?'}: ${tsNode(field.schema)}`).join('; ')} }`;
  if (node.kind === 'array') return `readonly (${tsNode(node.items)})[]`;
  if (node.kind === 'uint') return 'number | bigint';
  if (node.kind === 'bytes') return 'Uint8Array';
  if (node.kind === 'text') return 'string';
  if (node.kind === 'bool') return 'boolean';
  if (node.kind === 'const') return JSON.stringify(node.value);
  throw Error(`unsupported TypeScript schema node ${node.kind}`);
};
const operationBindings = Object.fromEntries(ops.map(operation => [operation.name, {
  code: operation.code,
  featureId: operation.feature_id,
  grantScope: operation.grant_scope,
  consentMode: operation.consent_mode,
  stateChanging: operation.state_changing,
  operationIdRequired: operation.operation_id_required,
  frame: operation.cddl.Request.replace(/Request$/, 'Frame'),
  request: operation.cddl.Request,
  accepted: operation.cddl.Accepted,
  progress: operation.cddl.Progress,
  result: operation.cddl.Result,
  error: operation.cddl.Error,
  allowedErrors: operation.allowed_errors.map(error => error.code),
}]));
const featureIds = [...new Set(ops.map(operation => operation.feature_id))].sort();
const ts=`${header}\n\nexport const REGISTRY_SHA256 = '${sha(cddl)}' as const;\nexport const JSON_PROJECTION_SHA256 = '${sha(projectionText)}' as const;\nexport const SEMANTIC_SCHEMA_SHA256 = '${semanticHash}' as const;\nexport const PROTOCOL = 'cord.origin.host/2' as const;\nexport const OPERATION_CODES = ${JSON.stringify(Object.fromEntries(ops.map(o=>[id(o.name),o.code])),null,2)} as const;\nexport const ERROR_CODES = ${JSON.stringify(Object.fromEntries(errors.map(e=>[e.name,e.code])),null,2)} as const;\nexport type OperationName = ${ops.map(o=>`'${o.name}'`).join(' | ')};\nexport type ErrorName = ${errors.map(e=>`'${e.name}'`).join(' | ')};\nexport type CddlTypeName = ${defs.map(d=>`'${d.name}'`).join(' | ')};\nexport type ClosedSchemaNode = { readonly kind:'ref'; readonly name:CddlTypeName } | { readonly kind:'union'; readonly variants:readonly ClosedSchemaNode[] } | { readonly kind:'map'; readonly fields:readonly {readonly key:number;readonly required:boolean;readonly schema:ClosedSchemaNode}[] } | {readonly kind:'array';readonly min:number;readonly max:number;readonly items:ClosedSchemaNode} | {readonly kind:'uint';readonly min:string;readonly max:string} | {readonly kind:'bytes'|'text';readonly min:number;readonly max:number;readonly nfc?:boolean} | {readonly kind:'bool'} | {readonly kind:'const';readonly value:number|string|boolean};\nexport interface ClosedTypeDefinition { readonly name:CddlTypeName; readonly cddl:string; readonly schema:ClosedSchemaNode; }\nexport const CLOSED_TYPES:readonly ClosedTypeDefinition[]=${JSON.stringify(defs.map(d=>({...d,schema:cddlSchemas[d.name]})))} as const;\nexport const CLOSED_SCHEMAS:Readonly<Record<CddlTypeName,ClosedSchemaNode>>=${JSON.stringify(cddlSchemas)} as const;\nexport const CROSS_FIELD_RULES=${JSON.stringify(crossFieldRules)} as const;\nexport const SEMANTIC_TABLE_JSON=${JSON.stringify(semanticJson)} as const;\n`;
const rust=`${header}\n\npub const REGISTRY_SHA256:&str="${sha(cddl)}";\npub const JSON_PROJECTION_SHA256:&str="${sha(projectionText)}";\npub const SEMANTIC_SCHEMA_SHA256:&str="${semanticHash}";\npub const PROTOCOL:&str="cord.origin.host/2";\n#[derive(Clone,Copy,Debug,Eq,PartialEq)] #[repr(u16)] pub enum OperationCode {\n${ops.map(o=>`    ${rustVariant(o.name)} = ${o.code},`).join('\n')}\n}\n#[derive(Clone,Copy,Debug,Eq,PartialEq)] #[repr(u16)] pub enum ErrorCode {\n${errors.map(e=>`    ${rustVariant(e.name.toLowerCase())} = ${e.code},`).join('\n')}\n}\npub const OPERATIONS:&[(&str,u16)]=&[${ops.map(o=>`("${o.name}",${o.code})`).join(',')}];\npub const ERRORS:&[(&str,u16,bool)]=&[${errors.map(e=>`("${e.name}",${e.code},${e.retryable})`).join(',')}];\npub const CLOSED_TYPES:&[(&str,&str)]=&[${defs.map(d=>`("${d.name}",${JSON.stringify(d.cddl)})`).join(',')}];\npub const SEMANTIC_TABLE_JSON:&str=${JSON.stringify(semanticJson)};\n`;
const runtimeTs=`${header}\n\n// Generated from the frozen cord.origin.host/2 CDDL and operation registry. Do not edit.\nexport const HOST_V2_PROTOCOL = 'cord.origin.host/2' as const;\nexport const HOST_V2_MAJOR = ${operationRegistry.major} as const;\nexport const HOST_V2_MINOR = ${operationRegistry.minor} as const;\nexport const HOST_V2_REGISTRY_SHA256 = '${sha(cddl)}' as const;\nexport const HOST_V2_JSON_PROJECTION_SHA256 = '${sha(projectionText)}' as const;\nexport const HOST_V2_SEMANTIC_SCHEMA_SHA256 = '${semanticHash}' as const;\nexport type HostV2OperationName = ${ops.map(operation=>JSON.stringify(operation.name)).join(' | ')};\nexport type HostV2ErrorName = ${errors.map(error=>JSON.stringify(error.name)).join(' | ')};\nexport type HostV2FeatureId = ${featureIds.map(value=>JSON.stringify(value)).join(' | ')};\nexport const HOST_V2_FEATURE_IDS: readonly HostV2FeatureId[] = ${JSON.stringify(featureIds)} as const;\nexport type HostV2TypeName = ${defs.map(definition=>JSON.stringify(definition.name)).join(' | ')};\nexport type HostV2SchemaNode = { readonly kind:'ref'; readonly name:HostV2TypeName } | { readonly kind:'union'; readonly variants:readonly HostV2SchemaNode[] } | { readonly kind:'map'; readonly fields:readonly {readonly key:number;readonly required:boolean;readonly schema:HostV2SchemaNode}[] } | {readonly kind:'array';readonly min:number;readonly max:number;readonly items:HostV2SchemaNode} | {readonly kind:'uint';readonly min:string;readonly max:string} | {readonly kind:'bytes'|'text';readonly min:number;readonly max:number;readonly nfc?:boolean} | {readonly kind:'bool'} | {readonly kind:'const';readonly value:number|string|boolean};\n${defs.map(definition=>`export type ${definition.name} = ${tsNode(cddlSchemas[definition.name])};`).join('\n')}\nexport interface HostV2TypeMap {\n${defs.map(definition=>`  readonly ${definition.name}: ${definition.name};`).join('\n')}\n}\nexport const HOST_V2_SCHEMAS:Readonly<Record<HostV2TypeName,HostV2SchemaNode>>=${JSON.stringify(cddlSchemas)} as const;\nexport const HOST_V2_CROSS_FIELD_RULES=${JSON.stringify(crossFieldRules)} as const;\nexport const HOST_V2_OPERATION_BINDINGS=${JSON.stringify(operationBindings,null,2)} as const;\nexport const HOST_V2_ERROR_BINDINGS=${JSON.stringify(Object.fromEntries(errors.map(error=>[error.code,{name:error.name,retryable:error.retryable}])),null,2)} as const;\nexport interface HostV2OperationTypeMap {\n${ops.map(operation=>`  readonly ${JSON.stringify(operation.name)}: { readonly code: ${operation.code}; readonly frame: ${operation.cddl.Request.replace(/Request$/,'Frame')}; readonly request: ${operation.cddl.Request}; readonly accepted: ${operation.cddl.Accepted}; readonly progress: ${operation.cddl.Progress}; readonly result: ${operation.cddl.Result}; readonly error: ${operation.cddl.Error} };`).join('\n')}\n}\n`;
const runtimeRust=`${header}\n\n// Generated from the frozen cord.origin.host/2 CDDL and operation registry. Do not edit.\npub(crate) const PROTOCOL: &str = "cord.origin.host/2";\npub(crate) const MAJOR: u8 = ${operationRegistry.major};\npub(crate) const MINOR: u16 = ${operationRegistry.minor};\npub(crate) const REGISTRY_SHA256: &str = "${sha(cddl)}";\npub(crate) const JSON_PROJECTION_SHA256: &str = "${sha(projectionText)}";\npub(crate) const SEMANTIC_SCHEMA_SHA256: &str = "${semanticHash}";\npub(crate) const SEMANTIC_TABLE_JSON: &str = ${JSON.stringify(semanticJson)};\npub(crate) const FEATURE_IDS: &[&str] = &[${featureIds.map(value=>JSON.stringify(value)).join(',')}];\n#[derive(Clone, Copy, Debug, Eq, PartialEq)] #[repr(u16)] pub(crate) enum OperationCode {\n${ops.map(operation=>`\t${rustVariant(operation.name)} = ${operation.code},`).join('\n')}\n}\nimpl OperationCode { pub(crate) fn from_u16(value: u16) -> Option<Self> { match value { ${ops.map(operation=>`${operation.code} => Some(Self::${rustVariant(operation.name)}),`).join(' ')} _ => None } } }\npub(crate) struct OperationBinding { pub(crate) name: &'static str, pub(crate) code: u16, pub(crate) feature_id: &'static str, pub(crate) grant_scope: &'static str, pub(crate) consent_mode: &'static str, pub(crate) state_changing: bool, pub(crate) operation_id_required: bool, pub(crate) frame: &'static str, pub(crate) request: &'static str, pub(crate) accepted: &'static str, pub(crate) progress: &'static str, pub(crate) result: &'static str, pub(crate) error: &'static str, pub(crate) allowed_errors: &'static [u16] }\npub(crate) const OPERATIONS: &[OperationBinding] = &[\n${ops.map(operation=>`\tOperationBinding { name: ${JSON.stringify(operation.name)}, code: ${operation.code}, feature_id: ${JSON.stringify(operation.feature_id)}, grant_scope: ${JSON.stringify(operation.grant_scope)}, consent_mode: ${JSON.stringify(operation.consent_mode)}, state_changing: ${operation.state_changing}, operation_id_required: ${operation.operation_id_required}, frame: ${JSON.stringify(operation.cddl.Request.replace(/Request$/,'Frame'))}, request: ${JSON.stringify(operation.cddl.Request)}, accepted: ${JSON.stringify(operation.cddl.Accepted)}, progress: ${JSON.stringify(operation.cddl.Progress)}, result: ${JSON.stringify(operation.cddl.Result)}, error: ${JSON.stringify(operation.cddl.Error)}, allowed_errors: &[${operation.allowed_errors.map(error=>error.code).join(',')}] },`).join('\n')}\n];\n#[derive(Clone, Copy, Debug, Eq, PartialEq)] #[repr(u16)] pub(crate) enum ErrorCode {\n${errors.map(error=>`\t${rustVariant(error.name.toLowerCase())} = ${error.code},`).join('\n')}\n}\nimpl ErrorCode { pub(crate) fn from_u16(value: u16) -> Option<Self> { match value { ${errors.map(error=>`${error.code} => Some(Self::${rustVariant(error.name.toLowerCase())}),`).join(' ')} _ => None } } }\npub(crate) struct ErrorBinding { pub(crate) name: &'static str, pub(crate) code: u16, pub(crate) retryable: bool }\npub(crate) const ERRORS: &[ErrorBinding] = &[\n${errors.map(error=>`\tErrorBinding { name: ${JSON.stringify(error.name)}, code: ${error.code}, retryable: ${error.retryable} },`).join('\n')}\n];\npub(crate) trait Production { const NAME: &'static str; }\n${defs.map(definition=>`pub(crate) enum ${definition.name} {} impl Production for ${definition.name} { const NAME: &'static str = ${JSON.stringify(definition.name)}; }`).join('\n')}\n`;
const outputs=[
  [path.join(generatedTsDir,'origin-host-registry-v2.ts'),ts],
  [path.join(generatedRustDir,'origin_host_registry_v2.rs'),rust],
  [path.join(runtimeTsDir,'generated.ts'),runtimeTs],
  [path.join(runtimeRustDir,'generated.rs'),runtimeRust],
];
const check=process.argv.includes('--check');let bad=0;for(const [file,content] of outputs){if(check){if(!fs.existsSync(file)||fs.readFileSync(file,'utf8')!==content){console.error('drift '+file);bad=1}}else fs.writeFileSync(file,content)}if(bad)process.exit(1);
console.log(JSON.stringify({status:'pass',registry_sha256:sha(cddl),json_projection_sha256:sha(projectionText),semantic_schema_sha256:semanticHash,types:defs.length,operations:ops.length,errors:errors.length,field_semantics_compared:defs.length,hostile_required_drift_rejected:true,mode:check?'check':'write'}));
