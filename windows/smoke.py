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


def sample_text(slot):
    slot %= 3
    return ", ".join(str((i * (17 + slot * 6) + slot * 41) % 355 + 1) for i in range(300))


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
            self.respond(responses[self.path])

    def do_POST(self):
        if self.path not in ("/v1/chat/completions", "/v1/responses", "/v1/messages"):
            return self.send_error(404)
        if self.path == "/v1/messages":
            if self.headers.get("x-api-key") != "synthetic-client-key" or self.headers.get("anthropic-version") != "2023-06-01":
                return self.send_error(401)
        elif self.headers.get("Authorization") != "Bearer synthetic-client-key":
            return self.send_error(401)
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        prompt = request["input"] if self.path == "/v1/responses" else request["messages"][0]["content"]
        if request.get("model") != "synthetic-model" or not prompt:
            return self.send_error(400)
        answer = sample_text(self.server.samples)
        self.server.samples += 1
        if self.path == "/v1/responses":
            return self.respond({"model": "synthetic-model", "status": "completed", "output": [{
                "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": answer}],
            }]})
        if self.path == "/v1/messages":
            return self.respond({"model": "synthetic-model", "stop_reason": "end_turn",
                                 "content": [{"type": "text", "text": answer}]})
        self.respond({"model": "synthetic-model", "choices": [{
            "message": {"content": answer}, "finish_reason": "stop",
        }]})

    def respond(self, value):
        body = json.dumps(value).encode()
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
    page.get_by_role("button", name="管理连接", exact=True).click()
    page.get_by_role("spinbutton", name="管理端口").fill(str(port))
    page.get_by_label("管理密钥", exact=True).fill("synthetic-management-key")
    page.get_by_role("button", name="连接并读取列表").click()

    page.get_by_role("button", name="凭证优先", exact=True).click()
    expect(page.locator(".credential-card").get_by_text("windows@example.invalid", exact=True)).to_be_visible()
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


def verify_probe(page, server):
    probe = page.get_by_role("region", name="模型探查")
    result = probe.get_by_role("article", name="检测结果")
    start = probe.get_by_role("button", name="开始检测", exact=True)
    start.click()
    expect(result).to_contain_text("3 / 3 有效样本", timeout=30000)
    assert server.samples == 3

    probe.get_by_role("button", name="手动检测", exact=True).click()
    probe.get_by_role("textbox", name="模型回答 1", exact=True).fill(sample_text(0))
    probe.get_by_role("button", name="开始评分", exact=True).click()
    expect(result).to_contain_text("1 / 3 有效样本", timeout=30000)
    assert server.samples == 3

    probe.get_by_role("button", name="自动检测", exact=True).click()
    probe.get_by_role("button", name="第三方 API", exact=True).click()
    probe.get_by_role("button", name="添加第三方 API", exact=True).click()
    form = page.get_by_role("dialog", name="添加第三方 API")
    form.get_by_label("Base URL", exact=True).fill(f"http://127.0.0.1:{server.server_port}/v1")
    form.get_by_label("API Key", exact=True).fill("synthetic-client-key")
    form.get_by_role("button", name="保存并读取模型", exact=True).click()
    expect(form).to_be_hidden(timeout=15000)
    start.click()
    expect(result).to_contain_text("2 / 3 有效样本", timeout=30000)
    assert server.samples == 5
    result.get_by_role("button", name="ModelTrace 对照").click()
    expect(result).to_contain_text("同一批回答 · 独立排名")
    result.get_by_role("button", name="补测第 3 次").click()
    expect(result).to_contain_text("3 / 3 有效样本", timeout=30000)
    assert server.samples == 6
    for protocol, expected_count in [("Responses", 8), ("Anthropic Messages", 10)]:
        probe.get_by_role("button", name="编辑第三方 API").click()
        form = page.get_by_role("dialog", name="编辑第三方 API")
        form.get_by_role("button", name="接口协议：", exact=False).click()
        form.get_by_role("option", name=protocol, exact=True).click()
        form.get_by_label("模型 ID", exact=True).fill("synthetic-model")
        form.get_by_role("button", name="保存配置", exact=True).click()
        expect(form).to_be_hidden(timeout=15000)
        start.click()
        expect(result).to_contain_text("2 / 3 有效样本", timeout=30000)
        assert server.samples == expected_count
    probe.get_by_role("button", name="CLIProxy", exact=True).click()
    print("Windows CLIProxy、手动评分、三种第三方 API、两次采样后补测及 ModelTrace 对照通过。", flush=True)


