#!/usr/bin/env python3
"""Validate individual YAML documents before the Rust cross-reference checks."""
import json
from pathlib import Path
import sys

import jsonschema
import yaml

root = Path(__file__).resolve().parents[1]
schema = json.loads((root / "catalog/schema/tool.schema.json").read_text())
validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker())
failures = []
files = sorted((root / "catalog/tools").rglob("*.yaml"))
for path in files:
    try:
        value = yaml.safe_load(path.read_text())
        failures.extend(f"{path.relative_to(root)}: {error.message}" for error in validator.iter_errors(value))
    except Exception as error:
        failures.append(f"{path.relative_to(root)}: {error}")
if len(files) < 50:
    failures.append(f"expected at least 50 tools; found {len(files)}")
if failures:
    print("\n".join(failures), file=sys.stderr)
    sys.exit(1)
print(f"validated schema for {len(files)} tools")
