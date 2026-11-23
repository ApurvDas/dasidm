# Firefox Add-ons store listing: DasIDM – Download Manager

Paste-ready text for the Developer Hub (https://addons.mozilla.org/developers/addons → DasIDM →
Edit Product Page) if the store asks for listing details that `sign.ps1 -Listed` can't send.
The same text is in `metadata.json`, which `sign.ps1 -Listed` sends.

| Field | Value |
|---|---|
| Name | DasIDM – Download Manager (from the add-on's manifest) |
| Add-on URL (slug) | `dasidm` |
| Category | Download Management |
| Licence | Apache License 2.0 |
| Homepage | https://github.com/ApurvDas/dasidm |
| Support site | https://github.com/ApurvDas/dasidm/issues |
| Support email | leave empty (support is through GitHub issues) |
| Privacy policy | paste [PRIVACY.md](../../PRIVACY.md), or link https://github.com/ApurvDas/dasidm/blob/main/PRIVACY.md |
| Requires payment, non-free services or software | No (DasIDM is free and open source) |
| Experimental | No |
| Tags | download manager, downloads, download accelerator, torrent, resume |

## Summary (max 250 characters)

Sends the files you download in Firefox to DasIDM, a free and open-source download manager for
Windows: many connections per file, resume, torrents and video. Needs the DasIDM app.

## Description

See `description` in `metadata.json` (it uses AMO's simple HTML: `<b>` and line breaks).

## Screenshots (in `screenshots/`, 1280 × 800)

1. `1-handoff.png`: "A file downloaded in Firefox opens in DasIDM, filled in. Press Enter."
2. `2-pieces.png`: "Every connection fills its own piece of the file."
3. `3-popup.png`: "The toolbar button: connected, and three simple switches."

## Notes to reviewer

See `version.approval_notes` in `metadata.json`: how to install DasIDM, how to test, and why each
permission is needed.
