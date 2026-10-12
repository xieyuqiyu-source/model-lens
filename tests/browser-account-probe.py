"""Synthetic IPC/worker outcomes: no real credentials or paid model requests."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

url = os.environ.get('MODEL_LENS_TEST_URL', 'http://127.0.0.1:1420')
fixture = Path(__file__).parent / 'fixtures/credentials-ipc.js'
setup = r"""
localStorage.setItem('focus','credentials');
__testSnapshot.credentials=['one','two','three','four'].map(id=>({...row(id,true),five:80,week:70}));
__testSnapshot.accountTests={};__testSnapshot.accountHistory=[];
window.accountMode='match';window.pendingJobs=[];let nextRun=0;
const invoke=__TAURI_INTERNALS__.invoke;
__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
 if(cmd==='probe_models') return [{id:'gpt-5.4',name:'GPT-5.4',brand:'openai'},{id:'gpt-5.3',name:'GPT-5.3',brand:'openai'}];
 if(cmd==='probe_account_samples'){
  __testCalls.push({cmd,args});
  const mode=accountMode,runId=++nextRun;
  const test={runId,credentialId:args.id,credential:__testSnapshot.credentials.find(c=>c.id===args.id).name,
   model:args.model,status:'running',message:'采样中',at:Date.now()/1000};
  __testSnapshot.accountTests[args.id]=test;
  const entry={...test,samples:[]};__testSnapshot.accountHistory.unshift(entry);__testPublish();
  await new Promise(resolve=>pendingJobs.push(resolve));
  const error=mode==='error'?'模拟采样失败':null;
  const samples=error?[]:args.challenges.map((c,i)=>({...c,slot:i+1,text:mode,finish:'stop',reportedModel:'ignored',elapsedMs:1}));
  Object.assign(test,{status:error?'error':'analyzing',message:error||'分析中'});
  Object.assign(entry,test,{samples});__testPublish();
  return {runId,samples,error};
 }
 if(cmd==='probe_account_finish'){
  __testCalls.push({cmd,args});
  const t=__testSnapshot.accountTests[args.id];
  Object.assign(t,{status:args.status,message:args.message});
  Object.assign(__testSnapshot.accountHistory.find(h=>h.runId===args.runId),t);__testPublish();return;
 }
 return invoke(cmd,args);
};
// Only visual state is stubbed here; kernel and HTTP concurrency have separate tests.
window.Worker=class{
 postMessage(samples){this.timer=setTimeout(()=>{const mode=samples[0].text;this.onmessage({data:{result:{
  samples,warnings:[],analysis:{results:[{model:'gpt-5.4',display_name:'GPT-5.4',probability:.99},{model:'gpt-5.3',display_name:'GPT-5.3',probability:.01}],
   evidence:{label:'参考库比对',reason:'合成界面测试'},used_outputs:3,probability_status:'reference_calibrated',
   probability:mode==='unknown'?.4:.99,prediction:mode==='mismatch'?'gpt-5.3':'gpt-5.4',prediction_name:'Synthetic'}
 }}})},30)}
 terminate(){clearTimeout(this.timer)}
};
"""
with sync_playwright() as p:
 for engine in [p.webkit,p.chromium]:
  browser=engine.launch(headless=True)
  context=browser.new_context(viewport={'width':1120,'height':760})
  context.add_init_script(script=fixture.read_text()+'\n'+setup)
  page=context.new_page();errors=[]
  page.on('pageerror',lambda e:errors.append(str(e)))
  page.goto(url)
  for name in ['one','two','three']:
   page.get_by_role('button',name=f'测试 {name}@example.invalid',exact=True).click()
  fourth=page.get_by_role('button',name='测试 four@example.invalid',exact=True)
  expect(fourth).to_be_disabled()
  assert page.evaluate('pendingJobs.length===3 && !__testSnapshot.busy')
  page.get_by_role('button',name='采样中 one@example.invalid，查看详情',exact=True).click()
  expect(page.get_by_text('可关闭此面板，探查继续在后台运行。')).to_be_visible()
  page.get_by_role('button',name='关闭',exact=True).click()
  page.get_by_role('button',name='探查优先',exact=True).click()
  page.evaluate('pendingJobs.shift()()')
  expect(page.get_by_role('button',name='库内匹配 one@example.invalid，查看详情',exact=True)).to_be_visible()
  expect(fourth).to_be_enabled();fourth.click()
  page.evaluate('pendingJobs.splice(0).forEach(f=>f())')
  page.wait_for_function("Object.values(__testSnapshot.accountTests).every(t=>t.status==='match')")
  # Completion feedback is actionable; raw samples and analysis live in shared history.
  page.get_by_role('button',name='库内匹配 one@example.invalid，查看详情',exact=True).click()
  expect(page.get_by_role('article',name='检测结果')).to_be_visible()
  expect(page.locator('.account-history-detail details')).to_have_count(3)
  page.locator('.account-history-detail details').first.locator('summary').click()
  expect(page.locator('.account-history-detail details').first.locator('pre').last).to_have_text('match')
  for mode,label in [('mismatch','库内不匹配'),('unknown','证据不足'),('error','探查失败')]:
   page.evaluate('(m)=>accountMode=m',mode)
   page.get_by_role('button',name='重新探查当前模型',exact=True).click()
   page.get_by_role('button',name='关闭',exact=True).click()
   page.wait_for_function('pendingJobs.length===1')
   page.evaluate('pendingJobs.shift()()')
   page.get_by_role('button',name=f'{label} one@example.invalid，查看详情',exact=True).click()
   expect(page.locator('.account-history-detail > strong')).to_contain_text(label)
  expect(page.get_by_text('模拟采样失败',exact=True)).to_be_visible()
  # Old session records remain readable but cannot pay for a different current account.
  page.evaluate('__testSnapshot.accountTests={};__testPublish()')
  expect(page.get_by_role('button',name='重新探查当前模型',exact=True)).to_be_disabled()
  assert page.evaluate("!__testCalls.some(c=>['credentials_toggle','credential_toggle','credential_delete'].includes(c.cmd))")
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='probe_account_samples').every(c=>c.args.challenges.length===3)")
  snapshot=page.evaluate('__testSnapshot')
  # A different webview restores backend history instead of relying on local React state.
  tray=context.new_page();tray.set_viewport_size({'width':400,'height':270})
  tray.add_init_script(script='window.addEventListener("DOMContentLoaded",()=>{});')
  tray.goto(url+'?view=tray')
  tray.evaluate('(s)=>{__testSnapshot=s;__testPublish()}',snapshot)
  tray.get_by_role('button',name='历史',exact=True).click()
  expect(tray.locator('.probe-history > .button')).to_have_count(7)
  tray.locator('.probe-history > .button').first.click()
  expect(tray.locator('.account-history-detail > strong')).to_contain_text('探查失败')
  for theme in ['light','dark']:
   tray.emulate_media(color_scheme=theme)
   assert tray.evaluate('document.documentElement.scrollWidth<=innerWidth')
  if engine.name=='webkit':tray.screenshot(path='/tmp/modellens-account-history.png')
  assert not errors,errors
  context.close();browser.close()
  print(engine.name+': three parallel jobs, background progress, shared history, raw samples, all outcomes and safe retry passed')
