#!/usr/bin/env python3
"""Behavioral contract tests for the align-grid scaffold generator."""

from html.parser import HTMLParser
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
GENERATOR = ROOT / "skills" / "align-grid" / "scripts" / "grid_tokens.py"


class Structure(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.classes = set()

    def handle_starttag(self, _tag, attrs):
        values = dict(attrs)
        if values.get("id"):
            self.ids.add(values["id"])
        self.classes.update(values.get("class", "").split())


def generate(*args):
    return subprocess.run(
        [sys.executable, str(GENERATOR), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


class GridTokensContract(unittest.TestCase):
    def test_scaffold_uses_one_configured_grid_contract(self):
        result = generate(
            "--scaffold",
            "--cols",
            "10",
            "--baseline",
            "6",
            "--gutter",
            "18",
            "--margin",
            "60",
            "--maxw",
            "1200",
            "--accent",
            "#c026d3",
        )
        self.assertEqual(result.returncode, 0, result.stderr)

        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "grid.html"
            fixture.write_text(result.stdout, encoding="utf-8")
            html = fixture.read_text(encoding="utf-8")

        structure = Structure()
        structure.feed(html)
        self.assertIn("gridToggle", structure.ids)
        self.assertTrue({"spread", "wrap", "grid", "band", "guides", "cols", "rows"}.issubset(structure.classes))

        for token in (
            "--cols:10",
            "--bl:6px",
            "--lh:18px",
            "--gutter:18px",
            "--margin:60px",
            "--maxw:1200px",
            "--accent:#c026d3",
        ):
            self.assertIn(token, html)
        self.assertGreaterEqual(html.count("repeat(var(--cols),1fr)"), 3)
        self.assertIn("grid-template-columns:subgrid", html)
        self.assertIn("body.grid-on .guides", html)
        self.assertIn("aria-pressed", html)
        self.assertIn("e.key==='g'||e.key==='G'", html)
        self.assertIn("actualBoundingBoxLeft", html)
        self.assertIn("document.fonts.ready", html)

    def test_rejects_non_positive_dimensions(self):
        for option, value in (("--cols", "0"), ("--baseline", "0"), ("--gutter", "-1"), ("--margin", "-1"), ("--maxw", "0")):
            with self.subTest(option=option):
                result = generate(option, value)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("must be", result.stderr)


if __name__ == "__main__":
    unittest.main()
