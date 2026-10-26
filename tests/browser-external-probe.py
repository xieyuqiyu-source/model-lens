"""pnpm dev 后运行；多个第三方 API 由合成 IPC 替身返回，不访问真实服务。"""
from playwright.sync_api import sync_playwright, expect
import os, re
BASE_URL = os.environ.get("MODELLENS_TEST_URL", "http://127.0.0.1:1420")
MOCK = r"""
window.isTauri = true;
window.calls = [];
window.ext = JSON.parse(sessionStorage.getItem('ext') || '[]');
const persist = () => sessionStorage.setItem('ext', JSON.stringify(ext));
const callbacks = new Map(); let next = 0;
const snapshot = {connected:false,saved:false,port:8317,busy:false,stale:false,error:null,revision:1,credentials:[]};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener() {}};
window.__TAURI_INTERNALS__ = {
 transformCallback(cb) { callbacks.set(++next,cb); return next; }, unregisterCallback(id) {callbacks.delete(id);},
 async invoke(cmd, args) {
  calls.push({cmd, args});
  if(cmd === 'plugin:event|listen') return 1;
  if(cmd === 'discover_services') return {services:[],checkedAt:1,statusKnown:false,warning:null};
  if(cmd === 'management_snapshot') return structuredClone(snapshot);
  if(cmd === 'external_list') return ext.map(({key,...e})=>e);
  if(cmd === 'external_save') {
   if(args.key === 'bad') throw '第三方 API 认证失败，请检查 API Key';
   const baseUrl = args.baseUrl.replace(/\/$/,'');
   const name = args.name.trim() || new URL(baseUrl).host;
   if(args.id) { const e = ext.find(e=>e.id===args.id); Object.assign(e, {name, baseUrl}, args.key ? {key:args.key} : {}); }
   else ext.push({id:'id-'+ext.length+'-'+Date.now(), name, baseUrl, key:args.key});
   persist();
   const id = args.id || ext.at(-1).id;
   return {id, models:[{id:'m',name:'m',brand:'openai'}]};
  }
  if(cmd === 'external_forget') { ext = ext.filter(e=>e.id!==args.id); persist(); return null; }
  if(cmd === 'external_models') { const e = ext.find(e=>e.id===args.id); return [{id:e.name+'-gpt',name:e.name+'-gpt',brand:'openai'}]; }
  if(cmd === 'external_sample') {
   const n = calls.filter(c=>c.cmd==='external_sample').length-1;
   return {text:Array.from({length:300},(_,i)=>(i*(17+(n%3)*6)+(n%3)*41)%355+1).join(', '),finish:'stop',reportedModel:'relay-returned',elapsedMs:80};
  }
  if(cmd === 'probe_models' || cmd === 'probe_sample') throw 'CLIProxy 不应被调用';
  return null;
 }
};
"""
def add(page, name, url, key):
  dialog = page.get_by_role('dialog')
  dialog.get_by_label('名称').fill(name)
  dialog.get_by_label('Base URL').fill(url)
  dialog.get_by_label('API Key').fill(key)
  dialog.get_by_role('button', name='保存并读取模型').click()
with sync_playwright() as p:
 for engine in [p.webkit, p.chromium]:
  browser = engine.launch(headless=True)
  page = browser.new_page(viewport={'width':1120,'height':760})
  errors = []
  page.on('pageerror', lambda e: errors.append(str(e)))
  page.add_init_script(MOCK)
  page.goto(BASE_URL)
  start = page.get_by_role('button', name='开始检测', exact=True)
  page.get_by_role('button', name='第三方 API', exact=True).click()
  expect(page.get_by_text('添加第三方 API 后可探查')).to_be_visible()
  expect(start).to_be_disabled()
  page.get_by_role('button', name='添加 Base URL 与 Key', exact=True).click()
  add(page, '', 'https://relay-a.example.com/v1/', 'bad')
  expect(page.get_by_role('dialog').get_by_role('alert')).to_contain_text('认证失败')
  add(page, '', 'https://relay-a.example.com/v1/', 'sk-a')
  expect(page.get_by_role('dialog')).to_have_count(0)
  picker = page.get_by_role('button', name=re.compile('^选择第三方 API：'))
  expect(picker).to_have_accessible_name('选择第三方 API：relay-a.example.com')
  # Add a second entry through the dropdown.
  picker.click()
  page.get_by_role('option', name=re.compile('添加第三方 API')).click()
  add(page, 'Relay B', 'https://b.example.com/v1', 'sk-b')
  expect(page.get_by_role('dialog')).to_have_count(0)
  expect(picker).to_have_accessible_name('选择第三方 API：Relay B')
  expect(page.get_by_role('button', name='选择模型：Relay B-gpt', exact=True)).to_be_visible()
  assert 'sk-a' not in page.content() and 'sk-b' not in page.content()
  # Switch back to A; models follow the selected entry.
  picker.click()
  expect(page.get_by_role('option')).to_have_count(3)
  page.get_by_role('option', name=re.compile('relay-a')).click()
  expect(page.get_by_role('button', name='选择模型：relay-a.example.com-gpt', exact=True)).to_be_visible()
  start.click()
  result = page.get_by_role('article', name='检测结果')
  expect(result).to_contain_text('3 / 3 有效样本', timeout=30000)
  expect(result).to_contain_text('第三方 API · relay-a.example.com')
  samples = page.evaluate("calls.filter(c=>c.cmd==='external_sample').map(c=>c.args)")
  ida = page.evaluate("ext[0].id")
  assert len(samples) == 3 and all(c['id'] == ida and c['model'] == 'relay-a.example.com-gpt' for c in samples), samples
  assert not page.evaluate("calls.some(c=>c.cmd==='probe_sample'||c.cmd==='probe_models')")
  page.screenshot(path=f'/tmp/modellens-external-{engine.name}.png')
  # Edit A with empty key keeps its key; rename shows in picker.
  page.get_by_role('button', name='编辑第三方 API', exact=True).click()
  dialog = page.get_by_role('dialog')
  expect(dialog.get_by_label('Base URL')).to_have_value('https://relay-a.example.com/v1')
  dialog.get_by_label('名称').fill('Relay A')
  dialog.get_by_role('button', name='保存并读取模型').click()
  expect(picker).to_have_accessible_name('选择第三方 API：Relay A')
  assert page.evaluate("ext[0].key") == 'sk-a'
  # Reload remembers source and selection.
  page.reload()
  expect(picker).to_have_accessible_name('选择第三方 API：Relay A')
  # Delete A; B remains and gets selected.
  page.get_by_role('button', name='编辑第三方 API', exact=True).click()
  page.get_by_role('dialog').get_by_role('button', name='删除此 API').click()
  expect(picker).to_have_accessible_name('选择第三方 API：Relay B')
  page.get_by_role('button', name='编辑第三方 API', exact=True).click()
  page.get_by_role('dialog').get_by_role('button', name='删除此 API').click()
  expect(page.get_by_role('button', name='添加 Base URL 与 Key', exact=True)).to_be_visible()
  expect(start).to_be_disabled()
  page.get_by_role('button', name='CLIProxy', exact=True).click()
  expect(page.get_by_text('连接管理接口后可自动探查')).to_be_visible()
  assert not errors, errors
  print(f'{engine.name}: 多个第三方 API 添加/切换/编辑沿用 Key/记住选择/删除/采样通过', flush=True)
  browser.close()
