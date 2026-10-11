"""启动 pnpm dev 后：uv run --with playwright python tests/browser-focus.py"""
from playwright.sync_api import sync_playwright, expect


def has_ring(locator):
    return locator.evaluate("e => getComputedStyle(e).outlineStyle !== 'none' && getComputedStyle(e).outlineWidth !== '0px'")


def focus_with_tab(page, target):
    for _ in range(30):
        page.keyboard.press(tab_key)
        if target.evaluate('e => e === document.activeElement'):
            return
    raise AssertionError('Tab 未能到达目标控件')


with sync_playwright() as p:
    for engine in [p.webkit, p.chromium]:
        tab_key = 'Alt+Tab' if engine.name == 'webkit' else 'Tab'
        browser = engine.launch(headless=True)
        for tray in [False, True]:
            page = browser.new_page(viewport={'width': 400 if tray else 1120, 'height': 270 if tray else 760})
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto('http://127.0.0.1:1420' + ('/?view=tray' if tray else ''))
            page.wait_for_load_state('networkidle')
            trigger = page.get_by_role('button', name='外观设置' if tray else '外观与连接设置', exact=True)
            close = page.get_by_role('button', name='关闭', exact=True)
            dialog = page.get_by_role('dialog')

            # 鼠标打开和关闭时，自动焦点不应画出键盘焦点框。
            trigger.click()
            expect(close).to_be_focused()
            assert not has_ring(close)
            close.click()
            expect(dialog).to_have_count(0)
            assert not page.locator(':focus-visible').evaluate_all("els => els.some(e => getComputedStyle(e).outlineStyle !== 'none')")

            # 已有键盘焦点后再用鼠标打开，关闭恢复的旧焦点也不能残留边框。
            focus_with_tab(page, trigger)
            assert has_ring(trigger)
            trigger.click()
            assert not has_ring(close)
            close.click()
            expect(dialog).to_have_count(0)
            assert not has_ring(trigger)

            # 键盘入口、弹窗内 Tab、焦点约束、Esc 和入口焦点恢复。
            focus_with_tab(page, trigger)
            trigger.press('Enter')
            expect(close).to_be_focused()
            assert has_ring(close)
            for _ in range(12):
                page.keyboard.press(tab_key)
                assert dialog.evaluate('e => document.activeElement === document.body || e.contains(document.activeElement)')
                if page.locator(':focus').count():
                    assert has_ring(page.locator(':focus'))
            page.keyboard.press('Escape')
            expect(dialog).to_have_count(0)
            expect(trigger).to_be_focused()
            assert has_ring(trigger)

            if not tray:
                # 背景滚动区原有焦点、遮罩关闭和文本输入焦点反馈。
                panel = page.get_by_role('region', name='模型探查', exact=True)
                focus_with_tab(page, panel)
                trigger.click()
                page.mouse.click(10, 100)
                expect(dialog).to_have_count(0)
                assert not has_ring(panel)
                trigger.click()
                page.get_by_role('button', name='管理 CLIProxy 连接').click()
                password = page.locator('input[type="password"]')
                password.click()
                assert has_ring(password)
                close.click()
                expect(dialog).to_have_count(0)
            assert not errors, errors
            print(f'{engine.name} / {"菜单栏" if tray else "主窗口"}: 焦点回归通过')
            page.close()
        browser.close()
