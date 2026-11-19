"""Install and run the Windows package on a disposable Windows runner."""

import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import subprocess
from threading import Thread
import time
from urllib.request import urlopen

from playwright.sync_api import Error, expect, sync_playwright


class MockCpa(BaseHTTPRequestHandler):
    def do_GET(self):
        responses = {
            "/v8/management/credentials": {"files": [{
                "name": "test.json", "email": "windows@example.invalid", "provider": "openai",
                "auth_index": "test", "disabled": False,
            }]},
            "/v8/management/config/access/api-keys": ["synthetic-client-key"],
            "/v1/models": {"data": [{"id": "synthetic-model", "owned_by": "openai"}]},
        }
        key = "synthetic-client-key" if self.path == "/v1/models" else "synthetic-management-key"
        if self.headers.get("Authorization") != f"Bearer {key}":
            self.send_error(401)
        elif self.path not in responses:
            self.send_error(404)
        else:
            body = json.dumps(responses[self.path]).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)


@contextmanager
def webview_debug_port(port):
    import winreg

    policy = r"Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments"
    executable = "model-lens.exe"
    with winreg.CreateKeyEx(winreg.HKEY_LOCAL_MACHINE, policy, access=winreg.KEY_READ | winreg.KEY_WRITE) as key:
        try:
            previous = winreg.QueryValueEx(key, executable)
        except FileNotFoundError:
            previous = None
        # Elevated WebView2 hosts ignore user-level debugger overrides.
        winreg.SetValueEx(key, executable, 0, winreg.REG_SZ, f"--remote-debugging-port={port}")
        try:
            yield
        finally:
            if previous:
                value, kind = previous
                winreg.SetValueEx(key, executable, 0, kind, value)
            else:
                winreg.DeleteValue(key, executable)


def version(page):
    return page.evaluate("() => window.__TAURI_INTERNALS__.invoke('plugin:app|version')")


def attach(playwright, endpoint, expected, process=None):
    deadline = time.monotonic() + 120
    last_error = "未发现符合版本的主窗口"
    while time.monotonic() < deadline:
        if process is not None and process.poll() is not None:
            raise AssertionError(f"ModelLens 在启动时退出，退出码 {process.returncode}")
        try:
            with urlopen(f"{endpoint}/json/version", timeout=2) as response:
                debugger = json.load(response)["webSocketDebuggerUrl"]
            browser = playwright.chromium.connect_over_cdp(debugger, timeout=2000, no_defaults=True)
            for context in browser.contexts:
                for page in context.pages:
                    if page.url != "about:blank" and "view=tray" not in page.url and version(page) == expected:
                        return browser, page
            browser.close()
        except (OSError, Error, KeyError) as error:
            last_error = str(error)
        time.sleep(1)
    raise AssertionError(f"Windows 应用未进入版本 {expected}，最后一次检查：{last_error}")


def settings(page):
    page.get_by_role("button", name="外观与连接设置").click()
    return page.get_by_role("dialog", name="外观设置")


def verify_ui(browser, page, port):
    title = page.locator(".titlebar")
    title.get_by_role("button", name="连接 CLIProxy", exact=True).click()
    page.get_by_role("spinbutton", name="管理端口").fill(str(port))
    page.get_by_label("管理密钥", exact=True).fill("synthetic-management-key")
    page.get_by_role("button", name="连接并读取列表").click()
    expect(title.get_by_role("button", name="CLIProxy 已连接")).to_be_visible()

    page.get_by_role("button", name="凭证优先", exact=True).click()
    expect(page.get_by_text("windows@example.invalid", exact=True).first).to_be_visible()
    page.get_by_role("button", name="复制 API Key", exact=True).click()
    expect(page.get_by_text("API Key 已复制", exact=True)).to_be_visible()
    clip = subprocess.check_output(["pwsh", "-NoProfile", "-Command", "Get-Clipboard -Raw"], text=True)
    assert clip.strip() == "synthetic-client-key", "Windows 剪贴板内容与合成 Key 不同"

    page.get_by_role("button", name="探查优先", exact=True).click()
    expect(page.get_by_role("button", name="选择模型：synthetic-model", exact=True)).to_be_visible()
    page.get_by_role("button", name="系统托盘快捷入口").click()
    tray = next(p for context in browser.contexts for p in context.pages if "view=tray" in p.url)
    expect(tray.get_by_role("button", name="打开主窗口")).to_be_visible()
    tray.get_by_role("button", name="打开主窗口").click()


def main(installer, expected_update):
    expected = installer.name.removeprefix("ModelLens_").split("_", maxsplit=1)[0]
    directory = Path(os.environ["RUNNER_TEMP"]) / "ModelLensSmoke"
    subprocess.run([str(installer), "/S", f"/D={directory}"], check=True, timeout=180)
    executable = directory / "model-lens.exe"
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={port}"
    endpoint = f"http://127.0.0.1:{port}"

    with webview_debug_port(port), ThreadingHTTPServer(("127.0.0.1", 0), MockCpa) as server, sync_playwright() as playwright:
        Thread(target=server.serve_forever, daemon=True).start()
        process = subprocess.Popen([str(executable)])
        try:
            browser, page = attach(playwright, endpoint, expected, process)
            verify_ui(browser, page, server.server_port)
            print(f"Windows {expected} 安装成功，CLIProxy、模型目录、剪贴板和托盘已核验。", flush=True)
            dialog = settings(page)
            expect(dialog.get_by_role("button", name="检查应用更新")).to_be_visible()
            if expected_update:
                install = dialog.get_by_role("button", name="安装并重启")
                expect(dialog.get_by_text(f"发现 ModelLens {expected_update}", exact=False)).to_be_visible(timeout=30000)
                install.click(no_wait_after=True)
                assert process.wait(timeout=120) == 0, "旧版 ModelLens 未正常退出"
                browser, new = attach(playwright, endpoint, expected_update)
                expect(new.locator(".titlebar").get_by_role("button", name="CLIProxy 已连接")).to_be_visible(timeout=15000)
                expect(settings(new).get_by_text("ModelLens 已是最新版本", exact=True)).to_be_visible(timeout=30000)
                print(f"签名更新 {expected} → {expected_update} 完成，已重启并恢复合成连接。")
            browser.close()
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=15)
            server.shutdown()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installer", type=Path)
    parser.add_argument("--expect-update")
    args = parser.parse_args()
    main(args.installer.resolve(), args.expect_update)
