#!/usr/bin/env python3
"""Build the unsigned webOS .ipk and its Homebrew Channel manifest.

An .ipk is a Debian-style `ar` archive with `debian-binary`,
`control.tar.gz`, and `data.tar.gz`. Everything here is deterministic so the
SHA-256 in the manifest can be reproduced from the same commit and toolchain.
Only the Python standard library is required.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import shutil
import struct
import subprocess
import tarfile
import tempfile
from pathlib import Path

EPOCH = 1262304000  # 2010-01-01, accepted by old opkg builds
DEFAULT_REPO = "LovelessCodes/ArchivePlayer-webOS"
DEFAULT_BRANCH = "main"


def fail(message: str) -> "NoReturn":
    raise SystemExit(message)


def read_appinfo(repo: Path) -> dict:
    path = repo / "pkg" / "appinfo.json"
    try:
        app = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path}: {error}")
    for key in ("id", "version", "title", "type", "main"):
        if not isinstance(app.get(key), str) or not app[key]:
            fail(f"{path} has no {key!r}")
    if app["type"] != "native":
        fail(f"{path} must use type 'native', not {app['type']!r}")
    parts = app["version"].split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        fail(f"{path} version must be three dot-decimal integers")
    return app


def validate_arm_elf(path: Path) -> None:
    try:
        data = path.read_bytes()[:20]
    except OSError as error:
        fail(f"cannot read ARM binary {path}: {error}")
    if len(data) < 20 or data[:4] != b"\x7fELF":
        fail(f"{path} is not an ELF binary")
    if data[4] != 1 or data[5] != 1:
        fail(f"{path} must be a 32-bit little-endian ELF")
    if struct.unpack_from("<H", data, 18)[0] != 0x28:
        fail(f"{path} is not an ARM ELF")


def deterministic_tar(root: Path, extra: dict[str, bytes] | None = None) -> bytes:
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as archive:
        for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
            relative = path.relative_to(root).as_posix()
            info = archive.gettarinfo(str(path), arcname=relative)
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = EPOCH
            if info.isdir():
                info.mode = 0o755
            else:
                info.mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
            if info.isfile():
                with path.open("rb") as handle:
                    archive.addfile(info, handle)
            else:
                archive.addfile(info)
        for name, blob in sorted((extra or {}).items()):
            info = tarfile.TarInfo(name)
            info.size = len(blob)
            info.mode = 0o644
            info.mtime = EPOCH
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            archive.addfile(info, io.BytesIO(blob))
    out = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=out, mtime=0, compresslevel=9) as gz:
        gz.write(raw.getvalue())
    return out.getvalue()


def ar_archive(members: list[tuple[str, bytes]]) -> bytes:
    """Write the bare member names appinstalld expects (not GNU's trailing '/')."""
    out = bytearray(b"!<arch>\n")
    for name, data in members:
        if len(name) > 16:
            fail(f"ar member name too long: {name}")
        header = (
            f"{name:<16}"
            f"{EPOCH:<12}"
            f"{0:<6}"
            f"{0:<6}"
            f"{0o100644:<8o}"
            f"{len(data):<10}"
            "`\n"
        ).encode("ascii")
        out += header
        out += data
        if len(data) % 2:
            out += b"\n"
    return bytes(out)


def control_file(app: dict, installed_size: int) -> bytes:
    lines = [
        f"Package: {app['id']}",
        f"Version: {app['version']}",
        "Section: multimedia",
        "Priority: optional",
        "Architecture: arm",
        f"Installed-Size: {installed_size}",
        "Maintainer: LovelessCodes <hello@loveless.codes>",
        f"Description: {app.get('appDescription', app['title'])}",
        "webOS-Package-Format-Version: 2",
        "webOS-Packager-Version: 1.0.0",
    ]
    return ("\n".join(lines) + "\n").encode("utf-8")


def packageinfo(app: dict) -> bytes:
    return (
        json.dumps(
            {
                "id": app["id"],
                "version": app["version"],
                "app": app["id"],
                "loc_name": app["title"],
                "package_format_version": 2,
                "vendor": app.get("vendor", ""),
                "services": [],
            },
            indent=4,
        )
        + "\n"
    ).encode("utf-8")


def stage(repo: Path, app: dict, binary: Path, staging: Path, strip: str | None) -> int:
    app_dir = staging / "usr" / "palm" / "applications" / app["id"]
    package_dir = staging / "usr" / "palm" / "packages" / app["id"]
    app_dir.mkdir(parents=True)
    package_dir.mkdir(parents=True)

    target = app_dir / app["main"]
    shutil.copyfile(binary, target)
    target.chmod(0o755)
    if strip:
        subprocess.run([strip, "--strip-unneeded", str(target)], check=True)
    validate_arm_elf(target)

    for name in ("appinfo.json", "icon.png", "largeIcon.png", "splash.png"):
        source = repo / "pkg" / name
        if source.is_file():
            shutil.copyfile(source, app_dir / name)
        elif name == "appinfo.json":
            fail(f"missing {source}")

    for name in ("LICENSE", "NOTICE"):
        source = repo / name
        if source.is_file():
            shutil.copyfile(source, app_dir / name)

    (package_dir / "packageinfo.json").write_bytes(packageinfo(app))
    installed_size = sum(path.stat().st_size for path in staging.rglob("*") if path.is_file())
    return installed_size


def verify(ipk: Path, app: dict, binary_name: str) -> None:
    data = ipk.read_bytes()
    if not data.startswith(b"!<arch>\n"):
        fail("the .ipk is not an ar archive")
    members = {}
    offset = 8
    while offset < len(data):
        header = data[offset : offset + 60]
        if len(header) != 60 or header[58:60] != b"`\n":
            fail("malformed ar member header")
        name = header[:16].decode("ascii").rstrip()
        size = int(header[48:58].decode("ascii").strip())
        body = data[offset + 60 : offset + 60 + size]
        members[name] = body
        offset += 60 + size + (size % 2)
    if sorted(members) != ["control.tar.gz", "data.tar.gz", "debian-binary"]:
        fail(f"unexpected ar members: {sorted(members)}")
    if members["debian-binary"] != b"2.0\n":
        fail("debian-binary must contain 2.0")

    with tarfile.open(fileobj=io.BytesIO(members["control.tar.gz"]), mode="r:gz") as archive:
        control = archive.extractfile("control")
        if control is None:
            fail("control.tar.gz has no control file")
        fields = {}
        for line in control.read().decode("utf-8").splitlines():
            key, _, value = line.partition(": ")
            fields[key] = value
    if fields.get("Package") != app["id"] or fields.get("Version") != app["version"]:
        fail("control package/version differs from appinfo.json")

    prefix = f"usr/palm/applications/{app['id']}/"
    with tarfile.open(fileobj=io.BytesIO(members["data.tar.gz"]), mode="r:gz") as archive:
        names = set(archive.getnames())
        required = {
            f"{prefix}appinfo.json",
            f"{prefix}{binary_name}",
            f"usr/palm/packages/{app['id']}/packageinfo.json",
        }
        missing = required - names
        if missing:
            fail(f"package is missing {sorted(missing)}")
        staged_appinfo = json.load(archive.extractfile(f"{prefix}appinfo.json"))
        if staged_appinfo.get("id") != app["id"] or staged_appinfo.get("version") != app["version"]:
            fail("staged appinfo.json differs from pkg/appinfo.json")
        info = json.load(archive.extractfile(f"usr/palm/packages/{app['id']}/packageinfo.json"))
        if info.get("app") != app["id"] or info.get("version") != app["version"]:
            fail("packageinfo.json differs from appinfo.json")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="ARMv7 ELF binary to package")
    parser.add_argument("--output", type=Path, default=Path("dist"), help="artifact directory")
    parser.add_argument("--repo", default=DEFAULT_REPO, help="GitHub owner/name for URLs")
    parser.add_argument("--branch", default=DEFAULT_BRANCH, help="branch used for the icon URL")
    parser.add_argument("--strip", help="strip tool to run on the staged binary")
    parser.add_argument("--expect-version", help="fail unless appinfo.json has this version")
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    app = read_appinfo(repo)
    if args.expect_version and app["version"] != args.expect_version:
        fail(f"tag version {args.expect_version!r} does not match appinfo.json {app['version']!r}")

    binary = args.binary or (
        repo
        / "target"
        / "armv7-unknown-linux-gnueabi"
        / "release"
        / "archiveplayer-webos-sim"
    )
    binary = binary.resolve()
    validate_arm_elf(binary)
    if args.strip:
        strip = shutil.which(args.strip) or args.strip
    else:
        strip = None

    output = (repo / args.output).resolve() if not args.output.is_absolute() else args.output
    output.mkdir(parents=True, exist_ok=True)
    ipk_name = f"{app['id']}_{app['version']}_arm.ipk"
    ipk_path = output / ipk_name

    with tempfile.TemporaryDirectory(prefix="archiveplayer-ipk-") as temporary:
        staging = Path(temporary) / "data"
        installed_size = stage(repo, app, binary, staging, strip)
        control = control_file(app, installed_size)
        control_root = Path(temporary) / "control"
        control_root.mkdir()
        members = [
            ("debian-binary", b"2.0\n"),
            ("control.tar.gz", deterministic_tar(control_root, {"control": control})),
            ("data.tar.gz", deterministic_tar(staging)),
        ]
        ipk_path.write_bytes(ar_archive(members))

    verify(ipk_path, app, app["main"])
    digest = hashlib.sha256(ipk_path.read_bytes()).hexdigest()
    manifest = {
        "id": app["id"],
        "version": app["version"],
        "type": "native",
        "title": app["title"],
        "appDescription": app.get("appDescription", app["title"]),
        "iconUri": f"https://raw.githubusercontent.com/{args.repo}/{args.branch}/pkg/icon.png",
        "sourceUrl": f"https://github.com/{args.repo}",
        "rootRequired": False,
        "ipkUrl": ipk_name,
        "ipkHash": {"sha256": digest},
        "ipkSize": ipk_path.stat().st_size,
        "installedSize": installed_size,
    }
    manifest_path = output / f"{app['id']}.manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    (output / f"{ipk_name}.sha256").write_text(f"{digest}  {ipk_name}\n", encoding="utf-8")

    print(f"package:  {ipk_path} ({ipk_path.stat().st_size} bytes)")
    print(f"sha256:   {digest}")
    print(f"manifest: {manifest_path}")


if __name__ == "__main__":
    main()
