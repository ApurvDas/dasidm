// DasIDM for Firefox: files you download go to DasIDM instead of Firefox's own download list.
//
// Two ways in, both failing open (if DasIDM can't take it, Firefox downloads as usual):
//  1. A page or frame load whose response is a file (Content-Disposition: attachment, or a type
//     Firefox can't show) is handed over and cancelled before Firefox starts saving it.
//  2. Downloads the first step can't see (download links, script downloads) are caught when
//     Firefox creates them: paused, handed over, then cancelled and erased once DasIDM has them.
// DasIDM.exe is the native messaging host (dev.dasidm.host); each message starts it briefly.
/* global DasRules */
const HOST = 'dev.dasidm.host';
const R = DasRules;

const defaults = { enabled: true, minMb: 0, privateToo: false };
let settings = { ...defaults };
const ready = browser.storage.local.get(defaults).then((s) => (settings = { ...defaults, ...s }));
browser.storage.onChanged.addListener((changes) => {
  for (const [k, v] of Object.entries(changes)) settings[k] = v.newValue;
});

/** What the popup shows: is DasIDM there, and what was handed over last. */
const status = { connected: null, version: null, error: null, last: null };

/** Request headers worth passing on, by request id (a login's cookies, the page it came from). */
const sent = new Map();
/** Links Alt+clicked a moment ago: Firefox keeps those. */
const bypass = new Map();
/** Links DasIDM just took: Firefox must not also download them. */
const taken = new Map();

const fresh = (map, url, ms) => {
  const t = map.get(url);
  if (t && Date.now() - t < ms) return true;
  map.delete(url);
  return false;
};

async function native(msg) {
  try {
    const reply = await browser.runtime.sendNativeMessage(HOST, msg);
    status.connected = true;
    status.error = null;
    if (reply && reply.version) status.version = reply.version;
    return reply;
  } catch (e) {
    status.connected = false;
    status.error = String((e && e.message) || e);
    throw e;
  }
}

async function cookieHeader(url, incognito) {
  try {
    // The cookies Firefox itself would send to this address (unpartitioned, right window kind).
    const all = await browser.cookies.getAll({ url, firstPartyDomain: null, ...(incognito ? { storeId: 'firefox-private' } : {}) });
    return all.map((c) => `${c.name}=${c.value}`).join('; ') || undefined;
  } catch {
    return undefined;
  }
}

/** Ask DasIDM to take a download. Resolves true only if it did. */
async function handOff(d) {
  const msg = {
    type: 'download',
    url: d.url,
    fileName: d.fileName,
    size: d.size > 0 ? d.size : undefined,
    referrer: d.referrer,
    cookie: d.cookie,
    userAgent: navigator.userAgent,
  };
  try {
    const reply = await native(msg);
    const ok = !!(reply && reply.accepted);
    if (ok) {
      taken.set(d.url, Date.now());
      status.last = { name: d.fileName || R.filenameFromUrl(d.url) || d.url, at: Date.now() };
    }
    return ok;
  } catch {
    return false; // DasIDM isn't reachable: Firefox keeps the download
  }
}

// 1. Remember what each page or frame load sent.
browser.webRequest.onSendHeaders.addListener(
  (d) => {
    sent.set(d.requestId, {
      referrer: R.header(d.requestHeaders, 'referer'),
      cookie: R.header(d.requestHeaders, 'cookie'),
    });
    if (sent.size > 500) sent.delete(sent.keys().next().value);
  },
  { urls: ['<all_urls>'], types: ['main_frame', 'sub_frame'] },
  ['requestHeaders'],
);

