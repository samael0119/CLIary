#!/usr/bin/env python3
"""Isolated native installer smoke tests; build cliary before running.

CLIARY_TEST_BINARY selects debug/release without modifying the user's installation.
No real GitHub downloads, shell history or shell startup files are used.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get('CLIARY_TEST_BINARY', ROOT / 'target/debug/cliary')).resolve()
TARGETS = {
    ('Linux', 'x86_64'): 'x86_64-unknown-linux-musl',
    ('Linux', 'aarch64'): 'aarch64-unknown-linux-musl',
    ('Darwin', 'x86_64'): 'x86_64-apple-darwin',
    ('Darwin', 'arm64'): 'aarch64-apple-darwin',
}


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cliary-installer-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.assets = self.root / 'release assets'
        self.assets.mkdir()
        target = TARGETS[(os.uname().sysname, os.uname().machine)]
        self.archive = self.assets / f'cliary-{target}.tar.gz'
        with tarfile.open(self.archive, 'w:gz') as archive:
            archive.add(BINARY, arcname='cliary')
        self.write_checksums()
        self.env = dict(os.environ)
        for key in ('GH_TOKEN', 'GITHUB_TOKEN', 'CLIARY_RELEASE_TAG'):
            self.env.pop(key, None)
        self.env.update({
            'CLIARY_CONFIG_DIR': str(self.root / 'config'),
            'CLIARY_DATA_DIR': str(self.root / 'data'),
            'CLIARY_CACHE_DIR': str(self.root / 'cache'),
            'CLIARY_INSTALL_DIR': str(self.root / 'bin'),
            'CLIARY_REPO': 'example/private-repo',
        })
        self.installed = self.root / 'bin/cliary'

    def write_checksums(self):
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        (self.assets / 'SHA256SUMS').write_text(f'{digest}  {self.archive.name}\n')

    def install(self, *args):
        return subprocess.run(['sh', str(ROOT / 'scripts/install.sh'), *args,
                               '--no-scan', '--no-history'], env=self.env,
                              text=True, capture_output=True, timeout=30)

    def cli(self, *args):
        return subprocess.run([str(self.installed), *args], env=self.env,
                              check=True, text=True, capture_output=True, timeout=15).stdout

    def test_offline_install_and_upgrade_preserve_favorites_notes_and_history(self):
        self.env.pop('CLIARY_REPO')  # Offline mode requires neither repository nor login.
        result = self.install('--from', str(self.assets))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.cli('favorite', 'ncdu')
        self.cli('note', 'ncdu', '--set', 'upgrade fixture')
        history = self.root / 'history.fixture'
        history.write_text(': 1700000000:0;ncdu /synthetic\n')
        self.cli('import-history', '--shell', 'zsh', '--file', str(history), '--apply')
        before = {name: json.loads(self.cli(name, '--json'))
                  for name in ('favorites', 'history')}
        detail = json.loads(self.cli('show', 'ncdu', '--json'))
        result = self.install('--from', str(self.assets))
        self.assertEqual(result.returncode, 0, result.stderr)
        for name, expected in before.items():
            self.assertEqual(json.loads(self.cli(name, '--json')), expected)
        self.assertEqual(json.loads(self.cli('show', 'ncdu', '--json')), detail)
        self.assertEqual(self.installed.read_bytes(), BINARY.read_bytes())
        self.assertFalse(list(self.installed.parent.glob('.cliary.*')))

    def test_bad_checksum_keeps_existing_binary(self):
        self.assertEqual(self.install('--from', str(self.assets)).returncode, 0)
        original = self.installed.read_bytes()
        with self.archive.open('ab') as archive:
            archive.write(b'corruption')
        result = self.install('--from', str(self.assets))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('checksum mismatch', result.stderr)
        self.assertEqual(self.installed.read_bytes(), original)

    @unittest.skipUnless(os.uname().sysname == 'Linux', 'GNU fallback is Linux-only')
    def test_offline_prefers_musl_and_accepts_legacy_gnu(self):
        legacy = self.archive.with_name(self.archive.name.replace('-musl.', '-gnu.'))
        legacy.write_bytes(self.archive.read_bytes())
        # A bogus legacy checksum cannot affect the preferred, valid static package.
        with (self.assets / 'SHA256SUMS').open('a') as checksums:
            checksums.write(f'{"0" * 64}  {legacy.name}\n')
        result = self.install('--from', str(self.assets))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.archive.unlink()
        self.archive = legacy
        self.write_checksums()
        result = self.install('--from', str(self.assets))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.installed.read_bytes(), BINARY.read_bytes())

    def test_symlink_archive_is_rejected(self):
        with tarfile.open(self.archive, 'w:gz') as archive:
            entry = tarfile.TarInfo('cliary')
            entry.type = tarfile.SYMTYPE
            entry.linkname = '/does-not-exist'
            archive.addfile(entry)
        self.write_checksums()
        self.assertNotEqual(self.install('--from', str(self.assets)).returncode, 0)
        self.assertFalse(self.installed.exists())

    def fake_github(self, failure=False):
        commands = self.root / 'commands'
        commands.mkdir()
        helper = commands / 'gh'
        helper.write_text('''#!/usr/bin/env python3
import json, os, pathlib, shutil, sys
args = sys.argv[1:]
with open(os.environ['CLIARY_TEST_GH_LOG'], 'a') as out:
    out.write(json.dumps(args) + '\\n')
if os.environ.get('CLIARY_TEST_GH_FAILURE'):
    sys.exit(1)
if args[:2] == ['release', 'view']:
    print('v-test')
elif args[:3] == ['release', 'download', 'v-test']:
    directory = pathlib.Path(args[args.index('--dir') + 1])
    patterns = [args[i + 1] for i, arg in enumerate(args) if arg == '--pattern']
    for pattern in patterns:
        shutil.copyfile(pathlib.Path(os.environ['CLIARY_TEST_ASSETS']) / pattern, directory / pattern)
else:
    sys.exit(2)
''')
        helper.chmod(0o755)
        self.env.update({'PATH': f'{commands}:{os.environ["PATH"]}',
                         'CLIARY_TEST_GH_LOG': str(self.root / 'gh.jsonl'),
                         'CLIARY_TEST_ASSETS': str(self.assets)})
        if failure:
            self.env['CLIARY_TEST_GH_FAILURE'] = '1'

    def test_private_download_pins_release_and_uses_gh(self):
        self.fake_github()
        result = self.install('--github')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in (self.root / 'gh.jsonl').read_text().splitlines()]
        self.assertEqual(calls[0], ['release', 'view', '--repo', 'example/private-repo', '--json', 'tagName', '--jq', '.tagName'])
        self.assertEqual(calls[1][:3], ['release', 'download', 'v-test'])
        self.assertIn('SHA256SUMS', calls[1])
        self.assertEqual(calls[2][:3], ['release', 'download', 'v-test'])
        self.assertIn(self.archive.name, calls[2])

    @unittest.skipUnless(os.uname().sysname == 'Linux', 'GNU fallback is Linux-only')
    def test_private_legacy_gnu_release_is_still_installable(self):
        legacy = self.archive.with_name(self.archive.name.replace('-musl.', '-gnu.'))
        self.archive.rename(legacy)
        self.archive = legacy
        self.write_checksums()
        self.fake_github()
        result = self.install('--github')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in (self.root / 'gh.jsonl').read_text().splitlines()]
        self.assertIn(legacy.name, calls[2])

    def test_private_failure_does_not_replace_binary(self):
        self.assertEqual(self.install('--from', str(self.assets)).returncode, 0)
        original = self.installed.read_bytes()
        self.fake_github(failure=True)
        result = self.install('--github')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Check gh login', result.stderr)
        self.assertEqual(self.installed.read_bytes(), original)

    def test_invalid_options_fail_before_installing(self):
        for args in [('--from',), ('--tag', '-invalid'),
                     ('--from', str(self.assets), '--github')]:
            self.assertEqual(self.install(*args).returncode, 2)
        self.assertFalse(self.installed.exists())


if __name__ == '__main__':
    if not BINARY.is_file():
        raise SystemExit(f'Build cliary first: missing {BINARY}')
    unittest.main(verbosity=2)
