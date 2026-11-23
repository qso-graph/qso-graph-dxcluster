#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 the QG DX Cluster contributors. Part of QG DX Cluster; see LICENSE.
"""The release's decisions, each with the cases it must refuse. Run: uv run --no-project --python 3.11 python scripts/test_release_check.py"""

import sys
import unittest
import io
import re
import json
import tarfile
import tempfile
import subprocess
import os
import zipfile
from unittest.mock import patch
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_check as rc  # noqa: E402
import release_artifacts as artifacts  # noqa: E402

REPO = "qso-graph/qso-graph-dxcluster"
TREES = {"good": "T", "other": "U", "fork": "T"}


def run(**over):
    base = {
        "event": "pull_request",
        "conclusion": "success",
        "path": ".github/workflows/ci.yml",
        "head_repository": {"full_name": REPO},
        "head_sha": "good",
    }
    base.update(over)
    return base


class Source(unittest.TestCase):
    SHA = "a" * 40

    def args(self, **changes):
        values = dict(ref="refs/heads/main", event="workflow_dispatch", build_sha=self.SHA,
                      workflow_sha=self.SHA,
                      workflow_ref="qso-graph/qso-graph-dxcluster/.github/workflows/release.yml@refs/heads/main",
                      actor="KI7MT", triggering_actor="KI7MT")
        values.update(changes)
        return list(values.values())

    def test_manual_main_release_at_exact_commit(self):
        self.assertTrue(rc.source_allowed(*self.args()))
        rc.main(["source", *self.args()])

    def test_other_branches_tags_and_pr_refs_are_refused(self):
        for ref in ["refs/heads/feature/0.1.0", "refs/heads/0.1.x", "refs/tags/v0.1.0",
                    "refs/pull/18/merge", "refs/pull/18/head", "main", "refs/heads/main\n", ""]:
            with self.subTest(ref=ref), self.assertRaises(SystemExit):
                rc.main(["source", *self.args(ref=ref)])

    def test_automatic_events_are_refused(self):
        for event in ["push", "pull_request", "pull_request_target", "schedule", "repository_dispatch", ""]:
            with self.subTest(event=event):
                self.assertFalse(rc.source_allowed(*self.args(event=event)))

    def test_other_main_workflows_and_other_operators_are_refused(self):
        for workflow in ["binaries.yml", "ci.yml"]:
            with self.subTest(workflow=workflow):
                ref=f"qso-graph/qso-graph-dxcluster/.github/workflows/{workflow}@refs/heads/main"
                self.assertFalse(rc.source_allowed(*self.args(workflow_ref=ref)))
        self.assertFalse(rc.source_allowed(*self.args(workflow_ref="")))
        # Another repository's release cannot authorize this publication.
        self.assertFalse(rc.source_allowed(*self.args(
            workflow_ref="someone/qso-graph-dxcluster/.github/workflows/release.yml@refs/heads/main")))
        for actor in ["release-bot[bot]", "someone", "KI7MT-other", ""]:
            with self.subTest(actor=actor):
                self.assertFalse(rc.source_allowed(*self.args(actor=actor)))
                self.assertFalse(rc.source_allowed(*self.args(triggering_actor=actor)))
        self.assertTrue(rc.source_allowed(*self.args(actor="ki7mt", triggering_actor="ki7mt")))

    def test_main_cannot_sign_a_different_or_ambiguous_commit(self):
        for build_sha, workflow_sha in [("b" * 40, self.SHA), ("main", self.SHA),
                                        ("", self.SHA), ("", ""), ("a" * 7, "a" * 7),
                                        (self.SHA + "\n", self.SHA + "\n")]:
            with self.subTest(build_sha=build_sha):
                self.assertFalse(rc.source_allowed(*self.args(build_sha=build_sha, workflow_sha=workflow_sha)))


