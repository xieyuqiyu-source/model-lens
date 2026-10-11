import assert from 'node:assert/strict';
import { test } from 'node:test';
import { dismissSurface } from '../src/components/motion.ts';

test('减少动态效果和已卸载弹层立即关闭，不创建动画', () => {
  globalThis.window = { matchMedia: () => ({ matches: true }) };
  let calls = 0;
  const element = { animate() { throw new Error('不应创建动画'); } };
  assert.equal(dismissSurface(element, () => calls++), null);
  assert.equal(dismissSurface(null, () => calls++), null);
  assert.equal(calls, 2);
});

test('关闭动画完成后才移除弹层；取消动画不触发过期回调', async () => {
  globalThis.window = { matchMedia: () => ({ matches: false }) };
  let finish, cancel, calls = 0;
  const animation = { finished: new Promise((resolve, reject) => { finish = resolve; cancel = reject; }) };
  const element = { animate: () => animation };
  assert.equal(dismissSurface(element, () => calls++), animation);
  assert.equal(calls, 0);
  finish();
  await animation.finished;
  assert.equal(calls, 1);
  animation.finished = new Promise((resolve, reject) => { cancel = reject; });
  dismissSurface(element, () => calls++);
  cancel(new Error('组件已卸载'));
  await animation.finished.catch(() => {});
  assert.equal(calls, 1);
});
