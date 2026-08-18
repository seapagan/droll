# TODO

## Deferred review follow-ups

- [ ] Harden the Advisory Quality sticky-comment identity and deduplication.
  Match the marker exactly or structurally, and handle duplicate historical bot
  comments deterministically so stale matching comments cannot accumulate.
- [ ] Normalize `taiki-e/install-action` release versions and full-SHA pins
  across workflows when those action dependencies are next updated. The
  currently verified differing versions do not require immediate churn.
- [ ] Add a maintainable consistency check for current-Rust and MSRV
  declarations across the toolchain, workspace metadata, cargo-make tasks,
  workflows, and README without introducing unnecessary configuration.
- [ ] Deduplicate the Linux native dependency package list used by native CI,
  MSRV, Advisory Quality, verification, and documentation while preserving
  transparent installation and installed-version evidence.
