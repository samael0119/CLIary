#!/usr/bin/env python3
"""Evaluate frozen discovery queries against a CLI binary in a private temp workspace."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/cliary')
    parser.add_argument('--report', type=Path, help='Write detailed JSON results to a chosen file')
    args = parser.parse_args()
    cases = json.loads((ROOT / 'tests/fixtures/search-queries.json').read_text())
    report = []
    with tempfile.TemporaryDirectory(prefix='cliary-search-eval-') as directory:
        env = os.environ.copy()
        for key, name in [('CLIARY_CONFIG_DIR', 'config'), ('CLIARY_DATA_DIR', 'data'), ('CLIARY_CACHE_DIR', 'cache')]:
            env[key] = str(Path(directory) / name)
        for case in cases:
            started = time.monotonic()
            process = subprocess.run([str(args.binary.resolve()), '--json', 'search', case['query']],
                                     env=env, check=True, text=True, capture_output=True, timeout=30)
            elapsed = (time.monotonic() - started) * 1000
            ids = [item['tool']['id'] for item in json.loads(process.stdout)]
            passed = (bool(set(ids[:case['rank']]) & set(case['expected'])) if case['expected'] else not ids)
            report.append({**case, 'top': ids[:case['rank']], 'passed': passed, 'cli_ms': round(elapsed, 2)})
    success = True
    for group in ['exact', 'task', 'empty']:
        rows = [r for r in report if r['group'] == group]
        hits = sum(r['passed'] for r in rows)
        print(f'{group}: {hits}/{len(rows)}')
        success &= (hits * 100 >= len(rows) * 90 if group == 'task' else hits == len(rows))
    for row in report:
        if not row['passed']:
            print(f"MISS {row['query']!r}: {row['top']}")
    times = sorted(r['cli_ms'] for r in report)
    print(f'CLI wall time median: {times[len(times)//2]:.2f} ms (includes process/startup/database work)')
    if args.report:
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    return 0 if success else 1

if __name__ == '__main__':
    raise SystemExit(main())
