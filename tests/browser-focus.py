"""启动 pnpm dev 后：uv run --with playwright python tests/browser-focus.py"""
from playwright.sync_api import sync_playwright, expect
import re


def has_shadow(locator):
    return locator.evaluate("""async e => {
        await Promise.all(e.getAnimations().map(a => a.finished.catch(() => {})));
        const style = getComputedStyle(e);
        if (style.outlineStyle !== 'none') throw new Error('仍有硬焦点边框');
        return style.boxShadow !== 'none';
    }""")


def focus_with_tab(page, target):
    for _ in range(30):
        page.keyboard.press(tab_key)
        if target.evaluate('e => e === document.activeElement'):
            return
    raise AssertionError('Tab 未能到达目标控件')


def check_select(page, trigger):
    baseline = trigger.evaluate('e => getComputedStyle(e).borderColor')
    trigger.click()
    menu = page.get_by_role('listbox')
    expect(menu).to_be_visible()
    expect(trigger).to_have_css('border-color', baseline)
    expect(menu).to_have_css('outline-style', 'none')
    menu.get_by_role('option').nth(1).click()
    expect(menu).not_to_be_visible()
    assert not has_shadow(trigger)
    trigger.click()
    trigger.click()
    expect(menu).not_to_be_visible()
    assert not has_shadow(trigger)
    trigger.click()
    page.mouse.click(10, 100)
    expect(menu).not_to_be_visible()
    assert not has_shadow(trigger)
    focus_with_tab(page, trigger)
    trigger.press('ArrowDown')
    expect(menu).to_be_visible()
    menu.press('End')
    menu.press('Enter')
    expect(menu).not_to_be_visible()
    expect(trigger).to_be_focused()
    assert has_shadow(trigger)
    trigger.press('ArrowDown')
    menu.press('Escape')
    expect(trigger).to_be_focused()
    assert has_shadow(trigger)
    trigger.press('ArrowDown')
    menu.press('Tab')
    expect(menu).not_to_be_visible()


def check_dialog(page, trigger, title):
    dialog = page.get_by_role('dialog', name=title, exact=True)
    close = dialog.get_by_role('button', name='关闭', exact=True)
    trigger.click()
    expect(close).to_be_focused()
    assert not has_shadow(close)
    close.click()
    expect(dialog).to_have_count(0)
    assert not has_shadow(trigger)
    trigger.click()
    page.mouse.click(10, 100)
    expect(dialog).to_have_count(0)
    assert not has_shadow(trigger)
    focus_with_tab(page, trigger)
    trigger.press('Enter')
    expect(close).to_be_focused()
    assert has_shadow(close)
    close.press('Escape')
    expect(dialog).to_have_count(0)
    expect(trigger).to_be_focused()
    assert has_shadow(trigger)


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
            assert not has_shadow(close)
            close.click()
            expect(dialog).to_have_count(0)
            assert not page.locator(':focus-visible').evaluate_all("els => els.some(e => getComputedStyle(e).outlineStyle !== 'none')")

            # 已有键盘焦点后再用鼠标打开，关闭恢复的旧焦点也不能残留边框。
            focus_with_tab(page, trigger)
            assert has_shadow(trigger)
            trigger.click()
            assert not has_shadow(close)
            close.click()
            expect(dialog).to_have_count(0)
            assert not has_shadow(trigger)

            # 键盘入口、弹窗内 Tab、焦点约束、Esc 和入口焦点恢复。
            focus_with_tab(page, trigger)
            trigger.press('Enter')
            expect(close).to_be_focused()
            assert has_shadow(close)
            for _ in range(12):
                page.keyboard.press(tab_key)
                assert dialog.evaluate('e => document.activeElement === document.body || e.contains(document.activeElement)')
                if page.locator(':focus').count():
                    assert has_shadow(page.locator(':focus'))
            page.keyboard.press('Escape')
            expect(dialog).to_have_count(0)
            expect(trigger).to_be_focused()
            assert has_shadow(trigger)

            if not tray:
                # 背景滚动区原有焦点、遮罩关闭和文本输入焦点反馈。
                panel = page.get_by_role('region', name='模型探查', exact=True)
                focus_with_tab(page, panel)
                trigger.click()
                page.mouse.click(10, 100)
                expect(dialog).to_have_count(0)
                assert not has_shadow(panel)
                trigger.click()
                page.get_by_role('button', name='管理 CLIProxy 连接').click()
                password = page.locator('input[type="password"]')
                password.click()
                assert has_shadow(password)
                close.click()
                expect(dialog).to_have_count(0)
            check_select(page, page.get_by_role('button', name=re.compile('选择模型（预览）')))
            if tray:
                check_dialog(page, page.get_by_role('button', name='CLIProxy 状态未知', exact=True), '本机 CLIProxy')
            if not tray:
                check_select(page, page.get_by_role('button', name=re.compile('选择示例凭证')))
                for name, title in [('CLIProxy 状态未知', '本机 CLIProxy'), ('历史', '检测历史'), ('管理连接', '连接 CLIProxy'), ('菜单栏快捷入口', '菜单栏面板预览')]:
                    check_dialog(page, page.get_by_role('button', name=name, exact=True), title)
                page.get_by_role('button', name='凭证优先', exact=True).click()
                # Import now requires a connection; connected dialog flows use synthetic IPC in browser-credentials.py.
                expect(page.get_by_role('button', name='导入 JSON', exact=True)).to_be_disabled()
                page.get_by_role('button', name='探查优先', exact=True).click()
                for name in ['检测提示词', '原始回答与采样详情']:
                    control = page.get_by_role('button', name=name, exact=True)
                    page.mouse.move(10, 100)
                    before = control.evaluate('e => getComputedStyle(e).backgroundColor')
                    control.hover()
                    expect(control).to_have_css('background-color', before)
                    control.click()
                    expect(control).to_have_css('background-color', before)
                    assert not has_shadow(control)
                page.get_by_role('button', name='手动检测', exact=True).click()
                answer = page.get_by_role('textbox', name='外部模型回答')
                expect(answer).to_have_attribute('rows', '2')
                metrics = answer.evaluate("e => {const s=getComputedStyle(e);return {height:e.clientHeight,lines:parseFloat(s.lineHeight),padding:parseFloat(s.paddingTop)+parseFloat(s.paddingBottom)};}")
                assert abs(metrics['height'] - metrics['padding'] - metrics['lines'] * 2) < 2, metrics
                answer.fill('合成回答\n' * 30)
                assert has_shadow(answer)
                assert answer.evaluate('e => e.scrollHeight > e.clientHeight')
                answer.press('Control+End')
                answer.fill('')
                page.mouse.click(10, 100)
                assert not has_shadow(answer)
                for palette in ['翡翠绿', '海湾蓝', '鸢尾紫']:
                    for mode in ['浅色', '深色']:
                        trigger.click()
                        page.get_by_role('button', name=re.compile(palette)).click()
                        page.get_by_role('button', name=mode, exact=True).click()
                        close.click()
                        expect(dialog).to_have_count(0)
                        answer.click()
                        assert has_shadow(answer)
                        expect(answer).to_have_css('transition-property', re.compile('box-shadow'))
                        page.mouse.click(10, 100)
                        assert not has_shadow(answer)
                page.screenshot(path=f'/tmp/modellens-0.2.9-{engine.name}-dark.png')
            assert not errors, errors
            print(f'{engine.name} / {"菜单栏" if tray else "主窗口"}: 焦点回归通过')
            page.close()
        browser.close()
