# The Pirate Bay CLI

<p>
  <a href="https://flakehub.com/flake/tsirysndr/piratebay" target="_blank">
    <img src="https://img.shields.io/endpoint?url=https://flakehub.com/f/tsirysndr/piratebay/badge" />
  </a>
   <a href="https://flakestry.dev/flake/github/tsirysndr/piratebay" target="_blank">
    <img src="https://flakestry.dev/api/badge/flake/github/tsirysndr/piratebay" />
  </a>
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

PirateBay CLI helps you search torrents right from the terminal.

  <img src="https://vhs.charm.sh/vhs-6sDgMICizW9RTfFPPnaoed.gif" alt="Made with VHS">
  <a href="https://vhs.charm.sh">
    <img src="https://stuff.charm.sh/vhs/badge.svg">
  </a>

# Installation

Simply run:

```bash
$ cargo install piratebay
```

### macOS

```bash
brew install tsirysndr/tap/piratebay
```

## Arch Linux

```bash
yay -Syu piratebay
```

### Nix

```bash
cachix use tsirysndr
nix profile install --experimental-features "nix-command flakes" github:tsirysndr/piratebay
```

### NetBSD
```bash
pkgin install piratebay
```

Or download the latest release for your platform from [here](https://github.com/tsirysndr/piratebay/releases).

