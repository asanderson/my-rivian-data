import { spawn, execFile } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { promisify } from 'node:util';
import { root } from './process.mjs';

const executeFile = promisify(execFile);
export const defaultBinary = path.join(root, 'target/release', process.platform === 'win32' ? 'my-rivian-data.exe' : 'my-rivian-data');
const requestBody = JSON.stringify({ operation_id: 'vehicle-state', variables: { vehicle_id: 'demo-r1t-001' } });
const secretPattern = /^[a-f0-9]{64}$/u;

export function parseLaunchUrl(value) {
  let url;
  try { url = new URL(value); } catch { throw new Error('Invalid local launch URL.'); }
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port || url.username || url.password
    || url.pathname !== '/' || url.search || !/^#bootstrap=[a-f0-9]{64}$/u.test(url.hash)
    || value !== `${url.origin}/` + url.hash) throw new Error('Invalid local launch URL.');
  return { origin: url.origin, token: url.hash.slice('#bootstrap='.length) };
}

async function localFetch(url, options = {}) {
  return fetch(url, { ...options, redirect: 'error', signal: AbortSignal.timeout(5000) });
}

export async function createDemoSession(launchUrl) {
  const { origin, token } = parseLaunchUrl(launchUrl);
  let response;
  let body;
  try {
    response = await localFetch(`${origin}/api/bootstrap`, {
      method: 'POST', headers: { Origin: origin, 'Content-Type': 'application/json' }, body: JSON.stringify({ token }),
    });
    body = await response.json();
  } catch { throw new Error('Could not establish the protected local demo session.'); }
  const cookie = response.headers.get('set-cookie')?.split(';', 1)[0];
  if (response.status !== 200 || body?.mode !== 'demo' || !secretPattern.test(body.csrf_token)
    || !secretPattern.test(body.app_capability) || !/^my_rivian_data_\d+_[a-f0-9]{16}=[a-f0-9]{64}$/u.test(cookie ?? '')) {
    throw new Error('Could not establish the protected local demo session.');
  }
  const headers = { Origin: origin, 'Content-Type': 'application/json', Cookie: cookie,
    'X-CSRF-Token': body.csrf_token, 'X-App-Capability': body.app_capability };
  return {
    async executeVehicleState() {
      const started = performance.now();
      try {
        const result = await localFetch(`${origin}/api/execute`, { method: 'POST', headers, body: requestBody });
        // Drain the response for accurate round-trip timing and connection reuse.
        // Never parse or retain vehicle data or error response content.
        await result.arrayBuffer();
        return { status: result.status, latency_ms: performance.now() - started };
      } catch { return { status: 0, latency_ms: performance.now() - started }; }
    },
  };
}

export async function startLocalHost({ binary = defaultBinary, demo = false } = {}) {
  const started = performance.now();
  const child = spawn(binary, demo ? ['--demo', '--no-open', '--print-launch-url'] : [], {
    cwd: root, shell: false, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '';
  let launchUrl;
  let stopped = false;
  let hasExited = false;
  child.stderr.resume(); // Native output can contain secrets: never forward it.
  const exited = new Promise(resolve => child.once('exit', () => { hasExited = true; resolve(); }));
  const stop = async () => {
    if (stopped) return;
    stopped = true;
    output = ''; launchUrl = undefined;
    child.stdout.removeAllListeners('data'); child.stdout.resume();
    if (hasExited || !child.pid) return;
    child.kill(process.platform === 'win32' ? 'SIGTERM' : 'SIGINT');
    const timer = setTimeout(() => { if (!hasExited) child.kill('SIGKILL'); }, 2000);
    await exited;
    clearTimeout(timer);
  };
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => finish(new Error('Local application startup timed out.')), 15000);
      const finish = error => {
        clearTimeout(timer);
        child.stdout.off('data', onData);
        child.off('error', onError); child.off('exit', onExit);
        if (error) reject(error); else resolve();
      };
      const onError = () => finish(new Error('Cannot launch the application. Build the release executable or use --binary.'));
      const onExit = () => finish(new Error('The local application exited before startup.'));
      const onData = chunk => {
        output = (output + chunk.toString('utf8')).slice(-16384);
        if (demo) {
          const match = output.match(/Launch URL \(keep private\): (http:\/\/127\.0\.0\.1:\d+\/#bootstrap=[a-f0-9]{64})(?:\r?\n)/u);
          if (match) {
            launchUrl = match[1];
            try { parseLaunchUrl(launchUrl); finish(); } catch { finish(new Error('Invalid native launch URL.')); }
          }
        } else if (/My Rivian Data: Rivian account access at http:\/\/127\.0\.0\.1:\d+(?:\r?\n)/u.test(output)) finish();
      };
      child.on('error', onError); child.once('exit', onExit); child.stdout.on('data', onData);
    });
    output = '';
    child.stdout.resume();
    if (demo) {
      try {
        const { origin } = parseLaunchUrl(launchUrl);
        const response = await localFetch(`${origin}/api/health`);
        const health = await response.json();
        if (response.status !== 200 || health.mode !== 'demo' || health.status !== 'ok') throw new Error();
      } catch { throw new Error('The local demo application did not become ready.'); }
    }
    return {
      pid: child.pid, startup_ms: performance.now() - started, stop,
      // The origin contains no launch capability or account data.
      origin: demo ? parseLaunchUrl(launchUrl).origin : null,
      isRunning: () => !hasExited,
      async demoSession() {
        if (!demo || !launchUrl) throw new Error('A protected demo session is unavailable.');
        const value = launchUrl; launchUrl = undefined;
        return createDemoSession(value);
      },
    };
  } catch (error) { await stop(); throw error; }
}

