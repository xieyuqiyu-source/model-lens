import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createWorkspaceMotion } from '../src/components/workspaceMotion.ts';
function fixture() {
  const classes = new Set();
  const captures = [];
  const host = {
    documentElement: { classList: { add: x => classes.add(x), remove: x => classes.delete(x) } },
    startViewTransition(update) {
      let finish, rejectReady;
      const item = { update, skipped: false,
        ready: new Promise((_, reject) => { rejectReady = reject; }),
        finished: new Promise(resolve => { finish = resolve; }),
        skipTransition() { this.skipped = true; rejectReady(new Error('superseded')); },
        finish() { finish(); },
      };
      captures.push(item);
      return item;
    },
  };
  return { host, classes, captures, motion: createWorkspaceMotion(host) };
}
test('快速反向切换只提交最新布局；过期动画完成不会清除新动画状态', async () => {
  const { motion, captures, classes } = fixture();
  const committed = [];
  motion.run(() => committed.push('credentials'));
  motion.run(() => committed.push('probe'));
  assert.equal(captures[0].skipped, true);
  captures[0].update();
  captures[1].update();
  assert.deepEqual(committed, ['probe']);
  captures[0].finish();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(classes.has('workspace-transitioning'), true);
  captures[1].finish();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(classes.size, 0);
});
test('卸载时取消动画，延迟捕获不会提交状态', () => {
  const { motion, captures, classes } = fixture();
  motion.run(() => assert.fail('不应提交已卸载状态'));
  motion.cancel();
  captures[0].update();
  assert.equal(captures[0].skipped, true);
  assert.equal(classes.size, 0);
});
test('旧 WebView 不逐帧缩放布局，更新后播放淡入', () => {
  const { host } = fixture();
  const steps = [];
  host.startViewTransition = undefined;
  host.querySelector = () => ({ animate: () => steps.push('animate') });
  createWorkspaceMotion(host).run(() => steps.push('update'));
  assert.deepEqual(steps, ['update', 'animate']);
});
