// node --test extension/firefox
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const R = createRequire(import.meta.url)('./rules.js');
const on = { enabled: true, minMb: 0, privateToo: false };
const ok = (r, s = on) => R.shouldGrab({ url: 'https://example.org/f', method: 'GET', status: 200, ...r }, s);

test('files are grabbed, pages are not', () => {
  assert.equal(ok({ contentType: 'application/x-msdownload' }).grab, true);
  assert.equal(ok({ contentType: 'application/zip' }).grab, true);
  assert.equal(ok({ contentType: 'application/octet-stream' }).grab, true);
  assert.equal(ok({ contentType: 'text/html; charset=utf-8' }).grab, false);
  assert.equal(ok({ contentType: 'application/pdf' }).grab, false);
  assert.equal(ok({ contentType: 'image/png' }).grab, false);
  assert.equal(ok({ contentType: 'video/mp4' }).grab, false);
});

test('anything sent as an attachment is grabbed, even a type Firefox could show', () => {
  assert.equal(ok({ contentType: 'application/pdf', disposition: 'attachment; filename="a.pdf"' }).grab, true);
  assert.equal(ok({ contentType: 'text/html', disposition: 'Attachment' }).grab, true);
  assert.equal(ok({ contentType: 'application/pdf', disposition: 'inline; filename="a.pdf"' }).grab, false);
});

test('switches and limits', () => {
  assert.equal(ok({ contentType: 'application/zip' }, { ...on, enabled: false }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', incognito: true }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', incognito: true }, { ...on, privateToo: true }).grab, true);
  assert.equal(ok({ contentType: 'application/zip', bypassed: true }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', size: 500_000 }, { ...on, minMb: 1 }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', size: 5_000_000 }, { ...on, minMb: 1 }).grab, true);
  assert.equal(ok({ contentType: 'application/zip', size: 0 }, { ...on, minMb: 1 }).grab, true, 'unknown size: grab');
  assert.equal(ok({ contentType: 'application/zip', method: 'POST' }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', status: 404 }).grab, false);
  assert.equal(ok({ contentType: 'application/zip', url: 'blob:https://x/1' }).grab, false);
});

test('file names', () => {
  assert.equal(R.filenameFromDisposition('attachment; filename="report q3.pdf"'), 'report q3.pdf');
  assert.equal(R.filenameFromDisposition('attachment; filename=setup.exe'), 'setup.exe');
  assert.equal(R.filenameFromDisposition(`attachment; filename="x.bin"; filename*=UTF-8''%E0%A4%B9%E0%A4%BF.zip`), 'हि.zip');
  assert.equal(R.filenameFromDisposition('attachment; filename="..\\..\\evil.exe"'), 'evil.exe');
  assert.equal(R.filenameFromDisposition('attachment; filename="a:b?.txt"'), 'a_b_.txt');
  assert.equal(R.filenameFromDisposition('attachment'), undefined);
  assert.equal(R.filenameFromUrl('https://cdn.example.org/files/Big%20File.iso?x=1'), 'Big File.iso');
  assert.equal(R.filenameFromUrl('https://example.org/'), undefined);
});
