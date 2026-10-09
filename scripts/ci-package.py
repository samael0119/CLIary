#!/usr/bin/env python3
"""Verify platform executables and prepare binary-only release artifacts."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

try:
    import tomllib as tomli
except ImportError:
    import tomli

TARGETS = {
    'x86_64-unknown-linux-musl': ('cliary-linux-x86_64', 62),
    'aarch64-unknown-linux-musl': ('cliary-linux-arm64', 183),
    'x86_64-apple-darwin': ('cliary-macos-x86_64', 0x01000007),
    'aarch64-apple-darwin': ('cliary-macos-arm64', 0x0100000C),
}


def inspect_binary(data, target):
    expected_arch = TARGETS[target][1]
    if 'linux' in target:
        if data[:6] != b'\x7fELF\x02\x01' or struct.unpack_from('<H', data, 18)[0] != expected_arch:
            raise ValueError(f'Wrong ELF architecture for {target}')
        return {'linkage': 'static musl', 'validation': 'Native installer tests' if target.startswith('x86_64') else 'QEMU version and search'}
    if len(data) < 32 or struct.unpack_from('<II', data)[0:2] != (0xFEEDFACF, expected_arch):
        raise ValueError(f'Wrong Mach-O architecture for {target}')
    commands = struct.unpack_from('<I', data, 16)[0]
    offset = 32
    libraries = []
    minimum_os = None
    for _ in range(commands):
        if offset + 8 > len(data):
            raise ValueError('Truncated Mach-O load command')
        command, size = struct.unpack_from('<II', data, offset)
        if size < 8 or offset + size > len(data):
            raise ValueError('Invalid Mach-O load command')
        if command == 0x32:  # LC_BUILD_VERSION
            if size < 24:
                raise ValueError('Truncated macOS build version')
            platform, version = struct.unpack_from('<II', data, offset + 8)
            if platform != 1:
                raise ValueError('Mach-O does not target macOS')
            minimum_os = f'{version >> 16}.{(version >> 8) & 255}.{version & 255}'
        elif command == 0x24:  # LC_VERSION_MIN_MACOSX
            if size < 16:
                raise ValueError('Truncated macOS version')
            version = struct.unpack_from('<I', data, offset + 8)[0]
            minimum_os = f'{version >> 16}.{(version >> 8) & 255}.{version & 255}'
        elif command in (0xC, 0x80000018, 0x8000001F):
            if size < 24:
                raise ValueError('Truncated macOS library command')
            name_offset = struct.unpack_from('<I', data, offset + 8)[0]
            if name_offset < 24 or name_offset >= size:
                raise ValueError('Invalid macOS library name offset')
            name = data[offset + name_offset:offset + size].split(b'\0')[0].decode()
            if not name.startswith(('/usr/lib/', '/System/Library/')):
                raise ValueError(f'Non-system macOS dependency: {name}')
            libraries.append(name)
        offset += size
    if minimum_os != '13.0.0':
        raise ValueError(f'Unexpected minimum macOS version: {minimum_os}')
    return {'minimum_macos': minimum_os, 'system_libraries': libraries,
            'validation': 'Cross compiled; Mach-O architecture, minimum OS and system libraries checked; not run on a Mac'}


def main():
    destination = Path('dist')
    reports = Path('.ci-reports')
    if sys.argv[1] != '--finalize':
        target = sys.argv[1]
        asset = TARGETS[target][0]
        data = (Path('target') / target / 'release/cliary').read_bytes()
        details = inspect_binary(data, target)
        binary = destination / asset
        binary.write_bytes(data)
        binary.chmod(0o755)
        details.update(asset=asset, bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
        (reports / f'{target}.json').write_text(json.dumps(details, indent=2) + '\n')
        print(f'Verified {asset}: {len(data)} bytes')
        return
    with Path('Cargo.toml').open('rb') as stream:
        version = tomli.load(stream)['workspace']['package']['version']
    platforms = {target: json.loads((reports / f'{target}.json').read_text())
                 for target in TARGETS if (reports / f'{target}.json').exists()}
    info = {'version': version, 'source_commit': os.environ['GITHUB_SHA'],
            'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
            'cargo_zigbuild': '0.23.4', 'zig': '0.16.0',
            'memory_max_bytes': 500 * 1024**2, 'swap_max_bytes': 1024**3,
            'work_disk_bytes': 5 * 1024**3, 'platforms': platforms}
    (reports / 'BUILDINFO.json').write_text(json.dumps(info, indent=2) + '\n')
    notes = [f'CLIary {version} 自动构建（预发布）', '', f'源码提交：{info["source_commit"]}', '',
             '直接下载对应平台的二进制，设置执行权限后运行。Linux 为 musl 静态包。',
             'macOS 需要 13.0 或更新版本；交叉编译包尚未经过 Mac 实机验收。' if any('apple' in p for p in platforms) else '',
             '', 'SHA-256：', '', '```text']
    notes += [f'{p["sha256"]}  {p["asset"]}' for p in platforms.values()]
    notes += ['```', '']
    (reports / 'release-notes.md').write_text('\n'.join(notes))


if __name__ == '__main__':
    main()
