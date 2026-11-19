"""Run the shared publisher from the original Windows release entry point."""

from pathlib import Path
from runpy import run_path
import sys

if __name__ == "__main__":
    release = Path(__file__).resolve().parents[1] / "release"
    sys.path.insert(0, str(release))
    run_path(str(release / "publish.py"), run_name="__main__")
