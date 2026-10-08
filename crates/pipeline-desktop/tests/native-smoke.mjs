// Real Linux Tauri/WebKitGTK window and IPC; synthetic executable tools only.
// The fixture writes small text/SRA placeholders, never retrieves sequencing data.
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, chmod, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join, delimiter } from "node:path";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

const fixture = await mkdtemp(join(tmpdir(), "module-a-native-synthetic-"));
const tools = join(fixture, "tools");
const workspace = join(fixture, "workspace");
await mkdir(tools); await mkdir(workspace);
const script = `#!/usr/bin/env python3
import sys, pathlib, time
tool = pathlib.Path(sys.argv[0]).name
if '--version' in sys.argv or '-V' in sys.argv:
    print(tool + ' synthetic-e2e-1')
    sys.exit(0)
args = sys.argv[1:]
if tool == 'prefetch':
    time.sleep(1)
    accession = args[0]
    root = pathlib.Path(args[args.index('-O') + 1]) / accession
    root.mkdir(parents=True, exist_ok=True)
    (root / (accession + '.sra')).write_bytes(b'synthetic placeholder')
elif tool == 'fasterq-dump':
    root = pathlib.Path(args[args.index('-O') + 1])
    accession = pathlib.Path(args[0]).name
    (root / (accession + '.fastq')).write_bytes(b'@synthetic\\nACGT\\n+\\nIIII\\n')
print('synthetic ' + tool + ' complete')
`;
for (const name of ["prefetch", "vdb-validate", "fasterq-dump"]) {
  const path = join(tools, name); await writeFile(path, script); await chmod(path, 0o755);
}
const driver = spawn("tauri-driver", [], {
  env: { ...process.env, PATH: tools + delimiter + process.env.PATH },
  stdio: ["ignore", "pipe", "pipe"],
});
let diagnostics = "";
for (const stream of [driver.stdout, driver.stderr]) stream.on("data", (data) => { diagnostics = (diagnostics + data).slice(-16000); });
let session;
async function request(method, path, body) {
  const response = await fetch(`http://127.0.0.1:4444${path}`, {
    method, headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(45000),
  });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw new Error(JSON.stringify(result));
  return result.value;
}
async function execute(script, args = []) { return request("POST", `/session/${session}/execute/sync`, { script, args }); }
async function until(predicate, label) {
  const deadline = Date.now() + 45000;
  while (Date.now() < deadline) { if (await predicate()) return; await delay(250); }
  throw new Error(`Timed out: ${label}`);
}
try {
  await until(async () => { try { await request("GET", "/status"); return true; } catch { return false; } }, "driver readiness");
  const opened = await request("POST", "/session", { capabilities: { alwaysMatch: { "tauri:options": { application: resolve("target/debug/rnaseq-desktop") } } } });
  session = opened.sessionId;
  assert.ok(session);
  await until(() => execute("return !!window.__TAURI__ && !document.getElementById('host-notice').hidden === false;"), "native bridge initialization");
  await execute(`document.getElementById('job-id').value = 'native-synthetic';
    document.getElementById('runs').value = 'SRR900001\\nERR900002';
    document.getElementById('workspace').value = arguments[0];
    document.getElementById('create-form').requestSubmit();`, [workspace]);
  await until(() => execute("return document.getElementById('job-title').textContent === 'native-synthetic';"), "create through native IPC");
  assert.equal(await execute("return document.querySelectorAll('#run-rows tr').length;"), 2);
  await execute("document.getElementById('start').click();");
  await until(() => execute("return !document.getElementById('pause').disabled;"), "active worker");
  await execute("document.getElementById('pause').click();");
  await until(() => execute("return document.getElementById('job-state').textContent === 'Paused';"), "safe pause");
  assert.ok((await execute("return document.getElementById('run-rows').textContent;")).includes("SRA validated"));
  await execute("document.getElementById('start').click();");
  await until(() => execute("return document.getElementById('job-state').textContent === 'Complete';"), "resume and verify");
  await until(() => execute("return !document.getElementById('refresh').disabled;"), "worker lease released");
  assert.equal(await execute("return document.getElementById('start').disabled;"), true);
  const manifest = join(workspace, "native-synthetic/fastq/SRR900001/compressed/SHA256SUMS");
  const original = await readFile(manifest, "utf8");
  assert.match(original, /^[0-9a-f]{64}  SRR900001.fastq.gz/m);
  await execute("document.getElementById('refresh').click();");
  await until(() => execute("return document.getElementById('job-state').textContent === 'Complete';"), "refresh saved state");
  assert.equal(await readFile(manifest, "utf8"), original);
  assert.ok(!(await execute("return document.getElementById('run-rows').textContent;")).includes("%"));
  assert.equal(await execute("return document.getElementById('error').hidden;"), true);
  console.log("PASS: real Linux Tauri IPC create/batch/start/pause/resume/complete/refresh; synthetic tools only; checksum retained; unknown totals have no percentage.");
} catch (error) {
  console.error(diagnostics);
  if (session) { try { console.error(await execute("return document.body.innerText;")); } catch {} }
  throw error;
} finally {
  if (session) { try { await request("DELETE", `/session/${session}`); } catch {} }
  driver.kill("SIGTERM");
  await rm(fixture, { recursive: true, force: true });
}
