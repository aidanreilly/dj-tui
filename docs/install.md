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
newer.

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
cargo run -- ~/Music/one.flac ~/Music/two.mp3   # deck A, deck B
cargo run -- --demo                             # click tracks at 124 and 126 BPM
cargo run -- --midi-learn                       # print what a controller sends
```
