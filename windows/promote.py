"""Publish a verified staging directory on Super Linux via an atomic manifest swap."""

import fcntl
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


def semver(version):
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)", version):
        raise ValueError("版本必须为正式的 x.y.z")
    return tuple(map(int, version.split(".")))


def promote(stage, version, installer):
    current = stage.parent / "windows" / "latest.json"
    destination = current.parent / "releases" / version
    signature = f"{installer}.sig"
    assets = (installer, signature, "SHA256SUMS")
    target = semver(version)
    manifest = (stage / "latest.json").read_bytes()
    info = json.loads(manifest)
    if info["version"] != version or info["platforms"]["windows-x86_64"]["signature"] != (stage / signature).read_text().strip():
        raise ValueError("暂存更新清单与安装包签名不一致")
    subprocess.run(["sha256sum", "-c", "--strict", "SHA256SUMS"], cwd=stage, check=True)

    with open("/run/lock/modellens-windows.lock", "a", encoding="ascii") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if current.exists() and semver(json.loads(current.read_text())["version"]) > target:
            raise ValueError("发布版本低于线上版本")
        if destination.exists():
            if not all((stage / file).read_bytes() == (destination / file).read_bytes() for file in assets):
                raise ValueError("同版本安装包已存在，请增加版本号")
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


if __name__ == "__main__":
    stage, version, installer = sys.argv[1:]
    if not re.fullmatch(r"/var/www/html/modellens/\.stage-windows-[a-zA-Z0-9]{8}", stage):
        raise ValueError("暂存目录无效")
    semver(version)
    if installer != f"ModelLens_{version}_x64-setup.exe":
        raise ValueError("暂存安装包名称无效")
    promote(Path(stage), version, installer)
