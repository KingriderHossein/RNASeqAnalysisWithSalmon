import { stateLabel, formatBytes, acceptRevision, progressText } from "./presentation.mjs";

const $ = (id) => document.getElementById(id);
const host = window.__TAURI__;
let snapshot = null;
let selectedRun = null;
let selectedDatabase = null;
let busy = false;

function showError(message) {
  $("error").textContent = String(message);
  $("error").hidden = false;
}
function clearError() { $("error").hidden = true; }
async function call(command, args = {}) {
  if (!host) throw new Error("This browser view cannot access local jobs. Open the desktop app.");
  return host.core.invoke(command, args);
}
async function action(work) {
  clearError();
  try { await work(); } catch (error) { showError(error?.message ?? error); }
}
function cell(text, className) {
  const element = document.createElement("td");
  element.textContent = text;
  if (className) element.className = className;
  return element;
}

function render(next) {
  if (!next || !acceptRevision(snapshot, next)) return;
  snapshot = next;
  const { job, runs, actions = {}, activity } = snapshot;
  $("job-title").textContent = job.id;
  $("job-summary").textContent = `${runs.length} run${runs.length === 1 ? "" : "s"} · ${runs.filter((run) => run.state === "COMPLETE").length} complete`;
  $("job-state").textContent = snapshot.active ? "Worker active" : stateLabel(job.state);
  $("job-state").hidden = false;
  $("start").textContent = actions.primary_label ?? "Start job";
  $("start").disabled = !actions.primary;
  $("pause").disabled = !actions.pause || snapshot.pending_control === "pause";
  $("cancel").disabled = !actions.cancel || snapshot.pending_control === "cancel";
  $("retry").disabled = !actions.retry;
  $("refresh").disabled = !actions.refresh;
  $("create-job").disabled = snapshot.active || busy;
  $("open-job").disabled = snapshot.active || busy;
  $("control-status").textContent = snapshot.pending_control
    ? `${snapshot.pending_control === "pause" ? "Pause" : "Cancel"} requested. Waiting for a safe stage boundary.`
    : snapshot.active ? "A worker is running. Pause or Cancel and wait for its current stage before closing." : "Checkpoints saved. Completed runs will be skipped.";
  const message = snapshot.worker_error || job.error || snapshot.blocked_reason;
  if (message) showError([message, snapshot.blocked_reason].filter((value, index, array) => value && array.indexOf(value) === index).join("\n\n"));
  else clearError();
  $("empty-state").hidden = true;
  $("run-list").hidden = false;
  $("saved-details").hidden = false;
  $("saved-paths").textContent = `Saved job file: ${snapshot.database}\nOutput folder: ${job.output_root}\n\nSettings: ${job.settings}\nTools: ${job.tools}\n\n${(snapshot.artifacts ?? []).map((artifact) => `${artifact.kind}: ${artifact.path}\nSHA-256: ${artifact.sha256 ?? "not available"}`).join("\n\n")}`;
  $("run-rows").replaceChildren();
  for (const run of runs) {
    const row = document.createElement("tr");
    row.append(cell(run.accession));
    const state = cell(activity?.run_id === run.id ? activity.label : stateLabel(run.state));
    const checkpoint = document.createElement("span");
    checkpoint.className = "checkpoint";
    checkpoint.textContent = `Saved: ${stateLabel(run.state)} · ${run.checkpoint ?? "resolved"}`;
    state.append(checkpoint);
    if (run.error) { const error = document.createElement("span"); error.className = "row-error"; error.textContent = run.error; state.append(error); }
    row.append(state, cell(progressText(run)), cell(String(run.attempts)));
    const details = cell("");
    const button = document.createElement("button");
    button.className = "text-button";
    button.textContent = "View";
    button.setAttribute("aria-label", `View details for ${run.accession}`);
    button.addEventListener("click", () => openRun(run));
    details.append(button); row.append(details); $("run-rows").append(row);
  }
}

function openRun(run) {
  selectedRun = run;
  $("run-detail-title").textContent = run.accession;
  $("run-detail-state").textContent = `Last saved state: ${stateLabel(run.state)}\nCheckpoint: ${run.checkpoint ?? "resolved"}\n${progressText(run)}\nAttempts: ${run.attempts}\n\nSRA: ${run.sra_path ?? "not available"}\nFASTQ paths: ${run.fastq_paths}\n\n${run.error ?? "No recorded run error."}`;
  $("log-content").textContent = "Choose a log stream to read its latest attempt.";
  $("run-details").showModal();
}

