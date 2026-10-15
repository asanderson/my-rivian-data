import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { main, root } from './lib/process.mjs';
import { sha256 } from './lib/delivery.mjs';
import { buildEvidence } from './lib/build-evidence.mjs';
import { defaultBinary, sampleBackend, startLocalHost } from './lib/local-host.mjs';

export function benchmarkOptions(args) {
  const options = { demo: false, duration_seconds: 10, concurrency: 2, sample_interval_ms: 500 };
  const numeric = { '--duration-seconds': ['duration_seconds', 1, 86400], '--concurrency': ['concurrency', 1, 32], '--sample-interval-ms': ['sample_interval_ms', 100, 60000] };
  const seen = new Set();
  for (let index = 0; index < args.length; index++) {
    const flag = args[index];
    if (seen.has(flag)) throw new Error('Benchmark options must not be repeated.');
    seen.add(flag);
    if (flag === '--demo') options.demo = true;
    else if (flag === '--binary') {
      const value = args[++index];
      if (!value || value.startsWith('--')) throw new Error('--binary needs an executable path.');
      options.binary = path.resolve(value);
    } else if (numeric[flag]) {
      const [key, minimum, maximum] = numeric[flag];
      const value = Number(args[++index]);
      if (!Number.isSafeInteger(value) || value < minimum || value > maximum) throw new Error(`${flag} must be an integer from ${minimum} to ${maximum}.`);
      options[key] = value;
    } else throw new Error('Unknown benchmark option. Use --help.');
  }
  if (!options.demo) throw new Error('The synthetic benchmark requires explicit --demo; live-account workloads are not automated.');
  return options;
}

// Retain deterministic, evenly spaced observations while bounding memory and
// report size during day-long runs. Counts/min/max/means still cover every request.
function boundedSamples(limit = 10000) {
  let values = [];
  let stride = 1;
  return {
    add(sequence, value) {
      if (sequence % stride) return;
      if (values.length >= limit) {
        stride *= 2;
        values = values.filter(entry => entry.sequence % stride === 0);
        if (sequence % stride) return;
      }
      values.push({ sequence, ...value });
    },
    result() { return { sequence_stride: stride, observations: values }; },
  };
}

function quantile(sorted, fraction) {
  return sorted.length ? sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)] : null;
}

function payloadBytes(directory) {
  let total = 0;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) total += payloadBytes(file);
    else if (entry.isFile()) total += statSync(file).size;
  }
  return total;
}

function artifactDisk(binary) {
  const result = { executable_bytes: statSync(binary).size, executable_sha256: sha256(binary), portable_archive_bytes: null, installed_payload_bytes: null };
  const dist = path.join(root, 'dist');
  if (!existsSync(dist)) return result;
  const suffix = `-${process.platform}-${process.arch}.tar.gz`;
  const packages = readdirSync(dist).filter(name => {
    if (!/^my-rivian-data-[0-9A-Za-z.-]+$/u.test(name) || !name.endsWith(suffix)) return false;
    const payload = path.join(dist, name.slice(0, -'.tar.gz'.length));
    try {
      const build = JSON.parse(readFileSync(path.join(payload, 'BUILD.json'), 'utf8'));
      return build.sha256 === result.executable_sha256;
    } catch { return false; }
  });
  if (packages.length) {
    const archive = packages.map(name => path.join(dist, name)).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
    result.portable_archive_bytes = statSync(archive).size;
    const payload = archive.slice(0, -'.tar.gz'.length);
    if (existsSync(payload) && statSync(payload).isDirectory()) result.installed_payload_bytes = payloadBytes(payload);
  }
  return result;
}

