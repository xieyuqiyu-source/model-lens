import assert from 'node:assert/strict';
import { test } from 'node:test';
import { dismissSurface } from '../src/components/motion.ts';

test('已卸载弹层立即完成，不创建动画', () => {
  let calls = 0;
  assert.equal(dismissSurface(null, () => calls++), null);
  assert.equal(calls, 1);
});

for (const reduced of [true, false]) {
  test(`系统减少动态效果=${reduced} 时仍等待关闭动画；取消不触发过期回调`, async () => {
    globalThis.window = { matchMedia: () => ({ matches: reduced }) };
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
}
