// What DasIDM takes from Firefox, as plain functions (no browser APIs), so they can be tested
// with `node --test`. Loaded as a background script before background.js.
(function (root) {
  /** Types Firefox shows in the tab itself: those stay with the browser unless sent as a file. */
  const VIEWABLE = [
    /^text\/(html|plain|xml|css|javascript|csv|markdown|vtt|calendar)$/i,
    /^application\/(xhtml\+xml|xml|json|ld\+json|javascript|ecmascript|rss\+xml|atom\+xml|pdf|manifest\+json)$/i,
    /^image\//i,
    /^(audio|video)\//i,
    /^multipart\/x-mixed-replace$/i,
  ];

  function header(headers, name) {
    const n = name.toLowerCase();
    const h = (headers || []).find((x) => x.name.toLowerCase() === n);
    return h ? h.value : undefined;
  }

  function isAttachment(disposition) {
    return /^\s*attachment\b/i.test(disposition || '');
  }

  function mimeOf(contentType) {
    return (contentType || '').split(';')[0].trim().toLowerCase();
  }

  function viewable(mime) {
    return !!mime && VIEWABLE.some((r) => r.test(mime));
  }

  /** The file name a server gives, RFC 6266 style: filename*=UTF-8''… wins over filename=… */
  function filenameFromDisposition(disposition) {
    if (!disposition) return undefined;
    const star = /filename\*\s*=\s*([^']*)'[^']*'([^;]+)/i.exec(disposition);
    if (star) {
      try {
        return clean(decodeURIComponent(star[2].trim().replace(/^"|"$/g, '')));
      } catch {
        /* fall through */
      }
    }
    const plain = /filename\s*=\s*("((?:\\.|[^"\\])*)"|[^;]+)/i.exec(disposition);
    if (plain) return clean((plain[2] !== undefined ? plain[2].replace(/\\(["\\])/g, '$1') : plain[1]).trim());
    return undefined;
  }

  function filenameFromUrl(url) {
    try {
      const last = new URL(url).pathname.split('/').filter(Boolean).pop();
      return last ? clean(decodeURIComponent(last)) : undefined;
    } catch {
      return undefined;
    }
  }

  /** No folders, no characters Windows refuses. */
  function clean(name) {
    const base = name.split(/[\\/]/).pop() || '';
    const safe = base.replace(/[<>:"|?*\u0000-\u001f]/g, '_').trim();
    return safe && safe !== '.' && safe !== '..' ? safe : undefined;
  }

  /**
   * Whether a response Firefox is about to save (or show) should go to DasIDM instead.
   * r: {url, method, status, contentType, disposition, size, incognito, bypassed}
   * s: {enabled, minMb, privateToo}
   */
  function shouldGrab(r, s) {
    if (!s.enabled) return { grab: false, reason: 'capture is off' };
    if (!/^https?:/i.test(r.url || '')) return { grab: false, reason: 'not an http(s) link' };
    if (r.method && r.method !== 'GET') return { grab: false, reason: 'not a GET request' };
    if (r.status && (r.status < 200 || r.status >= 300)) return { grab: false, reason: 'not a success' };
    if (r.incognito && !s.privateToo) return { grab: false, reason: 'private window' };
    if (r.bypassed) return { grab: false, reason: 'left to Firefox (Alt+click)' };
    const mime = mimeOf(r.contentType);
    const attach = isAttachment(r.disposition);
    if (!attach && viewable(mime)) return { grab: false, reason: 'Firefox shows it' };
    if (!attach && !mime) return { grab: false, reason: 'no type given' };
    const min = (s.minMb || 0) * 1024 * 1024;
    if (min > 0 && r.size > 0 && r.size < min) return { grab: false, reason: 'smaller than the minimum' };
    return { grab: true, reason: attach ? 'sent as a file' : `a ${mime} file` };
  }

  const api = { header, isAttachment, mimeOf, viewable, filenameFromDisposition, filenameFromUrl, shouldGrab };
  root.DasRules = api;
  if (typeof module !== 'undefined') module.exports = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
