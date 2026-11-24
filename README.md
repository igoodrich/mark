# Mark

A small Markdown viewer for [Omarchy](https://omarchy.org). One window, no editing: it
shows a Markdown file the way GitHub would, in the colors of the current Omarchy theme,
and re-renders whenever the file changes on disk — keep it open beside your editor.

```sh
mark notes.md
```

It also opens Markdown files double-clicked in the file manager, and shows up in the
app launcher (Super + Space).

## Features

- GitHub-flavored Markdown: tables, task lists, footnotes, strikethrough, alerts
  (`> [!NOTE]`), and heading anchors for `#section` links.
- Live reload that keeps your scroll position.
- Follows the Omarchy theme, live, including Hyprland's window rounding.
- Relative images (`![](diagram.png)`) load from beside the file.
- Web links open in your browser; links to other local files are ignored.
- Scripts and inline HTML event handlers in a Markdown file don't run.

## Keys

| Key | Action |
|-----|--------|
| `j` / `k`, arrows | Scroll down / up |
| `d` / `u`, `Ctrl+D` / `Ctrl+U` | Half page down / up |
| `Space` / `b` | Page down / up |
| `g` / `G` | Top / bottom |
| `+` / `-` / `0` | Zoom in / out / reset |
| `r` | Reload |
| `?` | Show shortcuts |
| `q`, `Ctrl+Q`, `Ctrl+W` | Quit |

## Build and install

Needs Rust and `webkit2gtk-4.1` (already part of Omarchy).

```sh
cargo run -- README.md   # try it
cargo test
bin/install              # binary, launcher entry, icon; makes Mark the default for .md
bin/install --no-default # same, without changing the default .md app
bin/uninstall
```

Under the hood: `pulldown-cmark` renders the Markdown, and `tao` + `wry` show it in a
WebKitGTK view, served over a private `mark://` scheme.

## License

MIT
