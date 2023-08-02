# The Pirate Bay CLI

<p>
  <a href="https://crates.io/crates/piratebay" target="_blank">
    <img src="https://img.shields.io/crates/v/piratebay.svg" />
  </a>
  <a href="https://crates.io/crates/piratebay" target="_blank">
    <img src="https://img.shields.io/crates/dr/piratebay" />
  </a>
  <a href="https://docs.rs/piratebay" target="_blank">
    <img src="https://docs.rs/piratebay/badge.svg" />
  </a>
  <a href="LICENSE" target="_blank">
    <img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-blue.svg" />
  </a>
  <a href="https://github.com/tsirysndr/piratebay/actions/workflows/release.yml" target="_blank">
    <img alt="License: MIT" src="https://github.com/tsirysndr/piratebay/actions/workflows/release.yml/badge.svg" />
  </a>
  <a href="https://github.com/tsirysndr/piratebay/actions/workflows/rust-clippy.yml" target="_blank">
    <img alt="release" src="https://github.com/tsirysndr/piratebay/actions/workflows/rust-clippy.yml/badge.svg?branch=master" />
  </a>
</p>

<p>
<a href="https://www.buymeacoffee.com/tsiry">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-red.png" alt="Buy Me A Coffee" height="40" />
</a>
</p>

PirateBay CLI helps you search torrents right from the terminal.

# Installation

Simply run:

```bash
$ cargo install piratebay
```

### macOS

```bash
$ brew install tsirysndr/tap/piratebay
```

### Nix

```bash
nix profile install --experimental-features "nix-command flakes" github:tsirysndr/piratebay
```

Or download the latest release for your platform from [here](https://github.com/tsirysndr/piratebay/releases).

<img src="https://raw.githubusercontent.com/tsirysndr/piratebay/master/preview.png" />
