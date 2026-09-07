import importlib.util
import io
from pathlib import Path
import tarfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "skills/publish-kibob/scripts/update_homebrew_formula.py"
spec = importlib.util.spec_from_file_location("updater", MODULE)
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


def archive(cli_version="0.4.0", extra=None, license_name="LICENCE.md"):
    files = {
        license_name: "Apache License\nVersion 2.0",
        "Cargo.toml": '[workspace]\nmembers = ["crates/kibana-sync", "crates/kibana-object-manager"]',
        "Cargo.lock": '[[package]]\nname="kibana-sync"\nversion="0.3.2"\n[[package]]\nname="kibana-object-manager"\nversion="0.4.0"',
    }
    for name, version in [("kibana-sync", "0.3.2"), ("kibana-object-manager", cli_version)]:
        files[f"crates/{name}/Cargo.toml"] = f'[package]\nname="{name}"\nversion="{version}"\nlicense="Apache-2.0"'
    if extra:
        files.update(extra)
    data = io.BytesIO()
    with tarfile.open(fileobj=data, mode="w:gz") as tar:
        for name, text in files.items():
            info = tarfile.TarInfo("source/" + name)
            body = text.encode()
            info.size = len(body)
            tar.addfile(info, io.BytesIO(body))
    return data.getvalue()


class ReleaseUpdaterTests(unittest.TestCase):
    def test_independent_versions_are_supported(self):
        updater.validate_archive(archive(), "0.4.0")

    def test_historical_license_entry_remains_supported(self):
        updater.validate_archive(archive(license_name="LICENSE"), "0.4.0")

    def test_wrong_release_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "selected release"):
            updater.validate_archive(archive("0.5.0"), "0.4.0")

    def test_unsafe_archive_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unsafe paths"):
            updater.validate_archive(archive(extra={"../outside": "bad"}), "0.4.0")

    def test_lockfile_disagreement_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Lockfile"):
            updater.validate_archive(archive(extra={"Cargo.lock": '[[package]]\nname="wrong"\nversion="0.4.0"'}), "0.4.0")

    def test_bad_version_and_non_archive_are_rejected(self):
        with self.assertRaises(ValueError):
            updater.tarball_url("../../main")
        with self.assertRaises(tarfile.TarError):
            updater.validate_archive(b"server error", "0.4.0")

    def test_formula_changes_only_source_fields(self):
        source = 'class Kibob < Formula\n  url "old"\n  sha256 "abcd"\n  license "Apache-2.0"\nend\n'
        result = updater.patch_formula(source, "new", "1234")
        self.assertEqual(result, source.replace('"old"', '"new"').replace('"abcd"', '"1234"'))


if __name__ == "__main__":
    unittest.main()
