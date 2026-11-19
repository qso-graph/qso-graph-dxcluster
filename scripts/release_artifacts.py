#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Stage unsigned files and assemble CLI archives after the shared signing jobs.

Only stage reads target/. Archive reads only the downloaded (possibly signed) files.
"""
import shutil
import sys
import tarfile
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLATFORMS = {"macos-arm64", "linux-x86_64", "linux-arm64", "windows-x86_64"}


def version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


def names(platform):
    if platform not in PLATFORMS:
        raise SystemExit(f"Unknown platform: {platform}")
    return "qg-dxc.exe" if platform == "windows-x86_64" else "qg-dxc"


def expected(platform):
    return {"version.txt", f"cli/{names(platform)}", "cli/README.md", "cli/LICENSE"}


def stage(platform, target, output):
    exe = names(platform)
    output = Path(output)
    if output.exists():
        raise SystemExit(f"Stage directory already exists: {output}")
    (output / "cli").mkdir(parents=True)
    (output / "version.txt").write_text(version() + "\n")
    shutil.copy2(ROOT / "target" / target / "release" / exe, output / "cli" / exe)
    shutil.copy2(ROOT / "crates/qso-graph-dxcluster-cli/README.md", output / "cli/README.md")
    shutil.copy2(ROOT / "LICENSE", output / "cli/LICENSE")


def archive(platform, source, output):
    exe = names(platform)
    source, output = Path(source), Path(output)
    actual = {p.relative_to(source).as_posix() for p in source.rglob("*") if p.is_file()}
    if actual != expected(platform) or source.is_symlink() or any(p.is_symlink() for p in source.rglob("*")):
        raise SystemExit(f"Wrong bundle contents: missing {expected(platform) - actual}; extra {actual - expected(platform)}")
    if (source / "version.txt").read_text() != version() + "\n":
        raise SystemExit("Artifact version differs from the checked-out source")
    # Actions artifacts preserve bytes, but not Unix execute permissions.
    (source / "cli" / exe).chmod(0o755)
    output.mkdir(parents=True, exist_ok=True)
    base = f"qg-dxc-{version()}-{platform}"
    files = sorted(p for p in (source / "cli").rglob("*") if p.is_file())
    if platform == "windows-x86_64":
        path = output / f"{base}.zip"
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for file in files:
                bundle.write(file, Path(base) / file.relative_to(source / "cli"))
    else:
        path = output / f"{base}.tar.gz"
        with tarfile.open(path, "w:gz") as bundle:
            for file in files:
                bundle.add(file, arcname=str(Path(base) / file.relative_to(source / "cli")), recursive=False)
    print(path)


def main(argv):
    if len(argv) == 4 and argv[0] == "stage":
        stage(*argv[1:])
    elif len(argv) == 4 and argv[0] == "archive":
        archive(*argv[1:])
    else:
        raise SystemExit("release_artifacts.py stage PLATFORM TARGET OUTPUT | archive PLATFORM SOURCE OUTPUT")


if __name__ == "__main__":
    main(sys.argv[1:])
