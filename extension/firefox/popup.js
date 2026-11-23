const $ = (id) => document.getElementById(id);

async function refresh() {
  $('state').className = 'state wait';
  $('state').textContent = 'Checking…';
  const s = await browser.runtime.sendMessage({ type: 'status' });
  if (s.connected) {
    $('state').className = 'state ok';
    $('state').textContent = `Connected${s.version ? ` · ${s.version}` : ''}`;
    $('why').hidden = true;
    $('retry').hidden = true;
  } else {
    $('state').className = 'state bad';
    $('state').textContent = 'Not connected';
    $('why').hidden = false;
    $('why').textContent =
      'DasIDM isn’t reachable, so Firefox keeps your downloads. Start DasIDM once (it registers itself with Firefox), then Retry.' +
      (s.error ? ` (${s.error})` : '');
    $('retry').hidden = false;
  }
  $('enabled').checked = !!s.settings.enabled;
  $('privateToo').checked = !!s.settings.privateToo;
  $('minMb').value = String(s.settings.minMb || 0);
  if (s.last) $('last').textContent = `Last: ${s.last.name} · ${new Date(s.last.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
}

for (const id of ['enabled', 'privateToo']) {
  $(id).addEventListener('change', (e) => browser.storage.local.set({ [id]: e.target.checked }));
}
$('minMb').addEventListener('change', (e) => browser.storage.local.set({ minMb: Math.max(0, Number(e.target.value) || 0) }));
$('retry').addEventListener('click', refresh);
refresh();
