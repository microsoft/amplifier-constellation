"""Real filesystem pointer/app exchange through the Python JSONL client."""

import json
import os
from pathlib import Path
import tempfile
import unittest

from test_driver import DRIVER, Refused, Session, protect


@unittest.skipUnless(DRIVER, "Actual native driver required")
class ActualExchangeTests(unittest.TestCase):
    def test_source_pointer_apply_and_explicit_rollback_preserve_baseline(self):
        with tempfile.TemporaryDirectory(
            dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
        ) as temp:
            root = Path(temp).resolve()
            protect(root)
            current = root / "app"
            current.mkdir()
            protect(current)
            (current / "baseline.txt").write_bytes(b"original baseline bytes")
            target = root / "candidate"
            target.mkdir()
            protect(target)
            (target / "candidate.txt").write_bytes(b"reviewed new source")
            config = root / "owner.json"
            config.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "locks": {"installation": str(root / "installation.lock")},
                        "exchanges": {
                            "app": {
                                "lock": "installation",
                                "profile": {
                                    "current": str(current),
                                    "roots": {"managed": str(root)},
                                },
                            }
                        },
                    }
                )
            )
            protect(config)

            def ref(name):
                return {"root": "managed", "path": name}

            with Session(DRIVER, config) as session:
                old = session.exchange_inspect("app")
                candidate = session.exchange_inspect("app", ref("candidate"))
                with self.assertRaises(Refused):
                    session.exchange_prepare_pointer(
                        "app", ref("prepared"), ref("candidate"), candidate
                    )
                self.assertFalse((root / "prepared").exists())
                with session.lock("installation"):
                    prepared = session.exchange_prepare_pointer(
                        "app", ref("prepared"), ref("candidate"), candidate
                    )
                    self.assertEqual(prepared["kind"], "pointer")
                    applied = session.exchange_apply(
                        "app", ref("prepared"), ref("baseline"), old, prepared
                    )
                    self.assertEqual(applied["current"], prepared)
                    self.assertEqual(applied["retained"], old)
                    self.assertEqual(
                        (current / "candidate.txt").read_bytes(), b"reviewed new source"
                    )
                    self.assertEqual(
                        (root / "baseline" / "baseline.txt").read_bytes(),
                        b"original baseline bytes",
                    )
                    with self.assertRaises(Refused):
                        session.exchange_apply(
                            "app", ref("baseline"), ref("candidate"), prepared, old
                        )
                    with self.assertRaises(Refused):
                        session.exchange_apply(
                            "app", ref("baseline"), ref("failed-candidate"), old, old
                        )
                    restored = session.exchange_apply(
                        "app", ref("baseline"), ref("failed-candidate"), prepared, old
                    )
                    self.assertEqual(restored["current"], old)
                    self.assertEqual(restored["retained"], prepared)
                    self.assertEqual(
                        (current / "baseline.txt").read_bytes(),
                        b"original baseline bytes",
                    )
                    self.assertEqual(
                        (root / "failed-candidate" / "candidate.txt").read_bytes(),
                        b"reviewed new source",
                    )
                    # A lost reply is reconciled through inspection, never a second exchange.
                    self.assertEqual(session.exchange_inspect("app"), old)
                    self.assertIsNone(session.exchange_inspect("app", ref("baseline")))
                    self.assertEqual(
                        session.exchange_inspect("app", ref("failed-candidate")),
                        prepared,
                    )
