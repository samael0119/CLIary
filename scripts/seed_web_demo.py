#!/usr/bin/env python3
"""Create an isolated, explicitly synthetic workspace for Web screenshots.

Refuses to overwrite an existing directory. Never scans or reads personal history.
"""
import argparse
from datetime import datetime
import os
from pathlib import Path
import subprocess
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory', required=True, type=Path)
parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/cliary')
args = parser.parse_args()
binary = args.binary.resolve(strict=True)
root = args.directory.resolve()
root.mkdir(parents=True, exist_ok=False)
env = dict(os.environ, CLIARY_CONFIG_DIR=str(root / 'config'),
           CLIARY_DATA_DIR=str(root / 'data'), CLIARY_CACHE_DIR=str(root / 'cache'),
           TZ='Asia/Shanghai')

def run(*command):
    subprocess.run([str(binary), *command], env=env, check=True, capture_output=True,
                   text=True, timeout=15)

run('internal', 'skip-scan')
run('internal', 'skip-history-guide')
run('config', 'set', 'language', 'zh-CN')
for tool in ('git', 'ncdu', 'jq', 'rg'):
    run('favorite', tool)
run('note', 'jq', '--set', '演示备注：查看和处理 JSON 数据。')

# Historical examples remain valid after this capture date; never guess live usage.
lines = []
tools = ('git', 'rg', 'jq', 'docker', 'ncdu', 'curl', 'fd', 'bat')
zone = ZoneInfo('Asia/Shanghai')
for year, months in ((2025, range(1, 13)), (2026, range(1, 10))):
    for month in months:
        for index, tool in enumerate(tools):
            count = max(2, 16 - index * 2 + month % 4)
            for event in range(count):
                stamp = int(datetime(year, month, 1 + event % 21, 10 + index, event, tzinfo=zone).timestamp())
                lines.append(f': {stamp}:0;{tool} --demo\n')
for day in range(1, 9):
    for index, tool in enumerate(tools[:5]):
        stamp = int(datetime(2026, 10, day, 12, index, tzinfo=zone).timestamp())
        lines.append(f': {stamp}:0;{tool} --demo\n')
fixture = root / 'synthetic.zsh_history'
fixture.write_text(''.join(lines), encoding='utf-8')
run('import-history', '--shell', 'zsh', '--file', str(fixture), '--apply')
print(f'Synthetic demo ready: {root}')
print(f'CLIARY_CONFIG_DIR={root}/config CLIARY_DATA_DIR={root}/data CLIARY_CACHE_DIR={root}/cache TZ=Asia/Shanghai {binary} web')