class Notes(unittest.TestCase):
    def test_exact_section_excludes_neighbors_and_accepts_date(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            change = root / "CHANGELOG.md"
            change.write_text("## [Unreleased]\nfuture\n## [0.1.0] - 2026-11-18\n- Initial release.\n## [0.0.9]\nold\n")
            self.assertEqual(rc.changelog_section(root, "0.1.0"), "- Initial release.\n")
            self.assertIsNone(rc.changelog_section(root, "0.1.1"))
            change.write_text("## [0.1.0]\nfirst\n## [0.1.0]\nduplicate\n")
            self.assertIsNone(rc.changelog_section(root, "0.1.0"))

    def test_missing_duplicate_or_empty_notes_refuse_cli(self):
        for notes in [None, "", " \n"]:
            with self.subTest(notes=notes), patch.object(rc, "changelog_section", return_value=notes):
                with self.assertRaises(SystemExit):
                    rc.main(["notes", "0.1.0"])
        with patch.object(rc, "changelog_section", return_value="- Initial release.\n"), patch("sys.stdout", new_callable=io.StringIO) as output:
            rc.main(["notes", "0.1.0"])
            self.assertEqual(output.getvalue(), "- Initial release.\n")

    def test_repository_has_initial_release_notes(self):
        root = Path(__file__).resolve().parent.parent
        self.assertIn("qg-dxc", rc.changelog_section(root, "0.1.0"))


class Version(unittest.TestCase):
    def test_releases_and_betas(self):
        for v in ["0.1.0", "0.1.0-beta.1", "1.2.3-beta.12", "0.1.0-beta.1000", "10.0.0"]:
            self.assertTrue(rc.is_version(v), v)

    def test_refused(self):
        for v in ["0.1", "v0.1.0", "0.1.0-beta.0", "0.1.0-beta", "0.1.0-rc.1", "0.1.0-beta.1 ", "0.1.0\n",
                  "0.1.0-beta.1;x", "01.0", "01.0.0", "0.01.0", "0.1.00"]:
            self.assertFalse(rc.is_version(v), repr(v))


class ProvingRun(unittest.TestCase):
    def prove(self, runs, tree="T"):
        return rc.proving_run(runs, REPO, tree, TREES.get)

    def test_a_green_run_of_exactly_this_tree(self):
        self.assertEqual(self.prove([run()]), "good")

    def test_a_run_of_other_files(self):
        self.assertIsNone(self.prove([run(head_sha="other")]))

    def test_a_forks_run(self):
        self.assertIsNone(self.prove([run(head_sha="fork", head_repository={"full_name": "someone/qso-graph-dxcluster"})]))
        self.assertIsNone(self.prove([run(head_repository=None)]))

    def test_a_run_that_did_not_pass(self):
        for conclusion in ["failure", "cancelled", "skipped", None]:
            self.assertIsNone(self.prove([run(conclusion=conclusion)]), conclusion)

    def test_another_workflow_or_event(self):
        self.assertIsNone(self.prove([run(path=".github/workflows/release.yml")]))
        self.assertIsNone(self.prove([run(event="push")]))
        self.assertIsNone(self.prove([run(event="workflow_dispatch")]))

    def test_the_first_proving_run_among_others(self):
        self.assertEqual(self.prove([run(head_sha="other"), run(conclusion="failure"), run()]), "good")


class Published(unittest.TestCase):
    def test_absent(self):
        self.assertEqual(rc.published("c", "0.1.0", "aa", lambda c, v: None), "absent")

    def test_the_same_package(self):
        self.assertEqual(rc.published("c", "0.1.0", "aa", lambda c, v: "aa"), "same")

    def test_a_different_package_under_the_version(self):
        with self.assertRaises(SystemExit):
            rc.published("c", "0.1.0", "aa", lambda c, v: "bb")



class FirstToken(unittest.TestCase):
    def test_while_a_crate_is_new(self):
        self.assertTrue(rc.first_token_allowed({
            "qso-graph-dxcluster": True,
            "qso-graph-spaceweather": True,
            "qso-graph-http": False,
            "qso-graph-dxcluster-cli": True,
        }))
        self.assertTrue(rc.first_token_allowed({"a": False}))

    def test_refused_once_every_crate_exists(self):
        self.assertFalse(rc.first_token_allowed({
            "qso-graph-dxcluster": True,
            "qso-graph-spaceweather": True,
            "qso-graph-http": True,
            "qso-graph-dxcluster-cli": True,
        }))


class Tag(unittest.TestCase):
    def test_absent(self):
        self.assertEqual(rc.tag_state("", "0.1.0", "abc"), "absent")
        self.assertEqual(rc.tag_state("abc\trefs/tags/v0.1.0-beta.1\n", "0.1.0", "abc"), "absent")

    def test_on_this_commit(self):
        self.assertEqual(rc.tag_state("abc\trefs/tags/v0.1.0\n", "0.1.0", "abc"), "same")

    def test_annotated_on_this_commit(self):
        refs = "tagobj\trefs/tags/v0.1.0\nabc\trefs/tags/v0.1.0^{}\n"
        self.assertEqual(rc.tag_state(refs, "0.1.0", "abc"), "same")

    def test_on_another_commit(self):
        with self.assertRaises(SystemExit):
            rc.tag_state("def\trefs/tags/v0.1.0\n", "0.1.0", "abc")
        with self.assertRaises(SystemExit):
            rc.tag_state("abc\trefs/tags/v0.1.0\ndef\trefs/tags/v0.1.0^{}\n", "0.1.0", "abc")


class RegistryVisibility(unittest.TestCase):
    def test_absent_then_visible(self):
        replies = iter([None, None, "aa"])
        pauses = []
        rc.wait_published("c", "0.1.0", "aa", lambda c, v: next(replies), pauses.append)
        self.assertEqual(pauses, [10, 10])

    def test_timeout_is_a_failure(self):
        with self.assertRaises(SystemExit):
            rc.wait_published("c", "0.1.0", "aa", lambda c, v: None, lambda _: None, attempts=2)

    def test_wrong_checksum_is_never_retried(self):
        def no_sleep(_):
            self.fail("a mismatched version must fail immediately")
        with self.assertRaises(SystemExit):
            rc.wait_published("c", "0.1.0", "aa", lambda c, v: "bb", no_sleep)

    def test_https_and_response_checksum(self):
        def response(request, timeout):
            self.assertEqual(request.full_url, "https://crates.io/api/v1/crates/c/0.1.0")
            self.assertEqual(timeout, 30)
            return io.BytesIO(json.dumps({"version": {"checksum": "aa"}}).encode())
        with patch.object(rc.urllib.request, "urlopen", response):
            self.assertEqual(rc._checksum_of("c", "0.1.0"), "aa")

    def test_only_404_means_absent(self):
        for code in [404, 403, 429, 500]:
            error = urllib.error.HTTPError("https://crates.io", code, "synthetic", {}, None)
            with self.subTest(code=code), patch.object(rc.urllib.request, "urlopen", side_effect=error):
                if code == 404:
                    self.assertFalse(rc._exists("c"))
                    self.assertIsNone(rc._checksum_of("c", "0.1.0"))
                else:
                    with self.assertRaises(SystemExit):
                        rc._exists("c")
                    with self.assertRaises(SystemExit):
                        rc._checksum_of("c", "0.1.0")


class Bundles(unittest.TestCase):
    def staged(self, root, platform):
        source = root / "files"
        for name in artifacts.expected(platform):
            file = source / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(("signed bytes: " + name).encode())
        (source / "version.txt").write_text(artifacts.version() + "\n")
        return source

    def test_each_platform_archives_signed_cli_bytes(self):
        for platform in sorted(artifacts.PLATFORMS):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = self.staged(root, platform)
                artifacts.archive(platform, source, root / "dist")
                files = sorted((root / "dist").iterdir())
                self.assertEqual(len(files), 1)
                for file in files:
                    folder = "cli"
                    prefix = file.name.removesuffix(".zip").removesuffix(".tar.gz") + "/"
                    expected = {name.removeprefix(folder + "/"): (source / name).read_bytes()
                                for name in artifacts.expected(platform) if name.startswith(folder + "/")}
                    if file.suffix == ".zip":
                        with zipfile.ZipFile(file) as bundle:
                            actual = {name.removeprefix(prefix): bundle.read(name) for name in bundle.namelist()}
                    else:
                        with tarfile.open(file) as bundle:
                            actual = {m.name.removeprefix(prefix): bundle.extractfile(m).read() for m in bundle.getmembers()}
                            self.assertEqual(next(m for m in bundle.getmembers() if m.name.endswith("/qg-dxc")).mode & 0o111, 0o111)
                    self.assertEqual(actual, expected)

    def test_missing_program_extra_file_and_version_drift_fail(self):
        for planted in ["missing", "extra", "version"]:
            with self.subTest(planted=planted), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = self.staged(root, "windows-x86_64")
                if planted == "missing":
                    (source / "cli/qg-dxc.exe").unlink()
                elif planted == "extra":
                    (source / "unexpected.exe").write_bytes(b"unsigned")
                else:
                    (source / "version.txt").write_text("9.9.9\n")
                with self.assertRaises(SystemExit):
                    artifacts.archive("windows-x86_64", source, root / "dist")

    def test_exact_cli_only_contents_and_filenames(self):
        for platform in sorted(artifacts.PLATFORMS):
            exe = "qg-dxc.exe" if platform == "windows-x86_64" else "qg-dxc"
            self.assertEqual(artifacts.expected(platform), {"version.txt", "cli/README.md", "cli/LICENSE", "cli/NOTICE.md", f"cli/{exe}"})
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                artifacts.archive(platform, self.staged(root, platform), root / "dist")
                suffix = ".zip" if platform == "windows-x86_64" else ".tar.gz"
                self.assertEqual([p.name for p in (root / "dist").iterdir()],
                                 [f"qg-dxc-{artifacts.version()}-{platform}{suffix}"])

    @unittest.skipIf(sys.platform == "win32", "symlinks require Windows developer mode")
    def test_symlink_to_expected_file_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = self.staged(root, "linux-x86_64")
            license = source / "cli/LICENSE"
            content = license.read_bytes()
            license.unlink()
            (root / "outside").write_bytes(content)
            license.symlink_to(root / "outside")
            with self.assertRaises(SystemExit):
                artifacts.archive("linux-x86_64", source, root / "dist")

    def test_unknown_platform_is_refused(self):
        with self.assertRaises(SystemExit):
            artifacts.names("../../bad")


class WorkflowPolicy(unittest.TestCase):
    def test_signing_secrets_survive_each_workflow_hop(self):
        root = Path(__file__).resolve().parent.parent
        release = (root / ".github/workflows/release.yml").read_text()
        binaries = (root / ".github/workflows/binaries.yml").read_text()
        apple = ["CSC_LINK", "CSC_KEY_PASSWORD", "APPLE_API_KEY_P8", "APPLE_API_KEY_ID", "APPLE_API_ISSUER", "APPLE_TEAM_ID"]
        azure = ["AZURE_CLIENT_ID", "AZURE_TENANT_ID", "AZURE_SUBSCRIPTION_ID"]
        macos = binaries.split("  sign-macos:\n", 1)[1].split("  sign-windows:\n", 1)[0]
        windows = binaries.split("  sign-windows:\n", 1)[1].split("  assemble:\n", 1)[0]
        for name in apple + azure:
            forwarding = f"      {name}: ${{{{ secrets.{name} }}}}"
            self.assertIn(forwarding, release)
            self.assertIn(forwarding, macos if name in apple else windows)
            self.assertIn(f"      {name}:\n        required: false", binaries)
        for platform in ["macos", "windows"]:
            self.assertIn(f"uses: qso-graph/.github/.github/workflows/sign-{platform}.yml@ce98621da707d49c828805d92f3117b6bf282cd5", binaries)

    def test_required_check_aggregates_matrix_and_binaries_without_skips(self):
        root = Path(__file__).resolve().parent.parent
        ci = (root / ".github/workflows/ci.yml").read_text()
        self.assertIn("os: [ubuntu-latest, macos-latest, windows-latest]", ci)
        self.assertIn("ref: ${{ github.event.pull_request.head.sha || github.sha }}", ci)
        aggregate = ci.split("  ci-all-green:\n", 1)[1]
        self.assertIn("    name: ci-all-green\n", aggregate)
        self.assertIn("    needs: [verify, binaries]\n", aggregate)
        self.assertIn("    if: always()\n", aggregate)
        script = aggregate.split("        run: |\n", 1)[1]
        for results in ["success success", "failure success", "success cancelled", "skipped success"]:
            with self.subTest(results=results):
                result = subprocess.run(["bash", "-c", script], env=dict(os.environ, RESULTS=results), capture_output=True)
                self.assertEqual(result.returncode == 0, results == "success success")

    def test_pins_and_run_expressions(self):
        root = Path(__file__).resolve().parent.parent
        for file in (root / ".github/workflows").glob("*.yml"):
            in_run = None
            for line in file.read_text().splitlines():
                indent = len(line) - len(line.lstrip())
                body = line.strip().removeprefix("- ")
                if in_run is not None and line.strip() and indent <= in_run:
                    in_run = None
                if body.startswith("run:"):
                    in_run = indent
                if in_run is not None:
                    self.assertNotIn("${{", line, f"expression in run in {file}")
                if body.startswith("uses:"):
                    uses = body.removeprefix("uses:").strip()
                    if not uses.startswith("./"):
                        self.assertRegex(uses, r"^[^ ]+@[0-9a-f]{40} # .+", str(file))
        for file in ["release.yml", "binaries.yml"]:
            self.assertNotIn("secrets: inherit", (root / ".github/workflows" / file).read_text())



if __name__ == "__main__":
    unittest.main(verbosity=2)
