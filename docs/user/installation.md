# Installation

## Prerequisites

Before installing jj-spr, you need to have Jujutsu (jj) installed on your system. See the [Jujutsu installation guide](https://github.com/martinvonz/jj#installation) for instructions.

## Binary Installation

*Note: Binary distribution methods are still being set up. For now, please install from source.*

### Using Homebrew (coming soon)

```shell
# Not yet available
brew install jj-spr
```

### Using Cargo (coming soon)

```shell
# Not yet available
cargo install jj-spr
```

## Install from Source

jj-spr is written in Rust. You need a Rust toolchain to build from source. See [rustup.rs](https://rustup.rs) for information on how to install Rust if you have not got a Rust toolchain on your system already.

With Rust all set up:

1. Clone this repository
2. Run `cargo build --release`
3. The jj-spr binary will be in the `target/release` directory
4. Add the binary to your PATH or copy it to a directory in your PATH

```shell
# Example installation from source
git clone https://github.com/yourusername/jj-spr.git
cd jj-spr
cargo build --release
sudo cp target/release/spr /usr/local/bin/jj-spr
```

## Verify Installation

After installation, verify that jj-spr is available:

```shell
jj-spr --version
```