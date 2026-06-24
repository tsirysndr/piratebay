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

PirateBay CLI helps you search **and download** torrents right from the terminal — pure Rust, async (tokio), no native C dependencies.

<a href="https://pocketenv.io/new?repo=tangled:tsiry-sandratraina.com/piratebay" target="_blank"><img src="https://pocketenv.io/open-in-pocketenv.svg" alt="Open in Pocketenv" /></a>

  <img src="https://vhs.charm.sh/vhs-6sDgMICizW9RTfFPPnaoed.gif" alt="Made with VHS">
  <a href="https://vhs.charm.sh">
    <img src="https://stuff.charm.sh/vhs/badge.svg">
  </a>

# Features

- Search The Pirate Bay from the terminal (table or `--json` output)
- Browse by category (audio, video, applications, games, other)
- Pure-Rust torrent downloader (powered by [`librqbit`](https://crates.io/crates/librqbit), rustls only — **no OpenSSL, no system C libs**)
- Live progress bar with throughput (bytes/sec) and ETA
- `--stream` mode: pipe the largest file to stdout as it downloads — works with mpv, VLC, ffplay
- Synthwave-themed CLI help

# Usage

## Search

```bash
piratebay search "debian 12"
piratebay search "ubuntu" --json
```

## Get torrent info (and magnet link)

```bash
piratebay info 72614907
```

## Browse a category

```bash
piratebay category --video
piratebay category --audio
```

## Download

The `download` subcommand accepts three input forms:

```bash
# 1. A piratebay id from search results
piratebay download 72614907 --output ~/Downloads

# 2. A magnet: URL
piratebay download 'magnet:?xt=urn:btih:...&dn=...'

# 3. A local .torrent file
piratebay download ./my-torrent.torrent --output ./dl
```

Downloads run sequentially by default (librqbit's piece picker), so partial files
are playable as bytes arrive.

### Streaming to a media player

`--stream` opens the largest file in the torrent and pipes its bytes to stdout
while the download runs in the background — the progress bar is sent to stderr,
so you can pipe straight into a player:

```bash
piratebay download <id-or-magnet> --stream | mpv -
piratebay download <id-or-magnet> --stream | vlc -
piratebay download <id-or-magnet> --stream | ffplay -i pipe:0
```

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
