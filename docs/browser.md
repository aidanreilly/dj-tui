# The browser

Point dj-tui at your music:

```toml
[library]
folders = ["~/Music", "/mnt/crates"]
```

`b` opens the file browser full screen, `/` opens it beside the decks.

## Narrowing the list

Three keys narrow what you see.

| Key | Keeps |
| --- | --- |
| `Alt+b` | Tracks inside a tempo window around whatever deck is playing. Half and double time count, so a 64 BPM track still shows against 128. Press again to widen it, again to switch it off. |
| `Alt+k` | Keys that would mix with the playing deck's. |
| `Alt+g` | One genre, walking whatever your library holds. |

Each one you switch on draws a chip in the panel title saying what it is measuring against.

Typing narrows it too. `bpm:124`, `bpm:124-128`, `key:9a` and `genre:house` each filter by one
field, and anything else searches artist and title. Mix them freely, for example
`bpm:124-128 bicep`. `Ctrl+u` clears the filter.

`Alt+s` and `Alt+S` change the sort column and its direction.

## Analysis

`Alt+a` reads tags, asks Discogs about whatever the tags left blank, and analyses what has no
tempo yet. Press it again to stop.

## Genre

Genre comes from the file's own tags. Where a file carries none, dj-tui can ask Discogs
instead, matching on artist and title. This is off until you switch it on, and nothing leaves
your machine unless you press `Alt+a` or `Alt+d`.

```toml
[library]
discogs = true
```

```sh
export DJ_TUI_DISCOGS_TOKEN=...   # discogs.com, Settings then Developers
```

The token is read from the environment rather than the config file, so a config you copy
around carries no credential. Turning this on sends the artist and title of those files to
discogs.com. Answers are kept in `$XDG_STATE_HOME/dj-tui/discogs.json`, so nothing is asked
twice.
