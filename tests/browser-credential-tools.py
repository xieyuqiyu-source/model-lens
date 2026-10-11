"""Synthetic batch, plan and access UI; never touches real credentials/clipboard."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
url = os.environ.get('MODEL_LENS_TEST_URL', 'http://127.0.0.1:1420')
fixture = Path(__file__).parent / 'fixtures/credentials-ipc.js'
setup = """
window.__testCopied = [];
Object.defineProperty(navigator, 'clipboard', {value:{writeText: async value => __testCopied.push(value)}});
__testSnapshot.credentials[0].plan = 'free';
__testSnapshot.credentials[0].resetAvailable = 0;
__testSnapshot.credentials[0].quotaItems = [{label:'1w',period:'1w',remaining:60,reset:'',detail:''}];
for (const plan of ['plus','pro']) __testSnapshot.credentials.push({
 ...__testSnapshot.credentials[0], id:plan+'.json:idx',name:plan+'@example.invalid',fileName:plan+'.json',deleteToken:plan,plan,enabled:true,quotaItems:[],resetAvailable:plan==='pro'?3:null
});
"""
with sync_playwright() as p:
 for engine in [p.webkit,p.chromium]:
  browser=engine.launch(headless=True)
  context=browser.new_context(viewport={'width':1120,'height':760})
  context.add_init_script(path=fixture);context.add_init_script(script=setup)
  page=context.new_page();errors=[]
  page.on('pageerror',lambda e: errors.append(str(e)))
  page.goto(url)
  expect(page.get_by_role('button',name='http://127.0.0.1:8317/v1')).to_be_visible()
  page.get_by_role('button',name='http://127.0.0.1:8317/v1').click()
  expect(page.get_by_role('status')).to_have_text('代理地址已复制')
  assert page.evaluate("__testCopied.join('') === 'http://127.0.0.1:8317/v1'")
  page.get_by_role('button',name='复制 API Key',exact=True).click()
  expect(page.get_by_role('status')).to_have_text('API Key 已复制')
  assert page.evaluate('__testKeyCopies === 1')
  page.evaluate("__testNextError='CLIProxy 未配置客户端 API Key'")
  page.get_by_role('button',name='复制 API Key',exact=True).click()
  expect(page.get_by_role('alert')).to_contain_text('未配置客户端 API Key')
  expect(page.get_by_role('status')).to_have_text('')
  for label in ['Free','Plus','Pro']: expect(page.locator('.credential-grid').get_by_text(label,exact=True)).to_be_visible()
  free=page.locator('article').filter(has_text='stopped@example.invalid')
  expect(free.get_by_role('meter')).to_have_count(1)
  expect(free.get_by_text('重置剩余 0 次',exact=True)).to_be_visible()
  expect(page.get_by_text('重置剩余 3 次',exact=True)).to_be_visible()
  expect(page.get_by_label('选择 runtime@example.invalid')).to_be_disabled()
  for theme in ['light','dark']:
   page.emulate_media(color_scheme=theme)
   expect(page.locator("html")).to_have_attribute("data-appearance",theme)
   page.wait_for_function("Array.from(document.querySelectorAll('.proxy-url')).every(e => getComputedStyle(e).color === getComputedStyle(document.querySelector('.segment button[aria-pressed=true]')).color)")
   colors=page.locator('.plan-badge').evaluate_all('(xs)=>xs.filter(x=>["free","plus","pro"].includes(x.dataset.plan)).map(x=>getComputedStyle(x).color)')
   assert len(set(colors))==3,colors
   assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
  master=page.get_by_role('switch',name='全部凭证开关',exact=True)
  expect(page.get_by_label('全选可删除凭证')).not_to_be_checked()
  expect(master).to_be_checked()
  master.click()
  page.wait_for_function('__testSnapshot.busy === false && __testSnapshot.credentials.every(c=>!c.enabled)')
  expect(master).not_to_be_checked()
  assert page.evaluate('__testCalls.filter(c=>c.cmd==="credentials_toggle").at(-1).args.targets.length===2')
  assert page.evaluate('__testCalls.filter(c=>c.cmd==="credentials_toggle").at(-1).args.enabled===false')
  master.click()
  page.wait_for_function('__testSnapshot.busy === false && __testSnapshot.credentials.filter(c=>c.editable).every(c=>c.enabled)')
  expect(master).to_be_checked()
  expect(master).to_be_enabled()
  assert page.evaluate('__testCalls.filter(c=>c.cmd==="credentials_toggle").at(-1).args.targets.length===3')
  expect(page.get_by_label('全选可删除凭证')).not_to_be_checked()
  page.get_by_label('全选可删除凭证').check()
  page.get_by_role('button',name='删除所选 (3)',exact=True).click()
  dialog=page.get_by_role('dialog',name='批量删除凭证')
  expect(dialog.get_by_role('listitem')).to_have_count(3)
  dialog.get_by_role('button',name='取消',exact=True).click()
  expect(dialog).to_have_count(0)
  assert page.evaluate("!__testCalls.some(c=>c.cmd==='credentials_delete')")
  page.get_by_label('全选可删除凭证').check()
  page.get_by_role('button',name='删除所选 (3)',exact=True).click()
  page.evaluate("__testSnapshot.credentials[0].deleteToken='external';__testPublish()")
  expect(dialog.get_by_role('button',name='确认删除 3 个凭证',exact=True)).to_be_disabled()
  dialog.get_by_role('button',name='关闭',exact=True).click()
  expect(dialog).to_have_count(0)
  page.get_by_label('全选可删除凭证').check()
  page.get_by_role('button',name='删除所选 (3)',exact=True).click()
  dialog.get_by_role('button',name='确认删除 3 个凭证',exact=True).click()
  expect(dialog).to_have_count(0)
  expect(page.locator('article')).to_have_count(1)
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='credentials_delete').length===1")
  assert not errors,errors
  # Fresh preview: layout and old reset values when optional request fails.
  page.goto(url)
  page.evaluate("__testSnapshot.credentials[3].resetCountError='HTTP 403';__testPublish()")
  expect(page.get_by_text('上次剩余 3 次 · 更新失败',exact=True)).to_be_visible()
  for width in [920,1120,1280]:
   page.set_viewport_size({'width':width,'height':760})
   assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
   rects=page.locator('.proxy-access, .credential-batch-actions, .credential-grid').evaluate_all('(xs)=>xs.map(x=>{const r=x.getBoundingClientRect();return {top:r.top,bottom:r.bottom}})')
   assert rects[0]['bottom']<=rects[1]['top'] and rects[1]['bottom']<=rects[2]['top'],rects
  if engine.name=='webkit': page.screenshot(path='/tmp/modellens-credential-tools.png',full_page=True,animations='disabled')
  expect(page.get_by_role('button',name='历史',exact=True)).to_have_text('')
  expect(page.get_by_text('已停用，仍可查询额度',exact=True)).to_have_count(0)
  page.get_by_role('button',name='探查优先',exact=True).click()
  expect(page.locator('.credential-row .plan-badge')).to_have_count(4)
  for label in ['Free','Plus','Pro']: expect(page.locator('.credential-rows').get_by_text(label,exact=True)).to_be_visible()
  page.get_by_role('button',name='选择模型范围：全部已启用凭证',exact=True).click()
  for label in ['Plus','Pro']: expect(page.get_by_role('listbox').get_by_text(label,exact=True)).to_be_visible()
  page.keyboard.press('Escape')
  page.goto(url+'?view=tray')
  page.set_viewport_size({'width':400,'height':270})
  expect(page.locator('.credential-row .plan-badge')).to_have_count(4)
  for label in ['Free','Plus','Pro']: expect(page.locator('.credential-rows').get_by_text(label,exact=True)).to_be_visible()
  assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
  widths=page.locator('.account-bar').evaluate_all('(xs)=>xs.map(x=>x.getBoundingClientRect().width)')
  assert min(widths)>80,widths
  assert not errors,errors
  context.close();browser.close()
  print(engine.name+': proxy copy, backend key action, plans, resets, batch controls and confirmation passed')
