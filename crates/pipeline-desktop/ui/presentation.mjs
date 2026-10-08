export const stateLabel = (state) => ({
  QUEUED: "Queued", READY: "Ready", DOWNLOADING: "Acquiring reads", DOWNLOADED: "Downloaded",
  VALIDATING: "Validating SRA", SRA_VALID: "SRA validated", CONVERTING: "Converting FASTQ",
  FASTQ_READY: "FASTQ ready", COMPRESSING: "Compressing", CHECKSUMMING: "Verifying checksums",
  PAUSED: "Paused", PAUSED_AT_BOUNDARY: "Paused at a checkpoint", FAILED: "Needs attention",
  COMPLETE: "Complete", CANCELLED: "Cancelled", ACTIVE: "Running", WAITING_FOR_NETWORK: "Waiting for network",
}[state] ?? state ?? "Unknown");

export function formatBytes(value) {
  if (value == null) return "Unknown";
  const bytes = BigInt(value);
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let divisor = 1n;
  let unit = 0;
  while (unit < units.length - 1 && bytes >= divisor * 1024n) { divisor *= 1024n; unit += 1; }
  if (unit === 0) return `${bytes} B`;
  const tenths = bytes * 10n / divisor;
  return `${tenths / 10n}.${tenths % 10n} ${units[unit]}`;
}

export function acceptRevision(current, next) {
  return BigInt(next?.revision ?? "0") >= BigInt(current?.revision ?? "0");
}

export function progressText(run) {
  return `${formatBytes(run.observed_bytes)} observed · total size ${run.total_bytes == null ? "unknown" : formatBytes(run.total_bytes)}`;
}