async function storage() {
  if (!$("workspace").value.trim()) return;
  const value = $("peak-gib").value;
  const recommendation = value ? (BigInt(value) * 1024n ** 3n).toString() : null;
  const result = await call("inspect_storage", { destination: $("workspace").value.trim(), recommendation });
  $("storage").hidden = false;
  $("free-space").textContent = formatBytes(result.available_bytes);
  $("peak-space").textContent = formatBytes(result.recommended_peak_bytes);
  $("space-warning").hidden = !result.warning;
  // A recommendation warning never changes the core action capabilities.
}

$("create-form").addEventListener("submit", (event) => {
  event.preventDefault();
  if (busy) return;
  void action(async () => {
    busy = true; $("create-job").disabled = true; $("open-job").disabled = true;
    try {
      await storage();
      render(await call("create_download_job", { workspace: $("workspace").value.trim(), id: $("job-id").value.trim(), runs: $("runs").value, threads: Number($("threads").value) }));
      $("job-title").focus();
    } finally { busy = false; $("create-job").disabled = snapshot?.active ?? false; $("open-job").disabled = snapshot?.active ?? false; }
  });
});
$("choose-workspace").addEventListener("click", () => void action(async () => { const path = await call("choose_folder"); if (path) { $("workspace").value = path; await storage(); } }));
$("workspace").addEventListener("change", () => void action(storage));
$("peak-gib").addEventListener("change", () => void action(storage));
$("start").addEventListener("click", () => void action(async () => render(await call("run_download_job", { mode: snapshot.actions.primary_mode }))));
$("retry").addEventListener("click", () => void action(async () => render(await call("run_download_job", { mode: "retry" }))));
$("pause").addEventListener("click", () => void action(async () => render(await call("request_download_control", { intent: "pause" }))));
$("cancel").addEventListener("click", () => void action(async () => {
  if (window.confirm("Cancel unfinished runs in this job? Completed files are kept. The current stage finishes before cancellation is applied.")) render(await call("request_download_control", { intent: "cancel" }));
}));
$("refresh").addEventListener("click", () => void action(async () => render(await call("open_download_job", { database: snapshot.database, id: snapshot.job.id }))));
$("check-tools").addEventListener("click", () => void action(async () => {
  $("tool-details").hidden = false; $("tool-details").textContent = "Checking installed tools…";
  try { const tools = await call("tool_status"); $("tool-details").textContent = tools.map((tool) => `${tool.name}\n${tool.version}\n${tool.path}`).join("\n\n"); }
  catch (error) { $("tool-details").textContent = `${error}\n\nInstall SRA Toolkit and restart the app after updating PATH.`; throw error; }
}));
for (const stream of ["stderr", "stdout"]) $(stream).addEventListener("click", () => void action(async () => { $("log-content").textContent = await call("read_log_tail", { accession: selectedRun.accession, stream }); }));
$("open-job").addEventListener("click", () => void action(async () => {
  const database = await call("choose_database"); if (!database) return;
  const jobs = await call("list_download_jobs", { database }); selectedDatabase = database;
  $("job-options").replaceChildren();
  if (!jobs.length) $("job-options").textContent = "This saved job file has no jobs yet.";
  for (const job of jobs) {
    const button = document.createElement("button"); button.type = "button"; button.className = "job-choice secondary";
    const name = document.createElement("span"); name.textContent = job.id;
    const state = document.createElement("span"); state.textContent = stateLabel(job.state); button.append(name, state);
    button.addEventListener("click", () => void action(async () => { render(await call("open_download_job", { database: selectedDatabase, id: job.id })); $("job-picker").close(); $("job-title").focus(); }));
    $("job-options").append(button);
  }
  $("job-picker").showModal();
}));

if (!host) {
  $("host-notice").hidden = false;
  for (const button of document.querySelectorAll("button")) button.disabled = true;
} else {
  await host.event.listen("download-snapshot", ({ payload }) => render(payload));
  await host.event.listen("download-error", ({ payload }) => showError(payload));
}
