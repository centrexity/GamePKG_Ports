#!/usr/bin/env python3
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import opence_checkout as oc


class OpenCECheckoutTests(unittest.TestCase):
    def test_pin_is_unvendored_agpl_commit(self):
        pin = oc.load_pin()
        self.assertEqual(pin["commit"], "ce95f7deed5bbf726e50a9d5fe5227437194cf3b")
        self.assertFalse(pin["vendored"])
        self.assertEqual(pin["profile"], "opence-ce95f7de")
        names = [p.name for p in (oc.PORTS / "halo").iterdir()]
        self.assertIn("manifest.toml", names)
        self.assertNotIn("UPSTREAM.json", names)
        self.assertNotIn("README.md", names)

    def test_catalog_publishes_the_selector_without_game_data(self):
        catalog = tomllib.loads((oc.PORTS / "ports.gamepkglist").read_text())
        halo = next(game for game in catalog["games"] if game["id"] == "com.gamepkg.halo")
        self.assertEqual(halo["version"], "0.1.0")
        self.assertFalse(halo["requires_rom"])
        self.assertIn("i686", halo["description"])
        self.assertIn("No Halo assets", halo["description"])
        self.assertTrue(halo["download_url"].endswith("/halo.gamepkg"))
        self.assertTrue((oc.PORTS / "halo" / "manifest.toml").is_file())
        self.assertFalse((oc.PORTS / "halo" / "manifest.draft.toml").exists())

    def test_forbidden_names_are_game_data(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "README").write_text("notes")
            self.assertEqual(oc.forbidden_game_data(root), [])
            for relative in (
                "disc.ISO",
                "nested/game.xiso",
                "maps/bloodgulch.map",
                "custom_maps/bitmaps.map",
                "custom_maps/notes.txt",
                "Maps/a.map",
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"not a real disc")
            found = oc.forbidden_game_data(root)
            self.assertIn("disc.ISO", found)
            self.assertIn("nested/game.xiso", found)
            self.assertIn("maps/", found)
            self.assertIn("maps/bloodgulch.map", found)
            self.assertIn("custom_maps/notes.txt", found)
            self.assertIn("Maps/a.map", found)
            self.assertNotIn("README", found)

    def test_destination_inside_the_repository_is_rejected(self):
        with self.assertRaises(oc.CheckoutError):
            oc.assert_outside_repository(oc.PORTS / "halo" / "checkout")
        with tempfile.TemporaryDirectory() as raw:
            outside = Path(raw) / "opence-src"
            self.assertEqual(oc.assert_outside_repository(outside), outside.resolve())

    def test_license_digest_must_match(self):
        pin = oc.load_pin()
        with tempfile.TemporaryDirectory() as raw:
            checkout = Path(raw)
            (checkout / "LICENSE").write_text("GNU AFFERO GENERAL PUBLIC LICENSE\n")
            with self.assertRaises(oc.CheckoutError):
                oc.check_license(checkout, pin)

    def test_real_checkout_when_present(self):
        checkout = Path("/tmp/OpenCE")
        if not checkout.is_dir():
            self.skipTest("no local OpenCE checkout")
        pin = oc.verify(checkout)
        self.assertEqual(pin["commit"], "ce95f7deed5bbf726e50a9d5fe5227437194cf3b")

if __name__ == "__main__":
    unittest.main()
