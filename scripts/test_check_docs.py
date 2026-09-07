import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_docs


class DocumentationChecks(unittest.TestCase):
    def run_check(self, extra, readme=""):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            bundle = root / "docs"
            bundle.mkdir()
            (root / "README.md").write_text(readme)
            (root / "AGENTS.md").write_text("")
            (bundle / "index.md").write_text("[Guide](guide.md)\n")
            (bundle / "guide.md").write_text("Guide\n")
            for name in extra:
                path = bundle / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture")
            with patch.object(check_docs, "ROOT", root), patch.object(check_docs, "BUNDLE", bundle):
                with contextlib.redirect_stdout(io.StringIO()):
                    check_docs.check()

    def test_lowercase_nested_assets_pass(self):
        self.run_check(["images/example_image.svg"], "[Guide](docs/guide.md)")

    def test_uppercase_nested_assets_fail(self):
        with self.assertRaisesRegex(SystemExit, "must be lowercase"):
            self.run_check(["images/Example.svg"])

    def test_uppercase_directory_fails(self):
        with self.assertRaisesRegex(SystemExit, "must be lowercase"):
            self.run_check(["Images/example.svg"])

    def test_wrong_link_case_fails_on_any_filesystem(self):
        with self.assertRaises(SystemExit):
            self.run_check([], "[Guide](docs/GUIDE.md)")


if __name__ == "__main__":
    unittest.main()
