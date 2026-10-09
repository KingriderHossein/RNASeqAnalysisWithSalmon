import test from "node:test";
import assert from "node:assert/strict";
import { formatBytes, acceptRevision, progressText, controlStatus } from "../presentation.mjs";

test("unknown size stays unknown, without a percentage", () => {
  const text = progressText({ observed_bytes: "1048576", total_bytes: null });
  assert.equal(text, "1.0 MiB observed · total size unknown");
  assert.ok(!text.includes("%"));
  assert.equal(formatBytes(null), "Unknown");
});
test("large byte counts retain integer precision", () => {
  assert.equal(formatBytes("18446744073709551615"), "16383.9 PiB");
  assert.equal(formatBytes("0"), "0 B");
});
test("late command acceptance cannot replace a newer event", () => {
  assert.equal(acceptRevision({ revision: "9007199254740994" }, { revision: "9007199254740993" }), false);
  assert.equal(acceptRevision({ revision: "8" }, { revision: "9" }), true);
});

test("control acknowledgement describes the active stage without declaring it stopped", () => {
  assert.match(controlStatus({ active: true, pending_control: "pause", activity: { stage: "finalization" } }), /Pause requested.*safe buffer boundary/);
  for (const stage of ["acquisition", "conversion", undefined]) {
    assert.match(controlStatus({ active: true, pending_control: "cancel", activity: { stage } }), /Cancel requested.*external stage or next safe checkpoint/);
  }
  assert.match(controlStatus({ active: true }), /wait for it to stop before closing/);
  assert.match(controlStatus({ active: false }), /Checkpoints saved/);
});
