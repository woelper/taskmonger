# Taskmonger

A task manager for people who think faster than they organize.

Taskmonger is built on one radical idea: you don't need boards, columns, databases, or a five-step onboarding flow to keep track of what you're doing. You just need a text buffer and some color.

Open it up, start typing. That's it. Everything lives in one single text area. Your thoughts, your todos, your half-baked plans, your meeting notes. It's a scratchpad that secretly has superpowers.

## What makes it tick

**One buffer to rule them all.** There are no separate "task cards" or "items" to create. You type freely, like you would in any text editor. The magic is that you can select any chunk of text and tag it.

**Tags with color.** Create tags like "urgent", "backlog", "idea", or whatever fits your brain. Each tag gets its own color, and tagged text lights up right there in the buffer. You can see at a glance what's what without leaving your flow.

**Flexible by nature.** Because tags are just colored ranges on free-form text, you can use Taskmonger however you want. Strict GTD system? Sure. Chaotic brain dump? Also great. Priority lists, project notes, daily journals, grocery lists... it all works because there's no rigid structure fighting you.

## Features

- **Inline tagging** - Select text, click a tag, done. Tagged ranges are highlighted directly in the buffer with the tag's color. Overlapping tags blend their colors together.
- **Drag and drop ordering** - Reorder your tagged ranges by dragging them around in the sidebar.
- **Markdown preview** - Toggle the markdown view to see your tagged sections rendered as formatted text in a side panel.
- **Color customization** - Pick exact colors for your tags with a full color picker, or hit "Rand col" for a fresh random one.
- **Dark and light mode** - Switch between themes with one click.
- **Auto-save** - Every change is saved automatically to disk. There's even a plaintext backup file, just in case.
- **Background or text coloring** - Choose whether tags show up as colored text or as highlighted backgrounds.
- **Lightweight and fast** - Built with egui/eframe, ships as a small native binary. No Electron, no web views, no runtime dependencies.

## Building

```bash
cargo build --release
```

The release profile is already tuned for minimum binary size (LTO, single codegen unit, symbol stripping, abort on panic).

## Releasing

Push a version tag to trigger a GitHub Actions workflow that builds for macOS, Windows, and Linux:

```bash
./bump_and_release.sh        # bumps patch version
./bump_and_release.sh minor  # bumps minor version
./bump_and_release.sh major  # bumps major version
```


## Tech

- Rust, [egui](https://github.com/emilk/egui) and [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) for the UI
- IBM Plex Sans and Plex Mono for typography
- [Phosphor icons](https://phosphoricons.com/) for the UI icons
- Cross-compiled for macOS (arm64), Windows (x86_64), and Linux (x86_64) from a single CI runner
