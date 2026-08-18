"""Tests for advisory-quality platform aggregation."""

from __future__ import annotations

import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

import advisory_quality_report as report

ROOT = Path(__file__).parents[2]
SCRIPT = Path(__file__).with_name("advisory_quality_report.py")

TOO_MANY_LINES_DIAGNOSTIC = """\
warning: this function has too many lines (68/60)
  --> src/text_processing.rs:101:1
   |
101 | fn classify_and_decode(
   | ^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: https://rust-lang.github.io/rust-clippy/index.html#too_many_lines
"""
TOO_MANY_ARGUMENTS_DIAGNOSTIC = """\
warning: this function has too many arguments (9/8)
  --> tests\\crate\\cli.rs:42:1
   |
42 | fn parse_options(
   | ^^^^^^^^^^^^^^^^
   |
   = note: requested on the command line with `-W clippy::too-many-arguments`
"""


def finding(
    path: str = "src/text_processing.rs",
    line: int = 101,
    function: str = "classify_and_decode",
    *,
    lint: str = "clippy::too_many_lines",
    observed: int = 68,
    threshold: int = 60,
    unit: str = "lines",
) -> report.Finding:
    """Build a controlled maintainability finding."""
    return report.Finding(
        path=path,
        line=line,
        function=function,
        lint=lint,
        observed=observed,
        threshold=threshold,
        unit=unit,
        message=None,
    )


class ParsingTests(unittest.TestCase):
    """Exercise raw Clippy diagnostics through the public parsing pipeline."""

    def test_extracts_both_configured_lints(self) -> None:
        output = TOO_MANY_LINES_DIAGNOSTIC + TOO_MANY_ARGUMENTS_DIAGNOSTIC

        self.assertEqual(
            report.extract_findings(output),
            [
                finding(),
                finding(
                    "tests/crate/cli.rs",
                    42,
                    "parse_options",
                    lint="clippy::too_many_arguments",
                    observed=9,
                    threshold=8,
                    unit="arguments",
                ),
            ],
        )

    def test_extracts_generic_function_name(self) -> None:
        diagnostic = (
            "warning: this function has too many lines (68/60)\n"
            "  --> src/example.rs:5:1\n"
            "5 | fn generic<T>(value: T) {}\n"
            "   = note: requested with `-W clippy::too-many-lines`\n"
        )

        findings = report.extract_findings(diagnostic)

        self.assertEqual(len(findings), 1)
        self.assertEqual(findings[0].function, "generic")

    def test_extract_findings_removes_ansi_sequences(self) -> None:
        coloured = TOO_MANY_LINES_DIAGNOSTIC.replace(
            "warning:",
            "\x1b[1m\x1b[33mwarning\x1b[0m\x1b[1m:\x1b[0m",
            1,
        )

        self.assertEqual(report.extract_findings(coloured), [finding()])

    def test_unrelated_warning_is_ignored(self) -> None:
        diagnostic = """\
warning: unused variable: `value`
  --> src/main.rs:10:9
   |
10 |     let value = 1;
   |         ^^^^^ help: prefix it with an underscore: `_value`
"""

        self.assertEqual(report.extract_findings(diagnostic), [])

    def test_duplicate_diagnostics_are_deduplicated(self) -> None:
        output = TOO_MANY_LINES_DIAGNOSTIC + TOO_MANY_LINES_DIAGNOSTIC

        self.assertEqual(report.extract_findings(output), [finding()])


