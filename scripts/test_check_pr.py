import contextlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from check_pr import associated_ids, compare_delta, validate


REQUIREMENT = "### Requirement: Example\nThe tool SHALL work.\n\n#### Scenario: Run\n- **WHEN** run\n- **THEN** it works\n"


class PullRequestGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.old_dir = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, self.old_dir)
        self.addCleanup(self.temp.cleanup)
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Test")
        self.write("README.md", "test")
        self.write("openspec/changes/unrelated/proposal.md", "Still in progress")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", *args], stderr=subprocess.STDOUT).decode()

    def write(self, name, text):
        path = Path(name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-qm", "test: fixture")

    def archive(self, name="example", synced=True):
        prefix = f"openspec/changes/archive/2026-09-07-{name}"
        self.write(f"{prefix}/proposal.md", "Example proposal")
        self.write(f"{prefix}/tasks.md", "- [x] Complete")
        self.write(f"{prefix}/specs/example/spec.md", "## ADDED Requirements\n\n" + REQUIREMENT)
        if synced:
            self.write("openspec/specs/example/spec.md", "# Example\n## Requirements\n" + REQUIREMENT)

    def check(self, body="OpenSpec-Changes: none"):
        with contextlib.redirect_stdout(None):
            validate(self.base, "HEAD", "fix: example", body)

    def test_unrelated_active_change_does_not_block(self):
        self.write("README.md", "Updated")
        self.commit()
        self.check()

    def test_new_archive_requires_actual_synchronization(self):
        self.archive(synced=False)
        self.commit()
        with self.assertRaisesRegex(ValueError, "not synchronized"):
            self.check()
        self.write("openspec/specs/example/spec.md", REQUIREMENT)
        self.commit()
        self.check()

    def test_edited_active_change_fails(self):
        self.write("openspec/changes/unrelated/proposal.md", "Now part of this PR")
        self.commit()
        with self.assertRaisesRegex(ValueError, "active change"):
            self.check()

    def test_deletion_alone_is_not_archival(self):
        Path("openspec/changes/unrelated/proposal.md").unlink()
        self.commit()
        with self.assertRaisesRegex(ValueError, "preserved archive"):
            self.check()

    def test_declared_change_without_artifact_diff_is_checked(self):
        self.write("README.md", "Implementation-only change")
        self.commit()
        with self.assertRaisesRegex(ValueError, "active change"):
            self.check("OpenSpec-Changes: unrelated")

    def test_multiple_changes_and_already_synced_specs(self):
        self.write("openspec/specs/example/spec.md", REQUIREMENT)
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()
        self.archive("one", synced=False)
        self.archive("two", synced=False)
        self.commit()
        self.check("OpenSpec-Changes: one, two")

    def test_rename_diff_checks_both_paths(self):
        self.git("mv", "openspec/changes/unrelated", "openspec/changes/renamed")
        self.commit()
        with self.assertRaisesRegex(ValueError, "unrelated: expected"):
            self.check()
        self.assertEqual(associated_ids([
            "openspec/changes/unrelated/proposal.md",
            "openspec/changes/renamed/proposal.md",
        ]), {"unrelated", "renamed"})

    def test_removal_and_rename_contracts(self):
        self.assertEqual(compare_delta("## REMOVED Requirements\n" + REQUIREMENT, ""), [])
        self.assertTrue(compare_delta("## REMOVED Requirements\n" + REQUIREMENT, REQUIREMENT))
        delta = "## RENAMED Requirements\n- FROM: `### Requirement: Old`\n- TO: `### Requirement: Example`\n"
        self.assertEqual(compare_delta(delta, REQUIREMENT), [])
        self.assertTrue(compare_delta(delta, REQUIREMENT.replace("Example", "Old")))

    def test_missing_association_and_bad_title_fail(self):
        with self.assertRaisesRegex(ValueError, "OpenSpec-Changes"):
            self.check("")
        with self.assertRaisesRegex(ValueError, "Conventional Commit"):
            validate(self.base, "HEAD", "Update things", "OpenSpec-Changes: none")


if __name__ == "__main__":
    unittest.main()
