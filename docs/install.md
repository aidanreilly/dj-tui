# Installing dj-tui

## From the binary

Every tag publishes an x86_64 Linux tarball on the
[releases page](https://github.com/aidanreilly/dj-tui/releases):

```sh
tar xzf dj-tui-*-x86_64-linux.tar.gz
cd dj-tui-*-x86_64-linux
./dj-tui --demo
```

It needs glibc 2.35 or newer and `libasound2`, which any desktop that plays audio already has.
JACK is loaded at runtime rather than linked, so the same binary serves whether or not a JACK
server is installed. Every release carries a `.sha256` beside it:

```sh
sha256sum -c dj-tui-*-x86_64-linux.tar.gz.sha256
```

The tarball also carries `config.toml.example` and the licence, so keep the directory around
until you have copied the example config somewhere useful. See
[configuration](configuration.md).

## Putting it on your PATH

Running it from wherever you unpacked it gets old. Copy the binary into a directory your shell
already searches:

```sh
install -Dm755 dj-tui ~/.local/bin/dj-tui         # just for you, no sudo
sudo install -Dm755 dj-tui /usr/local/bin/dj-tui  # for everyone on the machine
```

`~/.local/bin` is on the PATH by default on Fedora and most systemd distributions. Check with
`echo $PATH` where you are unsure, and add it in your shell profile if it is missing.

From a checkout, `cargo install --path .` builds in release mode and puts the binary in
`~/.cargo/bin`, which the Rust installer already added to your PATH.

Either way, `dj-tui --demo` from any directory tells you it worked.

## From source

Fedora:

```sh
sudo dnf install cargo pipewire-jack-audio-connection-kit-devel
```

Debian and Ubuntu:

```sh
sudo apt install cargo libjack-jackd2-dev libasound2-dev pipewire-jack
```

Then `cargo build --release`, or run it straight from the checkout. Building needs Rust 1.90 or
newer. `cargo install --path .` does the release build and installs it in one step.

Tests:

```sh
cargo test --workspace
```

## Terminal requirements

Run dj-tui directly in a modern terminal emulator such as Ghostty, kitty, foot, WezTerm or a
recent Alacritty. Terminal multiplexers are not supported.

## Realtime audio

dj-tui asks for realtime scheduling on any thread it can. To grant it on Fedora and most
PipeWire desktops:

```sh
sudo usermod -aG pipewire $USER
```

Log out and back in, since group membership only applies to new sessions.

## Supported formats

WAV, FLAC, MP3, AAC/M4A, OGG Vorbis and AIFF.

## Running

```sh
dj-tui ~/Music/one.flac ~/Music/two.mp3   # deck A, deck B
dj-tui --demo                             # click tracks at 124 and 126 BPM
dj-tui --midi-learn                       # print what a controller sends
```

How you reach the binary depends on where it came from.

| Installed from | Command |
| --- | --- |
| A release tarball | `./dj-tui` inside the unpacked directory. |
| `cargo build --release` | `./target/release/dj-tui`. |
| `cargo install --path .`, or a copy into a PATH directory | `dj-tui` from anywhere. |
| A checkout, without building first | `cargo run --` followed by the arguments, as in `cargo run -- --demo`. |

The rest of these docs write the command as `dj-tui`.