class PlatformReportTests(unittest.TestCase):
    """Exercise per-platform report generation."""

    def test_no_findings_report(self) -> None:
        rendered = report.render_report("Linux", [])

        self.assertIn("### Linux", rendered)
        self.assertIn("✅ No advisory maintainability findings", rendered)

    def test_platform_mode_replaces_invalid_utf8(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            quality_log = root / "quality.log"
            markdown = root / "quality-linux.md"
            structured = root / "quality-linux.json"
            quality_log.write_bytes(
                b"incidental invalid byte: \xff\n"
                + TOO_MANY_LINES_DIAGNOSTIC.encode("utf-8")
            )
            argv = [
                str(SCRIPT),
                "platform",
                str(quality_log),
                str(markdown),
                str(structured),
                "Linux",
            ]

            with (
                mock.patch.object(sys, "argv", argv),
                mock.patch("builtins.print") as print_mock,
            ):
                report.main()

            print_mock.assert_called_once_with(1)
            result = report.load_platform_result(structured)
            self.assertEqual(result.platform, "Linux")
            self.assertEqual(result.findings, (finding(),))


class CombinedCommentTests(unittest.TestCase):
    """Exercise deterministic grouping across native platform results."""

    def render(
        self,
        linux: list[report.Finding],
        macos_arm64: list[report.Finding],
        macos_x64: list[report.Finding],
        windows: list[report.Finding],
    ) -> str:
        """Round-trip controlled platform findings through JSON files."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            linux_path = root / "quality-linux.json"
            macos_arm64_path = root / "quality-macos-arm64.json"
            macos_x64_path = root / "quality-macos-x64.json"
            windows_path = root / "quality-windows.json"
            report.write_platform_result(linux_path, "Linux", linux)
            report.write_platform_result(
                macos_arm64_path,
                "macOS Apple Silicon",
                macos_arm64,
            )
            report.write_platform_result(macos_x64_path, "macOS Intel", macos_x64)
            report.write_platform_result(windows_path, "Windows", windows)
            results = [
                report.load_platform_result(linux_path),
                report.load_platform_result(macos_arm64_path),
                report.load_platform_result(macos_x64_path),
                report.load_platform_result(windows_path),
            ]
            return report.render_combined_comment(results)

    def test_identical_findings_are_grouped_for_all_platforms(self) -> None:
        shared = finding()
        comment = self.render([shared], [shared], [shared], [shared])

        self.assertIn("## Maintainability checks ⚠️", comment)
        self.assertIn("These do not block merging.", comment)
        self.assertIn("### All platforms", comment)
        self.assertEqual(comment.count("`classify_and_decode`"), 1)

    def test_platform_only_findings_have_separate_headings(self) -> None:
        linux = finding("src/linux.rs", 20, "linux_check")
        macos_arm64 = finding("src/macos.rs", 30, "macos_check")
        comment = self.render([linux], [macos_arm64], [], [])

        self.assertIn("### Linux only", comment)
        self.assertIn("### macOS Apple Silicon only", comment)

    def test_findings_shared_by_both_macos_architectures_are_grouped(self) -> None:
        shared = finding("src/macos.rs", 30, "macos_check")
        comment = self.render([], [shared], [shared], [])

        self.assertIn("### macOS Apple Silicon + macOS Intel", comment)
        self.assertEqual(comment.count("`macos_check`"), 1)

    def test_no_findings_uses_droll_marker_and_no_platform_sections(self) -> None:
        comment = self.render([], [], [], [])

        self.assertEqual(
            comment,
            "<!-- droll-advisory-quality -->\n"
            "\n"
            "## Maintainability checks ✅\n"
            "\n"
            "No advisory maintainability findings.\n"
            "\n"
            "All checked functions are within the configured thresholds:\n"
            "\n"
            "- function lines: 60\n"
            "- arguments: 8\n",
        )
        self.assertNotIn("\n### ", comment)

    def test_same_function_at_different_locations_is_not_merged(self) -> None:
        linux = finding("src/one.rs", 10, "shared_name")
        windows = finding("src/two.rs", 20, "shared_name")
        comment = self.render([linux], [], [], [windows])

        self.assertEqual(comment.count("`shared_name`"), 2)
        self.assertIn("`src/one.rs:10`", comment)
        self.assertIn("`src/two.rs:20`", comment)

    def test_displayed_thresholds_match_clippy_config(self) -> None:
        config = tomllib.loads((ROOT / "clippy.toml").read_text(encoding="utf-8"))
        comment = self.render([], [], [], [])

        self.assertIn(
            f"- function lines: {config['too-many-lines-threshold']}",
            comment,
        )
        self.assertIn(
            f"- arguments: {config['too-many-arguments-threshold']}",
            comment,
        )


if __name__ == "__main__":
    unittest.main()
