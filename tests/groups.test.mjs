import assert from 'node:assert/strict';
import { test } from 'node:test';
import { credentialGroup, inGroup } from '../src/features/credentials/groups.ts';
test('凭证分组归一化别名，保留 Antigravity 独立和未知类型', () => {
  for (const [input, output] of [['codex','codex'],['gemini-cli','gemini'],['gemini','gemini'],['xai','grok'],['grok','grok'],['claude','claude'],['antigravity','antigravity'],['unknown','other'],['all','other']])
    assert.equal(credentialGroup(input),output);
});
test('分组只筛选并保持原顺序，不按开关重排', () => {
  const rows=[{id:'a',provider:'codex',enabled:false},{id:'b',provider:'claude',enabled:true},{id:'c',provider:'codex',enabled:true}];
  assert.deepEqual(inGroup(rows,'codex').map(r=>r.id),['a','c']);
  rows[0].enabled=true;rows[2].enabled=false;
  assert.deepEqual(inGroup(rows,'codex').map(r=>r.id),['a','c']);
  assert.equal(inGroup(rows,'all'),rows);
  assert.deepEqual(inGroup(rows,'gemini'),[]);
});
