"""Publish a verified staging directory on Super Linux via an atomic manifest swap."""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

PLATFORMS = {
    "windows": ("windows-x86_64", "ModelLens_{}_x64-setup.exe"),
    "macos": ("darwin-aarch64", "ModelLens_{}_aarch64.app.tar.gz"),
}


def semver(version):
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)", version):
        raise ValueError("版本必须为正式的 x.y.z")
    return tuple(map(int, version.split(".")))


def promote(stage, platform, version):
    import fcntl

    target_name, installer_name = PLATFORMS[platform]
    installer = installer_name.format(version)
    current = stage.parent / platform / "latest.json"
    destination = current.parent / "releases" / version
    signature = f"{installer}.sig"
    assets = (installer, signature, "SHA256SUMS")
    target = semver(version)
    manifest = (stage / "latest.json").read_bytes()
    info = json.loads(manifest)
    update = info["platforms"][target_name]
    url = f"https://www.ccoos.cn/modellens/{platform}/releases/{version}/{installer}"
    if info["version"] != version or update["url"] != url or update["signature"] != (stage / signature).read_text().strip():
        raise ValueError("暂存更新清单与安装包签名不一致")
    subprocess.run(["sha256sum", "-c", "--strict", "SHA256SUMS"], cwd=stage, check=True, stdout=subprocess.DEVNULL)

    with open(f"/run/lock/modellens-{platform}.lock", "a", encoding="ascii") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if current.exists() and semver(json.loads(current.read_text())["version"]) > target:
            raise ValueError("发布版本低于线上版本")
        if destination.exists():
            if (stage / installer).read_bytes() != (destination / installer).read_bytes():
                raise ValueError("同版本安装包已存在，请增加版本号")
            # A retry can produce a new signature timestamp. The published assets stay immutable.
            update["signature"] = (destination / signature).read_text().strip()
            manifest = (json.dumps(info, ensure_ascii=False, indent=2) + "\n").encode()
        else:
            release = stage / "release"
            release.mkdir(mode=0o755)
            for file in assets:
                (stage / file).chmod(0o644)
                (stage / file).rename(release / file)
            release.rename(destination)
        with tempfile.NamedTemporaryFile(dir=current.parent, prefix=".latest-", delete=False) as new:
            try:
                new.write(manifest)
                new.flush()
                os.fsync(new.fileno())
                os.fchmod(new.fileno(), 0o644)
                os.replace(new.name, current)
            finally:
                Path(new.name).unlink(missing_ok=True)
    shutil.rmtree(stage)
    return manifest


if __name__ == "__main__":
    stage, platform, version = sys.argv[1:]
    if platform not in PLATFORMS or not re.fullmatch(rf"/var/www/html/modellens/\.stage-{platform}-[a-zA-Z0-9]{{8}}", stage):
        raise ValueError("暂存目录无效")
    sys.stdout.buffer.write(promote(Path(stage), platform, version))
