"""Sign a Windows NSIS installer locally and publish it to Super Linux."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.request import urlopen

WINDOWS = Path(__file__).resolve().parent
SERVER = "root@124.221.252.165"
WEB_ROOT = "/var/www/html/modellens"
CONFIG = json.loads((WINDOWS / "tauri.conf.json").read_text())
MANIFEST_URL = CONFIG["plugins"]["updater"]["endpoints"][0]
BASE_URL = MANIFEST_URL.rsplit("/", 1)[0]


def digest(file):
    return hashlib.file_digest(file, "sha256").hexdigest()


def release_version(installer):
    with installer.open("rb") as file:
        if file.read(2) != b"MZ":
            raise ValueError("安装包不是 Windows 可执行文件")
    match = re.fullmatch(r"ModelLens_(\d+\.\d+\.\d+)_x64-setup\.exe", installer.name)
    if not match:
        raise ValueError("安装包文件名必须由 Tauri Windows x64 打包生成")
    version = match[1]
    if version != json.loads((WINDOWS.parent / "src-tauri/tauri.conf.json").read_text())["version"]:
        raise ValueError("安装包与源码版本不一致")
    return version


def publish(installer, notes):
    version = release_version(installer)
    key = Path(os.environ["TAURI_SIGNING_PRIVATE_KEY_PATH"]).expanduser().resolve()
    if key.with_suffix(".key.pub").read_text().strip() != CONFIG["plugins"]["updater"]["pubkey"]:
        raise ValueError("签名密钥的公钥与 Windows 更新配置不一致")
    subprocess.run(
        ["pnpm", "exec", "tauri", "signer", "sign", "-f", str(key), str(installer)],
        cwd=WINDOWS.parent, check=True, stdout=subprocess.DEVNULL,
    )
    signature = installer.with_suffix(".exe.sig")
    manifest = installer.parent / "latest.json"
    url = f"{BASE_URL}/releases/{version}/{installer.name}"
    manifest.write_text(json.dumps({
        "version": version,
        "pub_date": datetime.now(timezone.utc).isoformat(),
        "notes": notes,
        "platforms": {"windows-x86_64": {
            "url": url, "signature": signature.read_text().strip(),
        }},
    }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    checksums = installer.parent / "SHA256SUMS"
    with installer.open("rb") as file:
        sha256 = digest(file)
    checksums.write_text(f"{sha256}  {installer.name}\n", encoding="ascii")

    ssh = ["ssh", "-o", "BatchMode=yes", SERVER]
    subprocess.run([*ssh, "install", "-d", "-m", "755", f"{WEB_ROOT}/windows/releases"], check=True)
    stage = subprocess.check_output([*ssh, "mktemp", "-d", f"{WEB_ROOT}/.stage-windows-XXXXXXXX"], text=True).strip()
    if not re.fullmatch(re.escape(WEB_ROOT) + r"/\.stage-windows-[a-zA-Z0-9]{8}", stage):
        raise ValueError("服务器暂存目录无效")
    subprocess.run(["scp", "-o", "BatchMode=yes", installer, signature, manifest, checksums, f"{SERVER}:{stage}/"], check=True)
    with (WINDOWS / "promote.py").open("rb") as script:
        subprocess.run([*ssh, "python3", "-", stage, version, installer.name], stdin=script, check=True)

    with urlopen(MANIFEST_URL, timeout=30) as response:
        online = json.load(response)
    if online["version"] != version or online["platforms"] != json.loads(manifest.read_text(encoding="utf-8"))["platforms"]:
        raise RuntimeError("线上更新清单与发布内容不一致")
    with urlopen(url, timeout=120) as response:
        if digest(response) != sha256:
            raise RuntimeError("线上安装包的 SHA-256 不一致")
    print(url)
    print(f"SHA-256 {sha256}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installer", type=Path)
    parser.add_argument("--notes", required=True)
    args = parser.parse_args()
    publish(args.installer.resolve(), args.notes)
