"""Synthetic account sampling and worker outcomes; no real models, keys or credentials."""
import os
import re
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
url = os.environ.get('MODEL_LENS_TEST_URL', 'http://127.0.0.1:1420')
fixture = Path(__file__).parent / 'fixtures/credentials-ipc.js'
setup = r"""
localStorage.setItem('focus','probe');
__testSnapshot.credentials[0].enabled = true;
__testSnapshot.credentials[0].five = 80;
__testSnapshot.credentials[0].week = 70;
window.accountMode='match';
window.catalogError=false;
const invoke = __TAURI_INTERNALS__.invoke;
__TAURI_INTERNALS__.invoke = async (cmd,args) => {
 if(cmd==='probe_sample') throw 'HTTP 404';
 if(cmd==='probe_models') {
  if(catalogError) throw 'HTTP 404';
  return [{id:'gpt-5.4',name:'GPT-5.4',brand:'openai'},{id:'gpt-5.3',name:'GPT-5.3',brand:'openai'}];
 }
 if(cmd==='probe_account_samples') {
  __testCalls.push({cmd,args});
  const runId=__testSnapshot.revision;
  __testSnapshot.busy=true;
  __testSnapshot.accountTests={[args.id]:{model:args.model,runId,status:'running',message:'采样中',at:Date.now()/1000}};
  __testPublish();
  await new Promise(r=>setTimeout(r,700));
  const error=accountMode==='error'?'模拟采样失败':null;
  __testSnapshot.accountTests[args.id].status=error?'error':'analyzing';
  __testSnapshot.accountTests[args.id].message=error||'分析中';
  __testSnapshot.busy=false;__testPublish();
  return {runId,error,answers:error?[]:args.prompts.map(()=>({text:'synthetic',finish:'stop',reportedModel:'ignored',elapsedMs:1}))};
 }
 if(cmd==='probe_account_finish') {
  __testCalls.push({cmd,args});
  Object.assign(__testSnapshot.accountTests[args.id],{status:args.status,message:args.message});
  __testPublish();return;
 }
 return invoke(cmd,args);
};
// UI state/colour regression only; mathematical matching has independent unit tests.
window.Worker=class {
 postMessage() {setTimeout(()=>this.onmessage({data:{result:{analysis:{results:[{model:'gpt-5.4'},{model:'gpt-5.3'}],used_outputs:3,probability_status:'reference_calibrated',probability:accountMode==='unknown'?.4:.99,prediction:accountMode==='mismatch'?'gpt-5.3':'gpt-5.4',prediction_name:'Synthetic'}}}}),200);}
 terminate() {}
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
  icon=page.get_by_role('button',name='测试 stopped@example.invalid',exact=True)
  expect(icon).to_be_enabled()
  icon.click()
  expect(icon).to_be_disabled()
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='probe_account_samples').length===1")
  # Moving to the other panel must not cancel the background test.
  page.get_by_role('button',name='凭证优先',exact=True).click()
  card=page.locator('article').filter(has_text='stopped@example.invalid')
  expect(card).to_have_attribute('data-test','match')
  assert page.evaluate("__testCalls.find(c=>c.cmd==='probe_account_samples').args.prompts.length===3")
  page.get_by_role('button',name='选择模型：GPT-5.4',exact=True).click()
  page.get_by_role('option',name=re.compile('GPT-5.3')).click()
  expect(card).not_to_have_attribute('data-test','match')
  page.get_by_role('button',name='选择模型：GPT-5.3',exact=True).click()
  page.get_by_role('option',name=re.compile('GPT-5.4')).click()
  expect(card).to_have_attribute('data-test','match')
  for mode in ['mismatch','unknown','error']:
   page.evaluate(f"accountMode='{mode}'")
   icon.click()
   expect(card).to_have_attribute('data-test',mode)
  assert page.evaluate("!__testCalls.some(c=>['credentials_toggle','credential_delete'].includes(c.cmd))")
  if engine.name=='webkit': page.screenshot(path='/tmp/modellens-account-probe-cards.png',full_page=True,animations='disabled')
  # HTTP error and empty result should have visible separation at narrow widths.
  page.get_by_role('button',name='探查优先',exact=True).click()
  page.get_by_role('button',name='开始检测',exact=True).click()
  expect(page.locator('.probe > .notice')).to_have_text('HTTP 404')
  for width in [920,1120]:
   page.set_viewport_size({'width':width,'height':760})
   metrics=page.evaluate("""()=>{const n=document.querySelector('.probe > .notice'), r=document.querySelector('.probe > .result');return {gap:r.getBoundingClientRect().top-n.getBoundingClientRect().bottom,pad:parseFloat(getComputedStyle(n).paddingTop),overflow:document.documentElement.scrollWidth>innerWidth}}""")
   assert metrics['gap']>=15 and metrics['pad']>=12 and not metrics['overflow'],metrics
  if engine.name=='webkit': page.screenshot(path='/tmp/modellens-account-probe-error.png',full_page=True,animations='disabled')
  page.goto(url+'?view=tray')
  page.set_viewport_size({'width':400,'height':270})
  icon=page.get_by_role('button',name='测试 stopped@example.invalid',exact=True)
  bar=page.locator('.account-bar').first
  for mode in ['match','mismatch']:
   page.evaluate(f"accountMode='{mode}'")
   icon.click();expect(bar).to_have_attribute('data-test',mode)
   # Quota refresh errors must not accidentally override a test verdict border.
   page.evaluate("__testSnapshot.credentials[0].quotaError='old quota error';__testPublish()")
   page.wait_for_function("(mode)=>{let b=document.querySelector('.account-bar');let color=getComputedStyle(document.documentElement).getPropertyValue('--test-'+mode).trim();let tmp=document.createElement('i');tmp.style.color=color;document.body.append(tmp);let rgb=getComputedStyle(tmp).color;tmp.remove();return getComputedStyle(b,'::after').borderColor===rgb}",arg=mode)
  for theme in ['light','dark']:
   page.emulate_media(color_scheme=theme)
   expect(page.locator('html')).to_have_attribute('data-appearance',theme)
   assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
   rects=page.locator('.credential-row').first.locator('.account-bar, .account-test, .switch').evaluate_all('(xs)=>xs.map(x=>{let r=x.getBoundingClientRect();return {left:r.left,right:r.right,width:r.width}})')
   assert len(rects)==3 and rects[0]['right']<=rects[1]['left'] and rects[0]['width']>=80 and rects[-1]['right']<=384,rects
  if engine.name=='webkit': page.screenshot(path='/tmp/modellens-account-probe-tray.png',animations='disabled')
  assert not errors,errors
  context.close();browser.close()
  print(engine.name+': background account test, verdict colours, model isolation, no toggles, tray and error spacing passed')
