# Third-party code

## librqbit and librqbit-tracker-comms 9.1.0

- Source: the published crates `librqbit` 9.1.0 and `librqbit-tracker-comms` 9.1.0
  (https://github.com/ikatson/rqbit, tag v9.1.0), copied as released on crates.io.
- Copyright 2021 Igor Katson. Licensed under the Apache License, Version 2.0
  (see `LICENSE-rqbit-Apache-2.0`).
- Used through `[patch.crates-io]` in the workspace `Cargo.toml`; every other rqbit crate
  comes unchanged from crates.io.

DasIDM changed these files (each change is a commit whose message starts with `rqbit-patch:`):

- TLS through the system's native-tls (as the rest of DasIDM) instead of reqwest's
  rustls default.
- Trackers can be added, removed, replaced and reannounced while a torrent runs, and each
  tracker reports its status (working, not working with the reason, seeders, leechers,
  peers received, last and next announce).