def git_fixture(repo, *args):
    subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                    "-C", str(repo), *args], check=True, stdout=subprocess.DEVNULL)


def prepare_fixture(root):
    upstream = root / "local-upstream"
    source = upstream / "cmd" / "server" / "main.go"
    source.parent.mkdir(parents=True)
    source.write_bytes((Path(__file__).parent / "fixtures" / "cpa.go").read_bytes())
    (upstream / "go.mod").write_text("module github.com/router-for-me/CLIProxyAPI/v8\n\ngo 1.22\n", encoding="ascii")
    git_fixture(upstream, "init", "-b", "main")
    git_fixture(upstream, "add", ".")
    git_fixture(upstream, "commit", "-m", "first")
    env = os.environ.copy()
    env.update({
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": f"url.{upstream.as_uri()}.insteadOf",
        "GIT_CONFIG_VALUE_0": "https://github.com/router-for-me/CLIProxyAPI.git",
        "GIT_CONFIG_KEY_1": "protocol.file.allow",
        "GIT_CONFIG_VALUE_1": "always",
    })
    return upstream, source, env


def verify_directory_picker(dialog, pid, directory):
    import ctypes
    from ctypes import wintypes

    user = ctypes.windll.user32
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = (callback, wintypes.LPARAM)
    user.EnumChildWindows.argtypes = (wintypes.HWND, callback, wintypes.LPARAM)
    user.GetClassNameW.argtypes = (wintypes.HWND, wintypes.LPWSTR, ctypes.c_int)
    user.GetWindowThreadProcessId.argtypes = (wintypes.HWND, ctypes.POINTER(wintypes.DWORD))
    user.IsWindowVisible.argtypes = (wintypes.HWND,)
    user.SendMessageW.argtypes = (wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
    user.PostMessageW.argtypes = user.SendMessageW.argtypes
    target = []

    def class_name(hwnd):
        name = ctypes.create_unicode_buffer(64)
        user.GetClassNameW(hwnd, name, len(name))
        return name.value

    @callback
    def find_picker(hwnd, _):
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != pid or not user.IsWindowVisible(hwnd) or class_name(hwnd) != "#32770":
            return True

        @callback
        def find_edit(control, _):
            if class_name(control) == "Edit" and user.IsWindowVisible(control):
                target.append((hwnd, control))
                return False
            return True

        user.EnumChildWindows(hwnd, find_edit, 0)
        return not target

    dialog.get_by_role("button", name="选择目录", exact=True).click(no_wait_after=True)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not target:
        user.EnumWindows(find_picker, 0)
        if not target:
            time.sleep(0.1)
    assert target, "Windows 原生目录选择器未显示"
    hwnd, edit = target[0]
    path = ctypes.create_unicode_buffer(str(directory))
    assert user.SendMessageW(edit, 0x000C, 0, ctypes.addressof(path))
    assert user.PostMessageW(hwnd, 0x0111, 1, 0)
    expect(dialog.get_by_role("textbox", name="CLIProxy 安装路径")).to_have_value(str(directory / "CLIProxy"), timeout=10000)


def verify_service(page, pid, root, upstream, source):
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    install_path = root / "CLIProxy Fixture"
    page.locator(".titlebar .service-status").click()
    dialog = page.get_by_role("dialog", name="本机 CLIProxy")
    dialog.get_by_role("button", name="安装 CLIProxy", exact=True).click()
    verify_directory_picker(dialog, pid, root)
    dialog.get_by_role("spinbutton", name="CPA 端口").fill(str(port))
    dialog.get_by_role("textbox", name="CLIProxy 安装路径").fill(str(install_path))
    dialog.get_by_role("button", name="安装并启动", exact=True).click()
    expect(dialog.get_by_text("CLIProxy 已安装、启动并保存本机管理连接", exact=True).filter(visible=True).first).to_be_visible(timeout=100000)
    assert (install_path / "CLIProxyApi" / ".git").is_dir()
    assert (install_path / ".dev" / "cli-proxy-api.exe").is_file()
    config = str(install_path / ".dev" / "config.yaml").replace("'", "''")
    subprocess.run(["pwsh", "-NoProfile", "-Command",
                    f"$acl = Get-Acl -LiteralPath '{config}'; "
                    "$user = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value; "
                    "$sids = $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]).IdentityReference.Value; "
                    "if ($user -notin $sids -or @($sids | Where-Object { $_ -ne $user -and $_ -ne 'S-1-5-18' }).Count) { exit 1 }"],
                   check=True)
    url = f"http://127.0.0.1:{port}/"
    with urlopen(url, timeout=2) as response:
        assert response.read() == b"first"

    executable = install_path / ".dev" / "cli-proxy-api.exe"
    snapshot = page.evaluate("() => window.__TAURI_INTERNALS__.invoke('discover_services')")
    print("CPA discovery:", snapshot, flush=True)
    location = dialog.locator(".service-location").filter(has=page.get_by_label(f"CPA 地址：{executable}", exact=True))
    expect(location).to_have_count(1, timeout=10000)
    if dialog.get_by_role("radio").count():
        location.get_by_role("radio").check()

    source.write_text(source.read_text().replace('"first"', '"second"'), encoding="utf-8")
    git_fixture(upstream, "add", ".")
    git_fixture(upstream, "commit", "-m", "second")
    versions = dialog.locator(".cpa-versions")
    versions.get_by_role("button", name="检查 CPA 版本").click()
    expect(versions.get_by_text("有更新 · 落后 1 个提交", exact=True)).to_be_visible(timeout=20000)
    versions.get_by_role("button", name="更新并重启", exact=True).click()
    expect(versions.get_by_text("上次更新成功", exact=False)).to_be_visible(timeout=90000)
    with urlopen(url, timeout=2) as response:
        assert response.read() == b"second"
    signal_file = install_path / ".dev" / "graceful-stop"
    assert signal_file.read_text() == "first"

    saved_config = install_path / ".dev" / "config.yaml"
    config_before = saved_config.read_bytes()
    source.write_text(source.read_text().replace('"second"', '"unstartable"'), encoding="utf-8")
    git_fixture(upstream, "add", ".")
    git_fixture(upstream, "commit", "-m", "unstartable")
    versions.get_by_role("button", name="检查 CPA 版本").click()
    expect(versions.get_by_text("有更新 · 落后 1 个提交", exact=True)).to_be_visible(timeout=20000)
    versions.get_by_role("button", name="更新并重启", exact=True).click()
    expect(dialog.get_by_text("已恢复原程序与配置并重新启动", exact=False).filter(visible=True).first).to_be_visible(timeout=90000)
    assert saved_config.read_bytes() == config_before
    with urlopen(url, timeout=2) as response:
        assert response.read() == b"second"

    dialog.get_by_role("button", name="关闭 CPA", exact=True).click()
    expect(dialog.get_by_role("button", name="启动 CPA", exact=True)).to_be_enabled(timeout=25000)
    assert signal_file.read_text() == "second"
    dialog.get_by_role("button", name="启动 CPA", exact=True).click()
    expect(dialog.get_by_role("button", name="关闭 CPA", exact=True)).to_be_enabled(timeout=25000)
    with urlopen(url, timeout=2) as response:
        assert response.read() == b"second"
    dialog.get_by_role("button", name="关闭 CPA", exact=True).click()
    expect(dialog.get_by_role("button", name="启动 CPA", exact=True)).to_be_enabled(timeout=25000)
    dialog.get_by_role("button", name="关闭", exact=True).click()
    print("Windows 源码克隆、编译、管理连接、版本检查、更新重启、失败恢复和正常启停通过。", flush=True)


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
    upstream, source, env = prepare_fixture(directory.parent / "ModelLens CPA Smoke") if not expected_update else (None, None, os.environ.copy())

    with webview_debug_port(port), ThreadingHTTPServer(("127.0.0.1", 0), MockCpa) as server, sync_playwright() as playwright:
        server.samples = 0
        Thread(target=server.serve_forever, daemon=True).start()
        process = subprocess.Popen([str(executable)], env=env)
        try:
            browser, page = attach(playwright, endpoint, expected, process)
            verify_ui(browser, page, server.server_port)
            print(f"Windows {expected} 安装成功，CLIProxy、模型目录、剪贴板和托盘已核验。", flush=True)
            if not expected_update:
                verify_probe(page, server)
                verify_service(page, process.pid, upstream.parent, upstream, source)
            dialog = settings(page)
            expect(dialog.get_by_role("button", name="检查应用更新")).to_be_visible()
            if expected_update:
                install = dialog.get_by_role("button", name="安装并重启")
                expect(dialog.get_by_text(f"发现 ModelLens {expected_update}", exact=False)).to_be_visible(timeout=30000)
                install.click(no_wait_after=True)
                assert process.wait(timeout=120) == 0, "旧版 ModelLens 未正常退出"
                browser, new = attach(playwright, endpoint, expected_update)
                expect(new.get_by_role("button", name="选择模型：synthetic-model", exact=True)).to_be_visible(timeout=15000)
                verify_probe(new, server)
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
