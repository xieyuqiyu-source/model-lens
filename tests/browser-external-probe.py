"""pnpm dev 后运行；第三方 API 由合成 IPC 替身返回，不访问真实服务。"""
from playwright.sync_api import sync_playwright, expect
import os, re
BASE_URL = os.environ.get("MODELLENS_TEST_URL", "http://127.0.0.1:1420")
MOCK = r"""
window.isTauri = true;
window.calls = [];
window.ext = JSON.parse(sessionStorage.getItem('ext') || '{"saved":false,"baseUrl":""}');
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
  if(cmd === 'external_status') return structuredClone(ext);
  if(cmd === 'external_save') {
   if(args.key === 'bad') throw '第三方 API 认证失败，请检查 API Key';
   ext = {saved:true, baseUrl: args.baseUrl.replace(/\/$/,'')}; sessionStorage.setItem('ext', JSON.stringify(ext));
   return [{id:'relay-gpt',name:'relay-gpt',brand:'openai'}];
  }
  if(cmd === 'external_forget') { ext = {saved:false, baseUrl:''}; sessionStorage.setItem('ext', JSON.stringify(ext)); return null; }
  if(cmd === 'external_models') return [{id:'relay-gpt',name:'relay-gpt',brand:'openai'},{id:'relay-claude',name:'relay-claude',brand:'claude'}];
  if(cmd === 'external_sample') {
   const n = calls.filter(c=>c.cmd==='external_sample').length-1;
   return {text:Array.from({length:300},(_,i)=>(i*(17+n*6)+n*41)%355+1).join(', '),finish:'stop',reportedModel:'relay-returned',elapsedMs:80};
  }
  if(cmd === 'probe_models' || cmd === 'probe_sample') throw 'CLIProxy 不应被调用';
  return null;
 }
};
"""
with sync_playwright() as p:
 for engine in [p.webkit, p.chromium]:
  browser = engine.launch(headless=True)
  page = browser.new_page(viewport={'width':1120,'height':760})
  errors = []
  page.on('pageerror', lambda e: errors.append(str(e)))
  page.add_init_script(MOCK)
  page.goto(BASE_URL)
  start = page.get_by_role('button', name='开始检测', exact=True)
  expect(start).to_be_disabled()
  page.get_by_role('button', name='第三方 API', exact=True).click()
  expect(page.get_by_text('填写第三方 API 后可探查')).to_be_visible()
  page.get_by_role('button', name='填写 Base URL 与 Key', exact=True).click()
  dialog = page.get_by_role('dialog')
  dialog.get_by_label('Base URL').fill('https://relay.example.com/v1/')
  dialog.get_by_label('API Key').fill('bad')
  dialog.get_by_role('button', name='保存并读取模型').click()
  expect(dialog.get_by_role('alert')).to_contain_text('认证失败')
  dialog.get_by_label('API Key').fill('sk-synthetic')
  dialog.get_by_role('button', name='保存并读取模型').click()
  expect(page.get_by_role('dialog')).to_have_count(0)
  expect(page.get_by_role('button', name='https://relay.example.com/v1', exact=True)).to_be_visible()
  expect(start).to_be_enabled()
  # The key never comes back to the page after saving.
  assert 'sk-synthetic' not in page.content()
  start.click()
  result = page.get_by_role('article', name='检测结果')
  expect(result).to_contain_text('3 / 3 有效样本', timeout=30000)
  expect(result).to_contain_text('relay.example.com')
  samples = page.evaluate("calls.filter(c=>c.cmd==='external_sample')")
  assert len(samples) == 3 and all(c['args']['model'] == 'relay-gpt' for c in samples), samples
  assert not page.evaluate("calls.some(c=>c.cmd==='probe_sample'||c.cmd==='probe_models')")
  page.screenshot(path=f'/tmp/modellens-external-{engine.name}.png')
  # Reload keeps the chosen source; forgetting returns to empty state.
  page.reload()
  expect(page.get_by_role('button', name='https://relay.example.com/v1', exact=True)).to_be_visible()
  page.get_by_role('button', name='https://relay.example.com/v1', exact=True).click()
  page.get_by_role('dialog').get_by_role('button', name='忘记 Key').click()
  expect(page.get_by_role('button', name='填写 Base URL 与 Key', exact=True)).to_be_visible()
  expect(start).to_be_disabled()
  page.get_by_role('button', name='CLIProxy', exact=True).click()
  expect(page.get_by_text('连接管理接口后可自动探查')).to_be_visible()
  assert not errors, errors
  print(f'{engine.name}: 第三方 API 保存/错误/采样/记住来源/忘记通过', flush=True)
  browser.close()
