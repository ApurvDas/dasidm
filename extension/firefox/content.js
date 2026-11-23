// Alt+click on a link leaves that one download to Firefox.
document.addEventListener(
  'click',
  (e) => {
    if (!e.altKey) return;
    const a = e.target instanceof Element ? e.target.closest('a[href]') : null;
    if (a) browser.runtime.sendMessage({ type: 'bypass', url: a.href }).catch(() => {});
  },
  true,
);
