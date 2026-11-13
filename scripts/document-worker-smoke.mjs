import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { documentFixtures } from './document-fixtures.mjs';
await mkdir('test-results',{recursive:true});
const fixture=await mkdtemp(path.join(process.cwd(),'test-results','release-documents-'));
const files=await documentFixtures(fixture);
const executable=path.join(process.cwd(),'src-tauri/target/release/nabor.exe');
for(const[name,expected]of [['reference.pdf','Native PDF evidence'],['reference.docx','42 mm'],['reference.md','Native Markdown evidence'],['reference.txt','Native plain text evidence']]){
  const result=spawnSync(executable,['--extract-document',name.split('.').at(-1),files[name]],{windowsHide:true,encoding:'utf8',timeout:25000,maxBuffer:1000000});
  assert.equal(result.status,0,`${name}: ${result.error??result.stderr}`);assert(JSON.parse(result.stdout).Ok.includes(expected),name);
}
console.log(JSON.stringify({result:'passed',releaseHelper:true,formats:['pdf','docx','md','txt'],fixture}));
