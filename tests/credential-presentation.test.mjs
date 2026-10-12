import assert from 'node:assert/strict';
import { test } from 'node:test';
import { traySummary } from '../src/features/credentials/data.ts';

test('单账号额度失败不能显示为整个管理连接失败', () => {
  const state = { connected: true, busy: false, stale: false, error: '部分额度失败', credentials: [{ quotaError: 'HTTP 401' }, { quotaError: null }] };
  assert.equal(traySummary(state), '2 个凭证 · 1 项额度异常');
  assert.equal(traySummary({ ...state, connected: false }), '管理连接失败');
  assert.equal(traySummary({ ...state, stale: true }), '列表待核实，请刷新');
  assert.equal(traySummary({ ...state, busy: true }), '正在刷新…');
});

test('新厂商紧凑额度保留真实周期、模型最低值和未知百分比', async () => {
  const { compactQuota, credentialQuotas } = await import('../src/features/credentials/data.ts');
  const item = (label, remaining, period) => ({ label, remaining, period, detail: '', reset: '' });
  const gemini = { provider:'gemini-cli', quotaItems:[item('Pro',80,'模型'),item('Flash',0,'模型'),item('Unknown',null,'模型')] };
  assert.equal(compactQuota(gemini).value,0);
  assert.equal(compactQuota(gemini).period,'最低');
  assert.match(compactQuota(gemini).description,/Unknown：百分比未提供/);
  assert.equal(compactQuota({provider:'xai',quotaItems:[item('月度',75,'月度')]}).period,'月度');
  assert.equal(compactQuota({provider:'codex',five:0,week:90,resetFive:'',resetWeek:''}).period,'5h');
  assert.equal(compactQuota({provider:'antigravity',quotaItems:[item('A 5h',80,'5h'),item('B 5h',90,'5h'),item('A 1w',0,'1w')]}).value,80);
  assert.equal(compactQuota({provider:'gemini',quotaItems:[{...item('请求',null,'模型'),detail:'剩余 12 请求'}]}).value,null);
  assert.equal(credentialQuotas({provider:'xai'})[0].label,'额度');
});

test('账号等级只展示白名单文案，批量请求只包含核实字段', async () => {
  const { planLabel, credentialTargets } = await import('../src/features/credentials/data.ts');
  assert.equal(planLabel('free'), 'Free');
  assert.equal(planLabel('pro'), 'Pro');
  assert.equal(planLabel('private-token'), '等级未知');
  assert.deepEqual(credentialTargets([{id:'a',enabled:true,deleteToken:'version',name:'private',quotaItems:[]}]), [{id:'a',enabled:true,deleteToken:'version'}]);
});
