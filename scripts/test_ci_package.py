#!/usr/bin/env python3
"""Platform validation gates for the cross-compiled release binaries."""
import struct
import unittest

from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path

spec = spec_from_file_location('ci_package', Path(__file__).with_name('ci-package.py'))
package = module_from_spec(spec)
spec.loader.exec_module(package)


def macho(cpu, minimum=(13 << 16), library='/usr/lib/libSystem.B.dylib'):
    name = library.encode() + b'\0'
    dylib = struct.pack('<6I', 0xC, 24 + len(name), 24, 0, 0, 0) + name
    version = struct.pack('<6I', 0x32, 24, 1, minimum, 11 << 16 | 3 << 8, 0)
    return struct.pack('<8I', 0xFEEDFACF, cpu, 0, 2, 2, len(version+dylib), 0, 0) + version + dylib


class PlatformGateTests(unittest.TestCase):
    def test_both_mac_architectures(self):
        for target, cpu in [('x86_64-apple-darwin', 0x01000007), ('aarch64-apple-darwin', 0x0100000C)]:
            info = package.inspect_binary(macho(cpu), target)
            self.assertEqual(info['minimum_macos'], '13.0.0')
            self.assertEqual(info['system_libraries'], ['/usr/lib/libSystem.B.dylib'])

    def test_wrong_mac_architecture(self):
        with self.assertRaises(ValueError):
            package.inspect_binary(macho(0x01000007), 'aarch64-apple-darwin')

    def test_newer_os_requirement_rejected(self):
        with self.assertRaises(ValueError):
            package.inspect_binary(macho(0x01000007, 14 << 16), 'x86_64-apple-darwin')

    def test_non_system_library_rejected(self):
        with self.assertRaises(ValueError):
            package.inspect_binary(macho(0x01000007, library='/opt/local/lib/test.dylib'), 'x86_64-apple-darwin')

    def test_truncated_macho_rejected(self):
        with self.assertRaises(ValueError):
            package.inspect_binary(macho(0x01000007)[:40], 'x86_64-apple-darwin')

    def test_wrong_linux_architecture(self):
        data = bytearray(64)
        data[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', data, 18, 183)
        with self.assertRaises(ValueError):
            package.inspect_binary(data, 'x86_64-unknown-linux-musl')


if __name__ == '__main__':
    unittest.main()
