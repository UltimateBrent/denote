# DENote

A lightweight macOS daemon that watches Bear notes and syncs them as markdown files to a git repository. Designed for users who want their Bear notes accessible on remote machines — e.g., for an AI agent running on a separate server.

## Architecture

```
Bear App → SQLite DB → denote (watch + export + git push) → remote repo
```

denote runs as a launchd service on macOS. It monitors Bear's local SQLite database for changes, exports notes as individual markdown files with YAML frontmatter, and commits/pushes to a configurable git remote. It never writes to Bear's database.

## Quick Start

```sh
# Build
cargo build --release

# Initialize a new repo and config
denote init --repo ~/denote-notes --remote git@github.com:user/bear-notes.git

# One-shot sync
denote sync

# Continuous watch mode (for launchd)
denote watch

# Check status
denote status
```

## Configuration

All behavior is driven by `~/.config/denote/config.toml`. Every field has a sensible default so a minimal config only needs the repo path. See `config.example.toml` for all options.

```toml
repo_path = "~/denote-notes"
```

### Environment Variable Overrides

Every config key can be overridden via environment variable with the `DENOTE_` prefix. Nested keys use `__` as separator.

```sh
DENOTE_REPO_PATH=~/my-notes denote sync
DENOTE_EXPORT__FRONTMATTER=false denote sync
```

## CLI

```
denote <command> [options]

Commands:
  init      Initialize a new denote repo and write default config
  sync      One-shot: export changed notes, commit, and push
  watch     Continuous: monitor Bear's DB and sync on changes
  status    Show last sync time, note count, and repo state

Global flags:
  -c, --config <path>    Path to config file [default: ~/.config/denote/config.toml]
  -v, --verbose          Enable debug logging
  -q, --quiet            Suppress all output except errors
```

## Running as a launchd Service

Copy the plist and load it:

```sh
cp dev.launchd.plist ~/Library/LaunchAgents/com.denote.agent.plist
launchctl load ~/Library/LaunchAgents/com.denote.agent.plist
```

## Exported Note Format

Each note becomes a markdown file with YAML frontmatter:

```markdown
---
id: "A1B2C3D4-E5F6-..."
title: "Project Notes"
tags: ["work", "denote"]
created: "2026-03-15T10:30:00Z"
modified: "2026-03-27T14:22:00Z"
pinned: true
---

The actual note content from Bear follows here...
```

## Safety

denote never writes to Bear's database. It opens the SQLite file with `SQLITE_OPEN_READ_ONLY`, enforces `PRAGMA query_only = ON`, and uses short-lived connections to minimize lock contention.

## License

MIT
