"""Run against a production preview. All updater commands use synthetic IPC."""
import os
from pathlib import Path
from playwright.sync_api import expect, sync_playwright

url = os.environ.get("MODELLENS_TEST_URL", "http://127.0.0.1:1425/").rstrip("/") + "/"
fixture = (Path(__file__).parent / "fixtures/credentials-ipc.js").read_text() + """
const invokeFixture = window.__TAURI_INTERNALS__.invoke;
window.installCalls = 0;
window.updateChecks = 0;
window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    if (cmd === 'plugin:updater|check') {
        window.updateChecks++;
        return {rid: 42, currentVersion: '0.2.32', version: '0.2.33'};
    }
    if (cmd === 'install_macos_update') {
        if (args.rid !== 42) throw 'Unexpected update resource';
        window.installCalls++;
        await new Promise((_, reject) => {
            window.finishInstall = () => reject('合成安装失败，请重试。');
        });
    }
    if (cmd === 'plugin:resources|close') return;
    return invokeFixture(cmd, args);
};
"""

with sync_playwright() as p:
    for engine in (p.webkit, p.chromium):
        with engine.launch() as browser:
            config = {"user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15"}
            context = browser.new_context(**config)
            context.add_init_script(fixture)
            page = context.new_page()
            errors = []
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.goto(url)
            update = page.get_by_role("button", name="ModelLens 0.2.33 更新", exact=True)
            expect(update).to_be_visible()
            assert page.evaluate("installCalls") == 0
            update.click()
            dialog = page.get_by_role("dialog", name="外观设置")
            install = dialog.get_by_role("button", name="安装并重启", exact=True)
            expect(install).to_be_enabled()
            install.click()
            expect(dialog.get_by_role("status")).to_contain_text("正在下载并验证")
            close = dialog.get_by_role("button", name="关闭", exact=True)
            expect(close).to_be_disabled()
            expect(dialog.get_by_role("button", name="管理 CLIProxy 连接")).to_be_disabled()
            page.keyboard.press("Escape")
            expect(dialog).to_be_visible()
            assert page.evaluate("installCalls") == 1
            page.evaluate("finishInstall()")
            expect(dialog.get_by_role("status")).to_contain_text("合成安装失败")
            expect(install).to_be_enabled()
            expect(close).to_be_enabled()
            close.click()
            expect(dialog).not_to_be_visible()
            page.goto(url + "?view=tray")
            expect(page.locator(".tray-app")).to_be_visible()
            assert page.evaluate("updateChecks") == 0
            assert not errors, errors
            print(f"{engine.name} update prompt, install guard, retry and tray isolation passed.")
