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
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener() {}};
window.__TAURI_INTERNALS__ = {
 transformCallback() { return 1; }, unregisterCallback() {},
 async invoke(cmd, args) {
  if(cmd === 'plugin:event|listen') return 1;
  if(cmd === 'discover_services') return {services:[],checkedAt:1,statusKnown:false,warning:null};
  if(cmd === 'management_snapshot') return {connected:true,saved:true,port:8317,busy:false,stale:false,error:null,revision:1,credentials:[0,1].map(i=>({id:`mock-${i}`,name:`sample-${i}@example.invalid`,brand:'openai',provider:'codex',enabled:true,editable:true,deletable:true,fileName:`mock-${i}.json`,deleteToken:`mock-${i}`,supported:true,probeSupported:true,five:null,week:null,resetFive:'',resetWeek:'',quotaAt:null,quotaError:null}))};
  if(cmd === 'probe_models') return [{id:'test-a',name:'Test A',effort:null},{id:'test-b',name:'Test B',effort:null}];
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
  page.get_by_role('button',name='凭证优先',exact=True).click()
  card=page.locator('.credential-card').filter(has_text='sample-1@example.invalid')
  card.get_by_role('button',name='单独检测',exact=True).click()
  expect(page.get_by_role('button',name=re.compile('选择探查凭证：sample-1'))).to_be_visible()
  assert page.evaluate('mockCalls.length')==0
  page.get_by_role('button',name='选择模型：Test A',exact=True).click()
  page.get_by_role('option',name=re.compile('Test B')).click()
  page.get_by_role('button',name=re.compile('选择探查凭证：')).click()
  page.get_by_role('option',name=re.compile('sample-1')).click()
  expect(start).to_be_enabled()
  start.click()
  result=page.get_by_role('article',name='检测结果')
  expect(result).to_contain_text('3 / 3 有效样本',timeout=30000)
  expect(result).to_contain_text('库内置信度')
  calls=page.evaluate('mockCalls')
  assert len(calls)==3 and all(c['id']=='mock-1' and c['model']=='test-b' for c in calls)
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
  page.get_by_role('button',name='手动检测',exact=True).click()
  answer=page.get_by_role('textbox',name='模型回答 1',exact=True)
  expect(answer).to_have_attribute('rows','2')
  for i in range(3):
   page.get_by_role('button',name=f'回答 {i+1}',exact=True).click()
   page.get_by_role('textbox',name=f'模型回答 {i+1}',exact=True).fill(', '.join(str((j*(17+i*6)+i*41)%355+1) for j in range(300)))
  page.get_by_role('button',name='分析输入',exact=True).click()
  expect(result).to_contain_text('3 / 3 有效样本',timeout=30000)
  assert page.evaluate('mockCalls.length')==1
  page.screenshot(path=f'/tmp/modellens-probe-{engine.name}.png')
  page.get_by_role('button',name='检测提示词',exact=True).click()
  page.get_by_role('button',name='新一组提示词',exact=True).click()
  expect(page.get_by_role('button',name='分析输入',exact=True)).to_be_disabled()
  expect(result).to_have_count(0)
  assert not errors,errors
  print(f'{engine.name}: 自动三份、部分结果、取消、手动三份、历史、重置与 worker 通过',flush=True)
  browser.close()
