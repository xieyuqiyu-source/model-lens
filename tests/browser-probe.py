"""pnpm dev 后运行；全部采样由合成 IPC 替身返回，不访问真实服务。"""
from playwright.sync_api import sync_playwright, expect
import re
import os
import json
from pathlib import Path
BASE_URL = os.environ.get("MODELLENS_TEST_URL", "http://127.0.0.1:1420")
CSP = json.loads(Path("src-tauri/tauri.conf.json").read_text())["app"]["security"]["csp"]
MOCK = r"""
window.isTauri = true;
window.mockCalls = [];
window.mockMode = 'ok';
window.mockDelay = 60;
window.mockCatalogCalls = [];
window.mockCatalog = [
 {id:'test-a',name:'GPT Test',brand:'openai'},
 {id:'test-b',name:'Claude Test',brand:'claude'},
 {id:'test-c',name:'Gemini Test',brand:'gemini'},
 {id:'team/alias',name:'Custom alias',brand:'unknown'},
];
const callbacks = new Map(); let nextCallback = 0;
window.mockSnapshot = {connected:true,saved:true,port:8317,busy:false,stale:false,error:null,revision:1,credentials:[0,1].map(i=>({id:`mock-${i}`,name:`sample-${i}@example.invalid`,brand:i?'gemini':'openai',provider:i?'gemini':'codex',enabled:true,editable:true,deletable:true,fileName:`mock-${i}.json`,deleteToken:`mock-${i}`,supported:!i,five:null,week:null,resetFive:'',resetWeek:'',quotaAt:null,quotaError:null}))};
window.mockPublish = () => { mockSnapshot.revision++; for(const cb of callbacks.values()) cb({event:'credentials-changed',payload:structuredClone(mockSnapshot)}); };
localStorage.setItem('mode','manual'); // Old preference must not restore the removed UI.

window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener() {}};
window.__TAURI_INTERNALS__ = {
 transformCallback(cb) { callbacks.set(++nextCallback,cb); return nextCallback; }, unregisterCallback(id) {callbacks.delete(id);},
 async invoke(cmd, args) {
  if(cmd === 'plugin:event|listen') return 1;
  if(cmd === 'discover_services') return {services:[],checkedAt:1,statusKnown:false,warning:null};
  if(cmd === 'management_snapshot') return structuredClone(mockSnapshot);
  if(cmd === 'probe_models') {
   mockCatalogCalls.push(args.id);
   if(window.mockCatalogError) throw '模拟模型刷新失败';
   return args.id ? mockCatalog.filter(m=>m.id==='test-c') : structuredClone(mockCatalog);
  }
  if(cmd === 'probe_sample') {
   window.mockCalls.push(args);
   const n=window.mockCalls.length-1;
   await new Promise(resolve=>setTimeout(resolve,window.mockDelay));
   if(window.mockMode === 'fail-second' && n % 3 === 1) throw '模拟额度错误';
   return {text:Array.from({length:300},(_,i)=>(i*(17+(n%3)*6)+(n%3)*41)%355+1).join(', '),finish:'stop',reportedModel:'mock-returned',elapsedMs:100};
  }
 }
};
"""
with sync_playwright() as p:
 for engine in [p.webkit,p.chromium]:
  browser=engine.launch(headless=True)
  page=browser.new_page(viewport={'width':1120,'height':760})
  errors=[]
  page.on('pageerror',lambda e:errors.append(str(e)))
  if os.environ.get("MODELLENS_TEST_CSP"):
   def document_with_csp(route):
    response=route.fetch()
    route.fulfill(response=response,headers={**response.headers,'content-security-policy':CSP})
   page.route(BASE_URL+'/',document_with_csp)
  page.add_init_script(MOCK)
  page.goto(BASE_URL)
  start=page.get_by_role('button',name='开始检测',exact=True)
  expect(start).to_be_enabled()
  assert page.evaluate('mockCalls.length')==0
  expect(page.get_by_role('button',name='手动检测',exact=True)).to_have_count(0)
  expect(page.get_by_role('textbox',name=re.compile('模型回答'))).to_have_count(0)
  page.get_by_role('button',name='选择模型：GPT Test',exact=True).click()
  expect(page.get_by_role('option')).to_have_count(4)
  page.get_by_role('option',name=re.compile('Claude Test')).click()
  page.evaluate("mockCatalog.push({id:'new-model',name:'New model',brand:'unknown'})")
  page.get_by_role('button',name='刷新模型',exact=True).click()
  expect(start).to_be_enabled()
  page.get_by_role('button',name='选择模型：Claude Test',exact=True).click()
  expect(page.get_by_role('option')).to_have_count(5)
  page.get_by_role('option',name=re.compile('New model')).click()
  page.evaluate("mockCatalogError=true")
  page.get_by_role('button',name='刷新模型',exact=True).click()
  expect(page.get_by_text('模拟模型刷新失败',exact=True)).to_be_visible()
  expect(start).to_be_disabled()
  page.evaluate("mockCatalogError=false")
  page.get_by_role('button',name='刷新模型',exact=True).click()
  expect(start).to_be_enabled()
  page.get_by_role('button',name='凭证优先',exact=True).click()
  for width in [920,1120,1280]:
   page.set_viewport_size({'width':width,'height':760})
   expect(page.locator('.probe.compact')).to_be_visible()
   expect(page.get_by_role('button',name=re.compile('选择模型范围：'))).to_be_visible()
   page.evaluate("async () => { await Promise.all(document.getAnimations().filter(a => a.effect?.getTiming().iterations !== Infinity).map(a => a.finished.catch(() => {}))); }")
   boxes=page.locator('.probe.compact').evaluate("e=>{const a=e.querySelector('.start').getBoundingClientRect(),b=e.querySelector(':scope > .probe-status').getBoundingClientRect();return {bottom:a.bottom,top:b.top,scroll:e.scrollWidth,width:e.clientWidth}}")
   assert boxes['top'] >= boxes['bottom']+8, boxes
   assert boxes['scroll'] <= boxes['width']+1, boxes
  page.screenshot(path=f'/tmp/modellens-probe-compact-{engine.name}.png')
  card=page.locator('.credential-card').filter(has_text='sample-1@example.invalid')
  card.get_by_role('button',name='探查模型',exact=True).click()
  expect(page.get_by_role('button',name=re.compile('选择模型范围：sample-1'))).to_be_visible()
  expect(page.get_by_role('button',name='选择模型：Gemini Test',exact=True)).to_be_visible()
  assert page.evaluate('mockCalls.length')==0
  expect(start).to_be_enabled()
  start.click()
  result=page.get_by_role('article',name='检测结果')
  expect(result).to_contain_text('3 / 3 有效样本',timeout=30000)
  expect(result).to_contain_text('库内置信度')
  calls=page.evaluate('mockCalls')
  assert len(calls)==3 and all(c['id']=='mock-1' and c['model']=='test-c' for c in calls)
  page.get_by_role('button',name='原始回答与采样详情',exact=True).click()
  expect(page.get_by_text('服务返回型号：mock-returned',exact=True)).to_have_count(3)
  page.get_by_role('button',name='原始回答与采样详情',exact=True).click()
  page.get_by_role('button',name='历史',exact=True).click()
  expect(page.get_by_role('dialog')).to_contain_text('3/3')
  page.get_by_role('dialog').get_by_role('button',name='关闭',exact=True).click()
  page.evaluate("mockCalls=[];mockMode='fail-second'")
  start.click()
  expect(result).to_contain_text('1 / 3 有效样本',timeout=30000)
  expect(result).not_to_contain_text('库内置信度')
  expect(result).to_contain_text('模拟额度错误')
  assert page.evaluate('mockCalls.length')==2
  page.evaluate("mockCalls=[];mockMode='ok';mockDelay=1500")
  start.click()
  expect(page.get_by_role('button',name='停止',exact=True)).to_be_visible()
  page.get_by_role('button',name='停止',exact=True).click()
  expect(page.get_by_role('alert')).to_contain_text('已停止',timeout=5000)
  assert page.evaluate('mockCalls.length')==1
  # Disabling the chosen scope returns to the complete live model list.
  page.evaluate("mockSnapshot.credentials[1].enabled=false; mockPublish()")
  expect(page.get_by_role('button',name='选择模型范围：全部已启用凭证',exact=True)).to_be_visible()
  expect(start).to_be_enabled()
  assert page.evaluate('mockCatalogCalls.at(-1)')==''
  assert page.evaluate('mockCatalogCalls.length') < 12
  page.screenshot(path=f'/tmp/modellens-probe-{engine.name}.png')
  page.get_by_role('button',name='检测提示词',exact=True).click()
  page.get_by_role('button',name='新一组提示词',exact=True).click()
  expect(start).to_be_enabled()
  expect(result).to_have_count(0)
  assert not errors,errors
  print(f'{engine.name}: 全厂商目录、刷新/错误、范围更新、无手动入口、窄栏无重叠、三份采样/部分/取消/历史通过',flush=True)
  browser.close()
