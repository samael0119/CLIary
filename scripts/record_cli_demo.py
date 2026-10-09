#!/usr/bin/env python3
"""Capture real CLI output in an isolated demo workspace and render a GIF.

Requires Pillow and local DejaVu/Noto fonts. Does not read personal history.
"""
import argparse
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import struct
import subprocess
import termios
import time
import unicodedata

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory', required=True, type=Path)
parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/cliary')
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
root = args.directory.resolve(strict=True)
binary = args.binary.resolve(strict=True)
if not (root / 'synthetic.zsh_history').is_file():
    parser.error('Create an isolated workspace with seed_web_demo.py first.')
env = dict(os.environ, CLIARY_CONFIG_DIR=str(root / 'config'),
           CLIARY_DATA_DIR=str(root / 'data'), CLIARY_CACHE_DIR=str(root / 'cache'),
           TZ='Asia/Shanghai', TERM='xterm', NO_COLOR='1', COLUMNS='92', LINES='24')
commands = [('search', '磁盘空间'), ('show', 'ncdu'), ('compare', 'ncdu', 'dust')]
titles = ['按任务搜索工具', '查看速查指令与安装方式', '横向比较工具特性']
scenes = []
for command, title in zip(commands, titles):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 92, 0, 0))
    proc = subprocess.Popen([str(binary), '--lang', 'zh-CN', *command], env=env,
                            stdin=subprocess.DEVNULL, stdout=slave, stderr=slave)
    os.close(slave)
    data = bytearray()
    deadline = time.monotonic() + 15
    try:
        while True:
            if time.monotonic() > deadline:
                raise TimeoutError('Demo command exceeded 15 seconds')
            if not select.select([master], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            data.extend(chunk)
        if proc.wait(timeout=2) != 0:
            raise RuntimeError(data.decode('utf-8', errors='replace'))
    finally:
        os.close(master)
        if proc.poll() is None:
            proc.kill()
            proc.wait()
    output = data.decode('utf-8').replace('\r\n', '\n')
    # Remove terminal colors and OSC links, preserving the actual text output.
    output = re.sub(r'\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)', '', output)
    output = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', output)
    if '\x1b' in output:
        raise ValueError('Unsupported terminal escape in demo output')
    scenes.append(dict(title=title, command='cliary --lang zh-CN ' + shlex.join(command),
                       output=output))

ascii_font = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf', 20)
cjk_font = ImageFont.truetype('/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc', 20, index=2)
frames, durations = [], []

def render(lines, title, duration):
    image = Image.new('RGB', (1152, 736), '#121a20')
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 0, 1152, 49), fill='#1f2b34')
    draw.text((24, 10), 'CLIary  /  ' + title, font=cjk_font, fill='#dce5e9')
    for row, line in enumerate(lines[-24:]):
        x = 24
        color = '#6ee7b7' if line.startswith('$ ') else '#dce5e9'
        for char in line:
            wide = unicodedata.east_asian_width(char) in ('W', 'F')
            draw.text((x, 62 + row * 26), char, font=cjk_font if ord(char) > 127 else ascii_font,
                      fill=color)
            x += 24 if wide else 12
            if x > 1128:
                raise ValueError('Output exceeds the recorded terminal width')
    draw.line((24, 697, 1128, 697), fill='#33434f')
    draw.text((24, 707), '独立演示环境 · 人工使用历史 · 安装状态尚未扫描', font=cjk_font, fill='#a5b6c0')
    frames.append(image)
    durations.append(duration)

for scene in scenes:
    prompt = '$ ' + scene['command']
    render(['$ '], scene['title'], 400)
    for length in range(5, len(prompt), 4):
        render([prompt[:length]], scene['title'], 140)
    render([prompt], scene['title'], 450)
    lines = scene['output'].strip('\n').splitlines()
    if len(lines) > 23:
        render([prompt, *lines[:23]], scene['title'], 8500)
    render([prompt, *lines], scene['title'], 7500 if len(lines) > 23 else 6500)
    render([prompt, *lines], scene['title'], 450)

args.output.parent.mkdir(parents=True, exist_ok=True)
frames[0].save(args.output, save_all=True, append_images=frames[1:], duration=durations,
               loop=0, optimize=True, disposal=1)
args.output.with_suffix('.json').write_text(json.dumps(dict(
    columns=92, rows=24, synthetic_history=True, timing='Edited playback, not a benchmark',
    scenes=scenes), ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'{args.output}: {len(frames)} frames, {sum(durations) / 1000:.1f}s')
