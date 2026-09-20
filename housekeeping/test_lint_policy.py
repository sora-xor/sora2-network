#!/usr/bin/env python3
"""Keep adopted first-party crates covered by the shared Clippy policy.

Run with Python 3.11+: python3 housekeeping/test_lint_policy.py
"""

import re
import tomllib
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ADOPTED_CRATES = {
    "runtime": "src/lib.rs",
    "node": "src/main.rs",
    "pallets/eth-bridge": "src/lib.rs",
}
PROTECTED_GROUPS = {"all", "correctness", "suspicious", "perf"}
LEVELS = {"allow": 0, "warn": 1, "deny": 2, "forbid": 3}


def read_manifest(path):
    with path.open("rb") as manifest:
        return tomllib.load(manifest)


def lint_level(setting):
    return setting.get("level") if isinstance(setting, dict) else setting


def lint_priority(setting):
    return setting.get("priority", 0) if isinstance(setting, dict) else 0


def protected_group_allows(source):
    """Find direct and cfg_attr-wrapped allows, including multiline attributes."""
    violations = []
    for attribute in re.finditer(r"#\s*!?\s*\[([^\]]*)\]", source, re.DOTALL):
        for allow in re.finditer(r"\ballow\s*\(([^)]*)\)", attribute.group(1)):
            for group in re.findall(r"\bclippy\s*::\s*(\w+)", allow.group(1)):
                if group in PROTECTED_GROUPS:
                    line = source.count("\n", 0, attribute.start()) + 1
                    violations.append((line, group))
    return violations


class LintPolicyTests(unittest.TestCase):
    def test_workspace_keeps_high_value_lints_enabled(self):
        policy = read_manifest(REPO_ROOT / "Cargo.toml")["workspace"]["lints"]
        required = {
            "rust": {"unused_must_use": "deny"},
            "clippy": {
                "all": "warn",
                "correctness": "deny",
                "suspicious": "warn",
                "perf": "warn",
            },
        }
        for tool, lints in required.items():
            for lint, minimum in lints.items():
                with self.subTest(tool=tool, lint=lint):
                    level = lint_level(policy.get(tool, {}).get(lint))
                    self.assertGreaterEqual(
                        LEVELS.get(level, -1),
                        LEVELS[minimum],
                        f"workspace.lints.{tool}.{lint} must be at least {minimum}",
                    )

    def test_specific_groups_override_the_all_group(self):
        policy = read_manifest(REPO_ROOT / "Cargo.toml")["workspace"]["lints"]["clippy"]
        for group in PROTECTED_GROUPS - {"all"}:
            with self.subTest(group=group):
                self.assertLess(
                    lint_priority(policy["all"]),
                    lint_priority(policy[group]),
                    f"clippy::{group} must take precedence over clippy::all",
                )

    def test_adopted_crates_inherit_workspace_lints(self):
        for crate in ADOPTED_CRATES:
            with self.subTest(crate=crate):
                manifest = read_manifest(REPO_ROOT / crate / "Cargo.toml")
                self.assertIs(manifest.get("lints", {}).get("workspace"), True)

    def test_adopted_crate_roots_do_not_disable_protected_groups(self):
        for crate, source in ADOPTED_CRATES.items():
            with self.subTest(crate=crate):
                path = REPO_ROOT / crate / source
                violations = protected_group_allows(path.read_text())
                self.assertFalse(
                    violations,
                    f"{crate}/{source}: protected Clippy group allows {violations}; "
                    "use a documented exception for an individual lint instead",
                )

    def test_group_guard_handles_conditional_and_multiline_attributes(self):
        self.assertEqual(
            protected_group_allows(
                "#![allow(\n clippy::all,\n clippy::perf\n)]\n"
                '#![cfg_attr(feature = "std", allow(clippy::correctness))]\n'
                "#[allow(clippy::suspicious)]\n"
                "#![allow(clippy::style, clippy::complexity)]\n"
                "#[allow(clippy::type_complexity)]\n"
            ),
            [(1, "all"), (1, "perf"), (5, "correctness"), (6, "suspicious")],
        )


if __name__ == "__main__":
    unittest.main()