function validSample(rssBytes, cpuSeconds) {
  if (!Number.isSafeInteger(rssBytes) || rssBytes < 0 || !Number.isFinite(cpuSeconds) || cpuSeconds < 0) {
    throw new Error('Backend process metrics are unavailable.');
  }
  return { rss_bytes: rssBytes, cpu_seconds: cpuSeconds };
}

export function parseLinuxProcess(stat, status, clockTicks) {
  // Fields after the final ')' start at stat field 3. Child CPU fields are
  // deliberately excluded: only utime (14) and stime (15) belong to the backend.
  const fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/u);
  const rss = status.match(/^VmRSS:\s+(\d+)\s+kB$/mu)?.[1];
  if (!rss || !Number.isFinite(clockTicks) || clockTicks <= 0) throw new Error('Backend process metrics are unavailable.');
  return validSample(Number(rss) * 1024, (Number(fields[11]) + Number(fields[12])) / clockTicks);
}

export function parseMacProcess(value) {
  const match = value.trim().match(/^(\d+)\s+(?:(\d+)-)?(?:(\d+):)?(\d+):(\d+(?:\.\d+)?)$/u);
  if (!match) throw new Error('Backend process metrics are unavailable.');
  return validSample(Number(match[1]) * 1024, Number(match[2] ?? 0) * 86400 + Number(match[3] ?? 0) * 3600 + Number(match[4]) * 60 + Number(match[5]));
}

export function parseWindowsProcess(value) {
  let sample;
  try { sample = JSON.parse(value); } catch { throw new Error('Backend process metrics are unavailable.'); }
  if (Object.keys(sample ?? {}).sort().join(',') !== 'cpu_seconds,rss_bytes') throw new Error('Backend process metrics are unavailable.');
  return validSample(sample.rss_bytes, sample.cpu_seconds);
}

let clockTicks;
const linuxProcessIds = new Map();
async function linuxProcess(pid) {
  const parentStatus = await readFile('/proc/self/status', 'utf8');
  const parent = parentStatus.match(/^Tgid:\s+(\d+)$/mu)?.[1];
  const belongsToChild = status => {
    const namespaces = status.match(/^NSpid:\s+([\d\s]+)$/mu)?.[1]?.trim().split(/\s+/u);
    const localPid = namespaces?.at(-1) ?? status.match(/^Pid:\s+(\d+)$/mu)?.[1];
    return Number(localPid) === pid && status.match(/^PPid:\s+(\d+)$/mu)?.[1] === parent;
  };
  // Containerized execution may expose outer /proc PIDs while spawn returns an
  // inner PID. Verify namespace identity AND parent before reading any metrics.
  const candidates = new Set([String(pid), linuxProcessIds.get(pid)].filter(Boolean));
  for (const candidate of candidates) {
    try {
      const status = await readFile(`/proc/${candidate}/status`, 'utf8');
      if (belongsToChild(status)) return { candidate, status };
    } catch { /* A child may have exited; do not sample a reused PID. */ }
  }
  for (const candidate of await readdir('/proc')) {
    if (!/^\d+$/u.test(candidate)) continue;
    try {
      const status = await readFile(`/proc/${candidate}/status`, 'utf8');
      if (belongsToChild(status)) {
        linuxProcessIds.set(pid, candidate); return { candidate, status };
      }
    } catch { /* Processes disappear while enumerating /proc. */ }
  }
  throw new Error('Backend process identity could not be verified.');
}
export async function sampleBackend(pid) {
  if (!Number.isSafeInteger(pid) || pid < 1) throw new Error('Backend process metrics are unavailable.');
  try {
    if (process.platform === 'linux') {
      clockTicks ??= executeFile('getconf', ['CLK_TCK'], { timeout: 5000, maxBuffer: 4096 }).then(result => Number(result.stdout.trim()));
      const { candidate, status } = await linuxProcess(pid);
      const [stat, ticks] = await Promise.all([readFile(`/proc/${candidate}/stat`, 'utf8'), clockTicks]);
      return parseLinuxProcess(stat, status, ticks);
    }
    if (process.platform === 'darwin') {
      const { stdout } = await executeFile('ps', ['-p', String(pid), '-o', 'rss=', '-o', 'time='], { timeout: 5000, maxBuffer: 4096 });
      return parseMacProcess(stdout);
    }
    if (process.platform === 'win32') {
      const command = `$rivianProcess = Get-Process -Id ${pid} -ErrorAction Stop; [pscustomobject]@{rss_bytes = $rivianProcess.WorkingSet64; cpu_seconds = $rivianProcess.TotalProcessorTime.TotalSeconds} | ConvertTo-Json -Compress`;
      const { stdout } = await executeFile('powershell.exe', ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', command], { timeout: 5000, maxBuffer: 4096, windowsHide: true });
      return parseWindowsProcess(stdout);
    }
  } catch { /* Output and native errors may contain private data; discard them. */ }
  throw new Error('Backend process metrics are unavailable on this host.');
}
