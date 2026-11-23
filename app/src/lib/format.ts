const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Fixed one-decimal sizes so live figures never change width. */
export function bytes(n: number | null | undefined): string {
  if (n == null) return '—';
  let i = 0;
  let v = n;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 ? v.toFixed(0) : v < 10 ? v.toFixed(2) : v < 100 ? v.toFixed(1) : v.toFixed(0)} ${UNITS[i]}`;
}

export function speed(n: number | null | undefined): string {
  if (!n) return '—';
  return `${bytes(n)}/s`;
}

/** Split a speed into number and unit for the big board figure. */
export function speedParts(n: number): [string, string] {
  if (!n) return ['0.0', 'MB/s'];
  const s = bytes(n);
  const [v, u] = s.split(' ');
  return [v, `${u}/s`];
}

export function duration(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return '—';
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m}m ${String(secs % 60).padStart(2, '0')}s`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h}h ${String(m % 60).padStart(2, '0')}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

/** Wall-clock time a download will finish, like a departure time. */
export function doneAt(etaSecs: number | null | undefined): string {
  if (etaSecs == null) return '--:--';
  const d = new Date(Date.now() + etaSecs * 1000);
  const hm = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  return etaSecs > 86400 ? `+${Math.floor(etaSecs / 86400)}d ${hm}` : hm;
}

export function when(ms: number | null | undefined): string {
  if (!ms) return '—';
  const d = new Date(ms);
  const today = new Date();
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  if (d.toDateString() === today.toDateString()) return `Today ${time}`;
  const y = new Date(today);
  y.setDate(today.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return `Yesterday ${time}`;
  return d.toLocaleDateString([], { day: 'numeric', month: 'short', year: d.getFullYear() === today.getFullYear() ? undefined : 'numeric' });
}

export function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

export function percent(done: number, size: number | null): number | null {
  if (!size) return null;
  return Math.min(100, Math.floor((done / size) * 100));
}
