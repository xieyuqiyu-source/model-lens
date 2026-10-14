"""Synthetic groups/CPA actions; never opens or pulls a real CPA repository."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
url=os.environ.get('MODEL_LENS_TEST_URL','http://127.0.0.1:1422')
fixture=Path(__file__).parent/'fixtures/credentials-ipc.js'
setup=r"""
__testSnapshot.credentials=[
 {...row('a-codex',true),provider:'codex'},{...row('b-claude',true),provider:'claude',brand:'claude'},
 {...row('c-gemini'),provider:'gemini-cli',brand:'gemini'},{...row('d-grok'),provider:'xai',brand:'xai'},
 {...row('e-antigravity'),provider:'antigravity',brand:'gemini'},{...row('f-other'),provider:'unknown'},
 {...row('z-codex'),provider:'codex'}
];
const invoke=__TAURI_INTERNALS__.invoke;
__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
 if(['cpa_open_management','cpa_pull','quota_refresh_group'].includes(cmd)){
  __testCalls.push({cmd,args});
  await new Promise(r=>setTimeout(r,180));
  if(window.cpaError) throw window.cpaError;
  if(cmd==='cpa_pull') return {updated:!window.noUpdate,before:'aaaaaaa',after:'bbbbbbb',directory:'/synthetic/CLIProxyApi'};
  if(cmd==='quota_refresh_group'){
   const group=p=>({'gemini-cli':'gemini',xai:'grok'})[p]||p;
   for(const c of __testSnapshot.credentials.filter(c=>group(c.provider)===args.group)){c.five=42;c.quotaAt=100;}
   __testPublish();return structuredClone(__testSnapshot);
  }
  return;
 }
 return invoke(cmd,args);
};
"""
with sync_playwright() as p:
 for engine in [p.webkit,p.chromium]:
  browser=engine.launch(headless=True)
  context=browser.new_context(viewport={'width':1120,'height':760})
  context.add_init_script(script=fixture.read_text()+'\n'+setup)
  page=context.new_page();errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
  page.goto(url)
  tabs=page.get_by_role('group',name='凭证分组')
  expect(tabs.get_by_role('button')).to_have_count(7)
  expect(page.locator('.credential-card')).to_have_count(7)
  tabs.get_by_role('button',name='Codex · 2',exact=True).click()
  expect(page.locator('.credential-card')).to_have_count(2)
  names=page.locator('.credential-card').get_by_role('switch').evaluate_all('(xs)=>xs.map(x=>x.getAttribute("aria-label"))')
  page.get_by_role('switch',name='本组凭证开关',exact=True).click()
  page.wait_for_function("__testCalls.some(c=>c.cmd==='credentials_toggle')")
  assert page.evaluate("__testCalls.find(c=>c.cmd==='credentials_toggle').args.targets.every(t=>t.id.includes('codex'))")
  expect(page.get_by_role('switch',name='本组凭证开关',exact=True)).to_be_enabled()
  assert page.locator('.credential-card').get_by_role('switch').evaluate_all('(xs)=>xs.map(x=>x.getAttribute("aria-label"))')==names
  assert page.evaluate("__testSnapshot.credentials.find(c=>c.provider==='claude').enabled")
  page.get_by_label('全选可删除凭证',exact=True).check()
  expect(page.get_by_role('button',name='删除所选 (2)',exact=True)).to_be_enabled()
  tabs.get_by_role('button',name='Claude · 1',exact=True).click()
  expect(page.get_by_role('button',name='删除所选',exact=True)).to_be_disabled()
  page.get_by_label('全选可删除凭证',exact=True).check()
  page.get_by_role('button',name='删除所选 (1)',exact=True).click()
  expect(page.locator('.batch-delete-list li')).to_have_count(1)
  expect(page.locator('.batch-delete-list')).to_contain_text('b-claude@example.invalid')
  page.get_by_role('button',name='取消',exact=True).click()
  page.get_by_role('button',name='刷新本组额度',exact=True).click()
  page.wait_for_function("__testSnapshot.credentials.find(c=>c.provider==='claude').five===42")
  assert page.evaluate("__testSnapshot.credentials.filter(c=>c.provider!=='claude').every(c=>c.five===null)")
  tabs.get_by_role('button',name='Antigravity · 1',exact=True).click()
  expect(page.locator('.credential-card')).to_have_count(1)
  expect(page.locator('.credential-card')).to_contain_text('e-antigravity')
  page.reload();expect(tabs.get_by_role('button',name='Antigravity · 1',exact=True)).to_have_attribute('aria-pressed','true')
  page.get_by_role('button',name='探查优先',exact=True).click()
  expect(page.locator('.credential-row')).to_have_count(7)
  page.get_by_role('button',name='凭证优先',exact=True).click()
  expect(page.locator('.credential-card')).to_have_count(1)
  page.get_by_role('button',name='CPA 管理',exact=True).click()
  page.wait_for_function("__testCalls.some(c=>c.cmd==='cpa_open_management')")
  update=page.get_by_role('button',name='拉取更新',exact=True)
  expect(update).to_be_enabled();update.click()
  expect(page.get_by_text('源码已更新至 bbbbbbb，尚未编译或重启',exact=True)).to_be_visible()
  page.evaluate('noUpdate=true');update.click()
  expect(page.get_by_text('没有可快进的新提交',exact=True)).to_be_visible()
  page.evaluate("cpaError='CPA 仓库有未提交改动，请先处理；未拉取更新'");update.click()
  expect(page.get_by_text('CPA 仓库有未提交改动，请先处理；未拉取更新',exact=True)).to_be_visible()
  # Tabs remain scrollable at narrower widths and after the final member disappears.
  for width in [920,1120]:
   page.set_viewport_size({'width':width,'height':760})
   assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
  page.evaluate("__testSnapshot.credentials=__testSnapshot.credentials.filter(c=>c.provider!=='antigravity');__testPublish()")
  expect(tabs.get_by_role('button',name='全部 · 6',exact=True)).to_have_attribute('aria-pressed','true')
  expect(page.locator('.credential-card')).to_have_count(6)
  if engine.name=='webkit':page.screenshot(path='/tmp/modellens-groups.png',full_page=True)
  assert not errors,errors
  context.close();browser.close()
  print(engine.name+': provider tabs, scoped batch/delete/quota, selection reset, persistence, CPA actions and failure feedback passed')