// 1. A response that is a file: hand it over and stop Firefox from saving it too.
browser.webRequest.onHeadersReceived.addListener(
  async (d) => {
    await ready;
    const req = sent.get(d.requestId) || {};
    sent.delete(d.requestId);
    const disposition = R.header(d.responseHeaders, 'content-disposition');
    const size = Number(R.header(d.responseHeaders, 'content-length')) || 0;
    const verdict = R.shouldGrab(
      {
        url: d.url,
        method: d.method,
        status: d.statusCode,
        contentType: R.header(d.responseHeaders, 'content-type'),
        disposition,
        size,
        incognito: d.incognito,
        bypassed: fresh(bypass, d.url, 15000),
      },
      settings,
    );
    if (!verdict.grab) return {};
    const ok = await handOff({
      url: d.url,
      fileName: R.filenameFromDisposition(disposition) || R.filenameFromUrl(d.url),
      size,
      referrer: req.referrer || d.originUrl,
      cookie: req.cookie || (await cookieHeader(d.url, d.incognito)),
    });
    if (!ok) return {};
    // A tab opened only for this download (target=_blank) would stay empty: close it.
    if (d.type === 'main_frame' && d.tabId >= 0) {
      setTimeout(async () => {
        try {
          const tab = await browser.tabs.get(d.tabId);
          if (tab.openerTabId !== undefined && (!tab.url || tab.url === 'about:blank')) await browser.tabs.remove(d.tabId);
        } catch {
          /* gone already */
        }
      }, 300);
    }
    return { cancel: true };
  },
  { urls: ['<all_urls>'], types: ['main_frame', 'sub_frame'] },
  ['blocking', 'responseHeaders'],
);

// 2. Everything else Firefox starts downloading.
browser.downloads.onCreated.addListener(async (item) => {
  await ready;
  const url = item.finalUrl || item.url;
  if (fresh(taken, item.url, 30000) || fresh(taken, url, 30000)) {
    // Already with DasIDM (step 1 raced Firefox): drop Firefox's copy.
    await drop(item.id);
    return;
  }
  if (item.byExtensionId === browser.runtime.id) return;
  const verdict = R.shouldGrab(
    {
      url,
      status: 200,
      contentType: item.mime || 'application/octet-stream',
      disposition: 'attachment',
      size: item.totalBytes,
      incognito: item.incognito,
      bypassed: fresh(bypass, item.url, 15000) || fresh(bypass, url, 15000),
    },
    settings,
  );
  if (!verdict.grab || item.state !== 'in_progress') return;
  try {
    await browser.downloads.pause(item.id);
  } catch {
    /* some downloads can't pause; it is cancelled below if DasIDM takes it */
  }
  const ok = await handOff({
    url,
    fileName: (item.filename && item.filename.split(/[\\/]/).pop()) || R.filenameFromUrl(url),
    size: item.totalBytes,
    referrer: item.referrer,
    cookie: await cookieHeader(url, item.incognito),
  });
  if (ok) await drop(item.id);
  else browser.downloads.resume(item.id).catch(() => {});
});

async function drop(id) {
  await browser.downloads.cancel(id).catch(() => {});
  await browser.downloads.erase({ id }).catch(() => {});
}

// Right-click a link: Download with DasIDM (even for a type Firefox would show).
browser.runtime.onInstalled.addListener(() => {
  browser.contextMenus.create({ id: 'dasidm-link', title: 'Download with DasIDM', contexts: ['link'] });
});
browser.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId !== 'dasidm-link' || !/^https?:/i.test(info.linkUrl || '')) return;
  const incognito = !!(tab && tab.incognito);
  await handOff({
    url: info.linkUrl,
    fileName: R.filenameFromUrl(info.linkUrl),
    referrer: info.pageUrl,
    cookie: await cookieHeader(info.linkUrl, incognito),
  });
});

// The content script (Alt+click) and the popup.
browser.runtime.onMessage.addListener((msg) => {
  if (msg.type === 'bypass' && typeof msg.url === 'string') {
    bypass.set(msg.url, Date.now());
    return undefined;
  }
  if (msg.type === 'status') {
    return native({ type: 'ping' })
      .catch(() => null)
      .then(() => ({ ...status, settings }));
  }
  return undefined;
});
