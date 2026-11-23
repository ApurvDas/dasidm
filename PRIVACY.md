# Privacy

DasIDM is a download manager that runs on your own Windows PC. It has no account, no
telemetry, no analytics and no ads, and it never sends anything about you to its author or
to anyone else.

## The Firefox add-on ("DasIDM – Download Manager")

When you download a file in Firefox, the add-on passes these details to the DasIDM app on
**the same computer**, so DasIDM can download the same file:

- the file's web address, its name and its size;
- the cookie, Referer and User-Agent that Firefox would send to that same site, so files
  behind your login download correctly.

They travel through Windows' native messaging and a local named pipe that only programs of
your own Windows account can reach. Nothing is sent over the network by the add-on, and
nothing goes to any server. The add-on keeps only its own settings (capture on or off,
private windows, minimum size) in Firefox's local storage. It does not read your history,
bookmarks or pages.

Private windows are left alone unless you turn on "Also in private windows" in the
add-on's popup. Alt+click a link to keep any single download in Firefox.

## The DasIDM app

DasIDM connects only to:

- the sites you download from, sending the details above when they came from Firefox;
- GitHub, to download yt-dlp, FFmpeg and Deno, only after you agree, and to check weekly
  for yt-dlp updates;
- BitTorrent trackers and peers, while a torrent is downloading or seeding (this shares
  your IP address with the swarm, as every BitTorrent program does).

Your downloads list and settings stay on your PC (`%APPDATA%\dev.dasidm.app`).

## Questions

Open an issue at https://github.com/ApurvDas/dasidm/issues.
