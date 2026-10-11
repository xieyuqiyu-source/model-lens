import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { parseAppearance, resolveAppearance } from '../src/features/appearance/preferences.ts';
const themes = JSON.parse(readFileSync(new URL('../src/features/appearance/tokens.json', import.meta.url)));
function luminance(hex) {
  const rgb = hex.slice(1).match(/../g).map(c => parseInt(c, 16) / 255).map(c => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4);
  return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
}
function contrast(a, b) {
  const x = luminance(a), y = luminance(b);
  return (Math.max(x, y) + .05) / (Math.min(x, y) + .05);
}
test('外观偏好验证、损坏恢复及明暗解析', () => {
  assert.deepEqual(parseAppearance('{'), { mode: 'system', palette: 'jade' });
  assert.deepEqual(parseAppearance('{"mode":"invalid","palette":"invalid"}'), { mode: 'system', palette: 'jade' });
  assert.deepEqual(parseAppearance('{"mode":"dark","palette":"ocean"}'), { mode: 'dark', palette: 'ocean' });
  assert.equal(resolveAppearance('system', true), 'dark');
  assert.equal(resolveAppearance('system', false), 'light');
  assert.equal(resolveAppearance('light', true), 'light');
  assert.equal(resolveAppearance('dark', false), 'dark');
});
for (const [palette, modes] of Object.entries(themes)) for (const [mode, t] of Object.entries(modes)) {
  test(`${palette}/${mode} 文字、操作、额度与主题色的对比`, () => {
    for (const [fg, bg, min] of [
      ...['paper','surface','hover','track'].flatMap(bg => [['text',bg,4.5], ['muted',bg,4.5]]),
      ['on-accent','accent',4.5], ['on-accent','accent-hover',4.5],
      ['quota-text','green',4.5], ['quota-text','dark-green',4.5],
      ['green','track',3], ['dark-green','track',3], ['accent','paper',3], ['accent','surface',3],
      ['border','surface',3], ['border','paper',3], ['warning','paper',4.5], ['error','paper',4.5],
      ...['plan-free','plan-plus','plan-pro'].map(fg => [fg,'paper',4.5]),
    ]) assert.ok(contrast(t[fg],t[bg]) >= min, `${fg}/${bg}: ${contrast(t[fg],t[bg]).toFixed(2)} < ${min}`);
  });
}
