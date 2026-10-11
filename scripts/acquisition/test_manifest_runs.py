"""Offline tests for sample-locked SRA accession export."""

import csv
import tempfile
import unittest
from pathlib import Path

from manifest_runs import main, planned_runs


class ManifestExportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifest = self.root / "manifest.tsv"
        with self.manifest.open("w", encoding="utf-8", newline="") as f:
            writer = csv.DictWriter(f, fieldnames=["gsm", "run", "library_layout"], delimiter="\t")
            writer.writeheader()
            for n in range(32):
                writer.writerow({"gsm": f"GSM{20000+n}", "run": f"SRR{30000+n}", "library_layout": "SINGLE"})

    def test_export_and_reuse_exact_contents(self):
        dest = self.root / "new" / "runs.txt"
        self.assertEqual(main(["--manifest", str(self.manifest), "--output", str(dest)]), 0)
        self.assertEqual(main(["--manifest", str(self.manifest), "--output", str(dest)]), 0)
        self.assertEqual(len([s for s in dest.read_text().splitlines() if s.startswith("SRR")]), 32)
        self.assertIn("manifest_sha256=", dest.read_text())

    def test_refuses_conflicting_output(self):
        dest = self.root / "runs.txt"
        dest.write_text("important existing file\n")
        self.assertEqual(main(["--manifest", str(self.manifest), "--output", str(dest)]), 2)
        self.assertEqual(dest.read_text(), "important existing file\n")

    def test_duplicate_rejected(self):
        txt = self.manifest.read_text()
        self.manifest.write_text(txt.replace("SRR30001", "SRR30000"))
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            planned_runs(self.manifest)

    def test_wrong_layout_rejected(self):
        txt = self.manifest.read_text()
        self.manifest.write_text(txt.replace("SINGLE", "PAIRED", 1))
        with self.assertRaisesRegex(ValueError, "Non-single-end"):
            planned_runs(self.manifest)

    def test_missing_run_rejected(self):
        txt = self.manifest.read_text()
        self.manifest.write_text("\n".join(txt.splitlines()[:-1]) + "\n")
        with self.assertRaisesRegex(ValueError, "exactly 32"):
            planned_runs(self.manifest)


if __name__ == "__main__":
    unittest.main()
