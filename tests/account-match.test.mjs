import assert from 'node:assert/strict';
import { test } from 'node:test';
import { accountVerdict } from '../src/features/probe/accountMatch.ts';
const result = (prediction='gpt-5.4', probability=.92) => ({analysis:{prediction,prediction_name:prediction,probability,probability_status:'reference_calibrated',used_outputs:3,results:[{model:'gpt-5.4'},{model:'gpt-5.5'},{model:'gpt-6-astra'},{model:'claude-sonnet-4.6'},{model:'gemini-3.8-flash'}]}});
test('账号型号匹配仅在完整校准且证据足够时着色', () => {
 assert.equal(accountVerdict('gpt-5.4',result()).status,'match');
 assert.equal(accountVerdict('gpt-5.5',result()).status,'mismatch');
 assert.equal(accountVerdict('unknown-alias',result()).status,'unknown');
 assert.equal(accountVerdict('gpt-5.4',result('gpt-5.4',.5)).status,'unknown');
 for (const patch of [{used_outputs:2},{probability_status:'unavailable'},{probability:null}]) {
  const r=result();Object.assign(r.analysis,patch);assert.equal(accountVerdict('gpt-5.4',r).status,'unknown');
 }
});
test('只归一化明确的型号写法，不猜测自定义路由别名', () => {
 assert.equal(accountVerdict('claude-sonnet-4-6',result('claude-sonnet-4.6')).status,'match');
 assert.equal(accountVerdict('gemini-3.8-flash-high',result('gemini-3.8-flash')).status,'match');
 assert.equal(accountVerdict('gpt-6.1-sol',result('gpt-6-astra')).status,'match');
 assert.equal(accountVerdict('team/gpt-5.4',result()).status,'unknown');
 assert.equal(accountVerdict('gpt-54',result()).status,'unknown');
});
