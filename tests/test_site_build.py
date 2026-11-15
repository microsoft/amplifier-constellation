"""Offline checks for nested static product-page reference boundaries."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("constellation_site_build", ROOT / "site/build.py")
build = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build)


class NestedSiteReferences(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.container = Path(self.temp.name)
        self.root = self.container / "site"
        self.page = self.root / "projects/example"
        self.page.mkdir(parents=True)
        (self.root / "style.css").write_text("body{}")
        (self.root / "assets").mkdir()
        (self.root / "assets/overview.svg").write_text("<svg/>")
        (self.container / "outside.txt").write_text("must remain outside")

    def test_nested_page_can_reference_shared_parent_assets(self):
        build.validate(self.page, '<link href="../../style.css"><img src="../../assets/overview.svg">', root=self.root)

    def test_existing_file_outside_output_root_is_refused(self):
        with self.assertRaisesRegex(ValueError, "escaping local reference"):
            build.validate(self.page, '<a href="../../../outside.txt">outside</a>', root=self.root)

    def test_encoded_escape_is_refused(self):
        with self.assertRaisesRegex(ValueError, "escaping local reference"):
            build.validate(self.page, '<a href="%2e%2e/%2e%2e/%2e%2e/outside.txt">outside</a>', root=self.root)

    def test_symlink_outside_output_root_is_refused(self):
        try:
            (self.root / "assets/outside.txt").symlink_to(self.container / "outside.txt")
        except OSError as exc:
            self.skipTest(f"Symbolic links unavailable: {exc}")
        with self.assertRaisesRegex(ValueError, "escaping local reference"):
            build.validate(self.page, '<a href="../../assets/outside.txt">outside</a>', root=self.root)

    def test_missing_shared_asset_is_refused(self):
        with self.assertRaisesRegex(ValueError, "Missing or escaping"):
            build.validate(self.page, '<img src="../../assets/missing.svg">', root=self.root)

    def test_page_folder_outside_root_is_refused(self):
        with self.assertRaisesRegex(ValueError, "Page folder escapes"):
            build.validate(self.container, '<p>outside</p>', root=self.root)

    def test_offline_project_links_use_public_site(self):
        page = '<a href="projects/">All</a><a href="./projects/example/index.html#start">Start</a><a href="#local">Local</a>'
        expected = f'<a href="{build.PUBLIC_URL}projects/">All</a><a href="{build.PUBLIC_URL}projects/example/index.html#start">Start</a><a href="#local">Local</a>'
        self.assertEqual(build.portable_project_links(page), expected)


if __name__ == "__main__":
    unittest.main()
