"""New provider UI with synthetic IPC; no credentials or external service calls."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

url = os.environ.get('MODEL_LENS_TEST_URL', 'http://127.0.0.1:1421')
fixture = Path(__file__).parent / 'fixtures/credentials-ipc.js'
providers = '''
const quota = (label, remaining, period, detail='') => ({label, remaining, period, detail, reset:'2026-11-01T00:00:00Z'});
const credential = (name, provider, brand, items) => ({id:name,name:name+'@example.invalid',provider,brand,enabled:false,editable:true,deletable:false,supported:true,five:null,week:null,resetFive:'',resetWeek:'',quotaItems:items,quotaAt:123,quotaError:null});
__testSnapshot.credentials = [
 credential('grok','xai','grok',[quota('Grok · 月度额度',75,'月度')]),
 credential('gemini','gemini-cli','gemini',[quota('gemini-test-pro · 请求',100,'模型'),quota('gemini-test-flash · 请求',0,'模型'),quota('gemini-amount · 请求',null,'模型','剩余 17 请求'),quota('gemini-unknown · 请求',null,'模型')]),
 credential('antigravity','antigravity','gemini',[quota('Gemini Pro · 5h',80,'5h'),quota('Gemini Flash · 5h',50,'5h'),quota('Gemini Pro · 1w',0,'1w')])
];
'''
with sync_playwright() as p:
    for engine in [p.webkit, p.chromium]:
        browser = engine.launch(headless=True)
        context = browser.new_context(viewport={'width':1120,'height':760})
        context.add_init_script(path=fixture)
        context.add_init_script(script=providers)
        page = context.new_page()
        errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.goto(url)
        grok = page.locator('article').filter(has_text='grok@example.invalid')
        gemini = page.locator('article').filter(has_text='gemini@example.invalid')
        expect(grok.get_by_role('meter')).to_have_attribute('aria-valuenow','75')
        expect(grok.get_by_role('button', name='刷新额度', exact=True)).to_be_enabled()
        expect(grok.get_by_role('button', name='探查模型')).to_be_disabled()
        # Probe access follows enabled state, independently from quota provider support.
        page.evaluate('__testSnapshot.credentials[0].enabled = true; __testPublish()')
        expect(grok.get_by_role('button', name='探查模型')).to_be_enabled()
        page.evaluate('__testSnapshot.credentials[0].enabled = false; __testPublish()')
        expect(gemini.get_by_text('100% 剩余',exact=True)).to_be_visible()
        expect(gemini.get_by_text('0% 剩余',exact=True)).to_be_visible()
        expect(gemini.get_by_text('剩余 17 请求',exact=True)).to_be_visible()
        expect(gemini.get_by_text('未提供',exact=True)).to_be_visible()
        expect(gemini.get_by_role('meter')).to_have_count(2)
        # A query error preserves the last values and does not enable the credential.
        page.evaluate("__testSnapshot.credentials[0].quotaError = '额度查询返回 HTTP 429'; __testPublish()")
        expect(grok.get_by_text('仍显示上次数据',exact=False)).to_be_visible()
        expect(grok.get_by_role('meter')).to_have_attribute('aria-valuenow','75')
        expect(grok.get_by_role('switch')).to_have_attribute('aria-checked','false')
        for theme in ['light','dark']:
            page.emulate_media(color_scheme=theme)
            expect(page.locator('html')).to_have_attribute('data-appearance', theme)
            assert page.locator('img.brand').evaluate_all('(imgs) => imgs.every(i => i.complete && i.naturalWidth > 0)')
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
            if engine.name == 'webkit': page.screenshot(path=f'/tmp/modellens-quota-{theme}.png', full_page=True, animations="disabled")
        page.goto(url+'?view=tray')
        rows = page.locator('.credential-row')
        expect(rows).to_have_count(3)
        expect(rows.nth(0).locator('.period')).to_have_text('月度')
        expect(rows.nth(0).locator('.account-bar')).to_have_attribute('style','--quota-width: 75%;')
        expect(rows.nth(1).locator('.period')).to_have_text('最低')
        expect(rows.nth(1).locator('.account-bar')).to_have_attribute('style','--quota-width: 0%;')
        expect(rows.nth(2).locator('.account-bar')).to_have_attribute('style','--quota-width: 50%;')
        page.set_viewport_size({'width':400,'height':270})
        expect(rows.nth(0).get_by_role('button',name='刷新 grok@example.invalid 额度')).to_be_enabled()
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
        # All unknown percentages remain a neutral empty bar, even with a count.
        page.evaluate("__testSnapshot.credentials[1].quotaItems = [{label:'请求',remaining:null,detail:'剩余 17 请求',reset:'',period:'模型'}]; __testPublish()")
        expect(rows.nth(1).locator('.account-bar')).to_have_attribute('data-quota','missing')
        expect(rows.nth(1).locator('.account-bar')).to_have_attribute('title',__import__('re').compile('剩余 17 请求'))
        assert not errors, errors
        context.close(); browser.close()
        print(f'{engine.name}: provider quota cards, icons, dark/light, errors and 400×270 tray passed')
