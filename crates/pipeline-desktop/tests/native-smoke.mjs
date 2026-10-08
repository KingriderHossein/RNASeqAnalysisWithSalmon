// Real Linux Tauri/WebKitGTK window and IPC; synthetic executable tools only.
// The fixture writes small text/SRA placeholders, never retrieves sequencing data.
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, chmod, readFile, rename, access, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join, delimiter } from "node:path";
import { spawn } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

const fixture = await mkdtemp(join(tmpdir(), "module-a-native-synthetic-"));
const tools = join(fixture, "tools");
const workspace = join(fixture, "workspace");
const batchFile = join(fixture, "runs with spaces.csv");
await writeFile(batchFile, "run,notes\nsrr900001,DRR999999\nERR900002,tumor\nSRR900001,normal\n");
await mkdir(tools); await mkdir(workspace);
const script = `#!/usr/bin/env python3
import sys, pathlib, time
tool = pathlib.Path(sys.argv[0]).name
if '--version' in sys.argv or '-V' in sys.argv:
    print(tool + ' synthetic-e2e-1')
    sys.exit(0)
args = sys.argv[1:]
if tool == 'prefetch':
    time.sleep(2)
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
  await until(() => execute("return !!window.__TAURI__ && document.getElementById('host-notice').hidden;"), "native bridge initialization");
  await execute(`document.getElementById('batch-path').value = arguments[0];
    document.getElementById('preview-batch').click();`, [batchFile]);
  await until(() => execute("return document.getElementById('batch-status').textContent.includes('2 unique runs loaded');"), "batch file preview through native IPC");
  assert.equal(await execute("return document.getElementById('runs').value;"), "SRR900001\nERR900002");
  assert.match(await execute("return document.getElementById('batch-status').textContent;"), /1 duplicate.*1 metadata/);
  assert.equal(await execute("return document.getElementById('start').disabled;"), true);
  await writeFile(batchFile, "run\nSRR1\nSRX2");
  await execute("document.getElementById('preview-batch').click();");
  await until(() => execute("return !document.getElementById('error').hidden;"), "invalid batch rejected");
  assert.equal(await execute("return document.getElementById('runs').value;"), "SRR900001\nERR900002");
  await execute(`document.getElementById('job-id').value = 'native-synthetic';
    document.getElementById('workspace').value = arguments[0];
    document.getElementById('create-form').requestSubmit();`, [workspace]);
  await until(() => execute("return document.getElementById('job-title').textContent === 'native-synthetic';"), "create through native IPC");
  assert.equal(await execute("return document.querySelectorAll('#run-rows tr').length;"), 2);
  console.log("VISUAL_QUEUED:" + await request("GET", `/session/${session}/screenshot`));
  await execute("document.getElementById('start').click();");
  await until(() => execute("return !document.getElementById('pause').disabled && document.getElementById('run-rows').textContent.includes('Acquiring reads');"), "acquisition stage started");
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
  console.log("VISUAL_COMPLETE:" + await request("GET", `/session/${session}/screenshot`));
  await request("POST", `/session/${session}/window/rect`, { width: 900, height: 650 });
  assert.ok(await execute("return document.documentElement.scrollWidth <= window.innerWidth;"));
  console.log("VISUAL_MINIMUM:" + await request("GET", `/session/${session}/screenshot`));

  // The GUI must surface core contention while another real CLI owns SQLite.
  await execute(`document.getElementById('job-id').value = 'native-contention';
    document.getElementById('runs').value = 'DRR900003';
    document.getElementById('create-form').requestSubmit();`);
  await until(() => execute("return document.getElementById('job-title').textContent === 'native-contention';"), "second synthetic job");
  const cli = spawn(resolve("target/debug/rnaseq-pipeline"), ["start", join(workspace, "module-a.sqlite"), "native-contention"], {
    env: { ...process.env, PATH: tools + delimiter + process.env.PATH }, stdio: ["ignore", "pipe", "pipe"],
  });
  const cliExit = new Promise((accept, reject) => { cli.on("error", reject); cli.on("exit", (code) => accept(code)); });
  let cliLogs = ""; for (const stream of [cli.stdout, cli.stderr]) stream.on("data", (data) => { cliLogs += data; });
  await until(async () => { try { await access(join(workspace, "native-contention/logs/DRR900003-attempt-1-prefetch.stdout.log")); return true; } catch { return false; } }, "CLI holds job database");
  await execute("document.getElementById('refresh').click();");
  await until(() => execute("return !document.getElementById('error').hidden;"), "GUI Busy error");
  assert.match(await execute("return document.getElementById('error').textContent;"), /owned|busy|lock/i);
  assert.equal(await cliExit, 0, cliLogs);
  await execute("document.getElementById('refresh').click();");
  await until(() => execute("return document.getElementById('job-state').textContent === 'Complete' && document.getElementById('error').hidden;"), "refresh after CLI release");
  assert.equal(await readFile(manifest, "utf8"), original);

  await rename(join(tools, "prefetch"), join(tools, "prefetch-disabled"));
  await execute("document.getElementById('check-tools').click();");
  await until(() => execute("return document.getElementById('tool-details').textContent.includes('Install SRA Toolkit');"), "missing toolkit guidance");
  await rename(join(tools, "prefetch-disabled"), join(tools, "prefetch"));
  console.log("PASS: real Linux Tauri IPC create/batch/start/pause/resume/complete/refresh; synthetic tools only; checksum retained; unknown totals have no percentage; minimum window has no page overflow.");
  console.log("PASS: real CLI ownership produces GUI Busy, Refresh clears the error after release, completed sibling manifest retained, missing toolkit has installation guidance.");
  console.log("PASS: real Linux batch-file Preview normalizes/deduplicates, ignores metadata, never starts a download, rejects study input and preserves the previous list before Create.");
} catch (error) {
  console.error(diagnostics);
  if (session) { try { console.error(await execute("return document.body.innerText;")); } catch {} }
  throw error;
} finally {
  if (session) { try { await request("DELETE", `/session/${session}`); } catch {} }
  driver.kill("SIGTERM");
  await rm(fixture, { recursive: true, force: true });
}