export async function runBenchmark(options) {
  const binary = options.binary ?? defaultBinary;
  const build = buildEvidence(binary);
  const disk = artifactDisk(binary);
  const startedAt = new Date().toISOString();
  const host = await startLocalHost({ binary, demo: true });
  const controller = new AbortController();
  const interrupt = () => controller.abort();
  process.once('SIGINT', interrupt); process.once('SIGTERM', interrupt);
  const latencySamples = boundedSamples();
  const processSamples = boundedSamples();
  let requests = 0; let failed = 0; let totalLatency = 0; let minLatency = Infinity; let maxLatency = 0;
  let metricSamples = 0; let metricFailures = 0; let peakRss = 0; let sumRss = 0;
  let firstMetric; let lastMetric;
  const statuses = {};
  let started;
  let runtime;
  let sessionStartup;
  const sample = async () => {
    try {
      const metrics = await sampleBackend(host.pid);
      const elapsed = performance.now() - started;
      const observation = { elapsed_ms: elapsed, ...metrics };
      if (lastMetric) {
        const wallSeconds = (elapsed - lastMetric.elapsed_ms) / 1000;
        observation.cpu_percent_one_core = wallSeconds > 0 ? Math.max(0, (metrics.cpu_seconds - lastMetric.cpu_seconds) / wallSeconds * 100) : null;
      } else observation.cpu_percent_one_core = null;
      firstMetric ??= observation;
      lastMetric = observation;
      peakRss = Math.max(peakRss, metrics.rss_bytes); sumRss += metrics.rss_bytes;
      processSamples.add(metricSamples++, observation);
    } catch { metricFailures++; }
  };
  try {
    const sessionStarted = performance.now();
    const session = await host.demoSession();
    sessionStartup = performance.now() - sessionStarted;
    started = performance.now();
    const deadline = started + options.duration_seconds * 1000;
    await sample();
    const sampling = (async () => {
      while (!controller.signal.aborted && performance.now() < deadline && host.isRunning()) {
        const delay = Math.min(options.sample_interval_ms, Math.max(0, deadline - performance.now()));
        try { await new Promise(resolve => {
          const finish = () => { clearTimeout(timer); controller.signal.removeEventListener('abort', finish); resolve(); };
          const timer = setTimeout(finish, delay);
          controller.signal.addEventListener('abort', finish, { once: true });
        }); } catch { break; }
        if (!controller.signal.aborted) await sample();
      }
    })();
    const worker = async () => {
      while (!controller.signal.aborted && performance.now() < deadline && host.isRunning()) {
        const result = await session.executeVehicleState();
        const sequence = requests++;
        statuses[result.status] = (statuses[result.status] ?? 0) + 1;
        if (result.status !== 200) failed++;
        totalLatency += result.latency_ms;
        minLatency = Math.min(minLatency, result.latency_ms); maxLatency = Math.max(maxLatency, result.latency_ms);
        latencySamples.add(sequence, { elapsed_ms: performance.now() - started, ...result });
      }
    };
    await Promise.all([sampling, ...Array.from({ length: options.concurrency }, worker)]);
    await sample();
    runtime = performance.now() - started;
    const rawLatencies = latencySamples.result();
    const sorted = rawLatencies.observations.map(value => value.latency_ms).sort((a, b) => a - b);
    const cpuWindow = firstMetric && lastMetric ? (lastMetric.elapsed_ms - firstMetric.elapsed_ms) / 1000 : 0;
    const cpuSeconds = cpuWindow > 0 ? Math.max(0, lastMetric.cpu_seconds - firstMetric.cpu_seconds) : null;
    const summary = {
      schema_version: 1, mode: 'demo', workload: 'synthetic fixed vehicle-state', scope: 'native backend process only',
      started_at: startedAt, platform: process.platform, architecture: process.arch,
      build,
      configuration: { duration_seconds: options.duration_seconds, concurrency: options.concurrency, sample_interval_ms: options.sample_interval_ms },
      artifact_disk: disk,
      startup: { health_ready_ms: host.startup_ms, protected_session_ms: sessionStartup },
      runtime_ms: runtime, interrupted: controller.signal.aborted, backend_exited_early: !host.isRunning(),
      requests: { total: requests, succeeded: requests - failed, failed, statuses, per_second: requests / (runtime / 1000) },
      latency_ms: { min: requests ? minLatency : null, mean: requests ? totalLatency / requests : null, max: requests ? maxLatency : null,
        p50: quantile(sorted, 0.5), p95: quantile(sorted, 0.95), p99: quantile(sorted, 0.99),
        percentile_sample_count: sorted.length, percentile_sequence_stride: rawLatencies.sequence_stride },
      backend: { sampler: { linux: 'proc', darwin: 'ps', win32: 'powershell Get-Process' }[process.platform] ?? 'unsupported',
        metric_samples: metricSamples, metric_unavailable_count: metricFailures,
        rss_peak_bytes: metricSamples ? peakRss : null, rss_sample_mean_bytes: metricSamples ? sumRss / metricSamples : null,
        cpu_seconds: cpuSeconds, cpu_mean_percent_one_core: cpuWindow > 0 ? cpuSeconds / cpuWindow * 100 : null },
      limitations: ['Synthetic local data only; no Rivian account or API acceptance.', 'Backend excludes browser, benchmark driver and child processes.',
        'RSS is sampled; CPU percentage uses one core as 100% and may exceed 100%.', 'Latency percentiles use bounded deterministic observations when sequence stride exceeds one.',
        'No battery or power measurement. Other operating systems require their own runs.'],
    };
    const directory = path.join(root, 'artifacts/benchmarks', `${startedAt.replace(/[:.]/gu, '-')}-${process.platform}-${process.arch}`);
    mkdirSync(directory, { recursive: true });
    writeFileSync(path.join(directory, 'raw.json'), `${JSON.stringify({ schema_version: 1, mode: 'demo', latency: rawLatencies, backend: processSamples.result() }, null, 2)}\n`);
    writeFileSync(path.join(directory, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`);
    return { summary, directory };
  } finally {
    controller.abort(); process.off('SIGINT', interrupt); process.off('SIGTERM', interrupt);
    await host.stop();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main(async () => {
    if (process.argv.slice(2).some(arg => arg === '--help' || arg === '-h')) {
      console.log('Usage: node scripts/benchmark.mjs --demo [--duration-seconds 10] [--concurrency 2] [--sample-interval-ms 500] [--binary PATH]\nSynthetic local vehicle-state workload. Measures the native backend only. Writes raw.json and summary.json under artifacts/benchmarks/.');
      return;
    }
    const { summary, directory } = await runBenchmark(benchmarkOptions(process.argv.slice(2)));
    console.log(`Synthetic backend benchmark: ${summary.requests.total} requests, ${summary.requests.failed} failed; p95 ${summary.latency_ms.p95?.toFixed(3) ?? 'unavailable'} ms; peak RSS ${summary.backend.rss_peak_bytes ?? 'unavailable'} bytes.\nReports: ${directory}`);
    if (summary.requests.failed || summary.backend_exited_early || !summary.backend.metric_samples) process.exitCode = 1;
  });
}
