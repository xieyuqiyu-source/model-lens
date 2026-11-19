"""Sign a desktop update locally and publish it to Super Linux."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess
import tarfile
from urllib.request import urlopen

from promote import PLATFORMS, semver

RELEASE = Path(__file__).resolve().parent
PROJECT = RELEASE.parent
SERVER = "root@124.221.252.165"
WEB_ROOT = "/var/www/html/modellens"

CONFIG = {
    "windows": PROJECT / "windows/tauri.conf.json",
    "macos": PROJECT / "src-tauri/tauri.macos.conf.json",
}


def digest(file):
    return hashlib.file_digest(file, "sha256").hexdigest()


def release_target(installer):
    config = json.loads((PROJECT / "src-tauri/tauri.conf.json").read_text())
    version = config["version"]
    semver(version)
    for platform, (target, template) in PLATFORMS.items():
        if installer.name == template.format(version):
            break
    else:
        raise ValueError("安装包文件名与当前发布版本不一致")
    if platform == "windows":
        with installer.open("rb") as file:
            if file.read(2) != b"MZ":
                raise ValueError("安装包不是 Windows 可执行文件")
    else:
        with tarfile.open(installer, "r:gz") as archive:
            with archive.extractfile("ModelLens.app/Contents/Info.plist") as file:
                info = plistlib.load(file)
            if info["CFBundleShortVersionString"] != version or info["CFBundleIdentifier"] != config["identifier"]:
                raise ValueError("Mac 安装包的应用标识或版本与源码不一致")
            executable = f"ModelLens.app/Contents/MacOS/{info['CFBundleExecutable']}"
            with archive.extractfile(executable) as file:
                # Little-endian 64-bit Mach-O header and CPU_TYPE_ARM64.
                if file.read(8) != bytes.fromhex("cffaedfe0c000001"):
                    raise ValueError("安装包不是 Apple Silicon 应用")
    return platform, target, version


def publish(installer, notes):
    platform, target, version = release_target(installer)
    updater = json.loads(CONFIG[platform].read_text())["plugins"]["updater"]
    manifest_url = updater["endpoints"][0]
    base_url = manifest_url.rsplit("/", 1)[0]
    key = Path(os.environ["TAURI_SIGNING_PRIVATE_KEY_PATH"]).expanduser().resolve()
    if key.with_name(f"{key.name}.pub").read_text().strip() != updater["pubkey"]:
        raise ValueError("签名密钥的公钥与更新配置不一致")
    signing_env = os.environ.copy()
    signing_env.setdefault("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "")
    subprocess.run(
        ["pnpm", "exec", "tauri", "signer", "sign", "-f", str(key), str(installer)],
        cwd=PROJECT, check=True, env=signing_env,
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
    )
    signature = installer.with_name(f"{installer.name}.sig")
    manifest = installer.parent / "latest.json"
    url = f"{base_url}/releases/{version}/{installer.name}"
    manifest.write_text(json.dumps({
        "version": version,
        "pub_date": datetime.now(timezone.utc).isoformat(),
        "notes": notes,
        "platforms": {target: {
            "url": url, "signature": signature.read_text().strip(),
        }},
    }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    checksums = installer.parent / "SHA256SUMS"
    with installer.open("rb") as file:
        sha256 = digest(file)
    checksums.write_text(f"{sha256}  {installer.name}\n", encoding="ascii")

    ssh = ["ssh", "-o", "BatchMode=yes", SERVER]
    subprocess.run([*ssh, "install", "-d", "-m", "755", f"{WEB_ROOT}/{platform}/releases"], check=True)
    stage = subprocess.check_output([*ssh, "mktemp", "-d", f"{WEB_ROOT}/.stage-{platform}-XXXXXXXX"], text=True).strip()
    if not re.fullmatch(rf"{re.escape(WEB_ROOT)}/\.stage-{platform}-[a-zA-Z0-9]{{8}}", stage):
        raise ValueError("服务器暂存目录无效")
    subprocess.run(["scp", "-o", "BatchMode=yes", installer, signature, manifest, checksums, f"{SERVER}:{stage}/"], check=True)
    with (RELEASE / "promote.py").open("rb") as script:
        published = subprocess.check_output([*ssh, "python3", "-", stage, platform, version], stdin=script)

    with urlopen(manifest_url, timeout=30) as response:
        online = json.load(response)
    update = online["platforms"][target]
    if online != json.loads(published) or online["version"] != version or update["url"] != url:
        raise RuntimeError("线上更新清单与发布内容不一致")
    with urlopen(url, timeout=120) as response:
        if digest(response) != sha256:
            raise RuntimeError("线上安装包的 SHA-256 不一致")
    manifest.write_bytes(published)
    signature.write_text(update["signature"], encoding="ascii")
    print(url)
    print(f"SHA-256 {sha256}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installer", type=Path)
    parser.add_argument("--notes", required=True)
    args = parser.parse_args()
    publish(args.installer.resolve(), args.notes)


if __name__ == "__main__":
    main()
