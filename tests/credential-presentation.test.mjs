import assert from 'node:assert/strict';
import { test } from 'node:test';
import { activeCredentialsFirst, traySummary } from '../src/features/credentials/data.ts';

test('单账号额度失败不能显示为整个管理连接失败', () => {
  const state = { connected: true, busy: false, stale: false, error: '部分额度失败', credentials: [{ quotaError: 'HTTP 401' }, { quotaError: null }] };
  assert.equal(traySummary(state), '2 个凭证 · 1 项额度异常');
  assert.equal(traySummary({ ...state, connected: false }), '管理连接失败');
  assert.equal(traySummary({ ...state, stale: true }), '列表待核实，请刷新');
  assert.equal(traySummary({ ...state, busy: true }), '正在刷新…');
});
test('紧凑列表优先显示启用账号，保留分组内顺序，不改动原列表', () => {
  const rows = [{ id: 'off', enabled: false }, { id: 'on-a', enabled: true }, { id: 'on-b', enabled: true }];
  assert.deepEqual(activeCredentialsFirst(rows).map(row => row.id), ['on-a', 'on-b', 'off']);
  assert.equal(rows[0].id, 'off');
});
