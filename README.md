# Steam CLI (Rust)

A local-first Steam CLI designed for LLM/tool workflows.

This project is intentionally not a "smart recommender". Instead, it exposes a set of stable primitives so an LLM (or any automation) can:

- discover the right tags / genres / categories
- compose Steam Store searches
- fetch structured app details
- optionally enrich with a user's library + playtime (Steam Web API)

## Why this exists

Steam has great data, but it's scattered across:

- Steam Store endpoints (search + app details)
- Steam Web API (user/library)
- community-driven dictionaries (genres/categories)

This CLI normalizes that into a small, predictable interface:

- human-readable output by default
- machine output via `--json` / `--format json`
- fast local search for tags/genres/categories via SQLite + FTS5

## Features

- LLM-friendly primitives (no "magic")
- Stable JSON envelope (`ok`, `data`, `pagination`, `meta`, `error`)
- Fast local lookup for:
  - Steam Tags (id <-> name)
  - Genres (id <-> name)
  - Categories/Features (id <-> name; e.g. Local Co-op)
- Steam Store search by tag IDs
  - optional facet extraction (related tags)
- Steam app details via `appdetails` with caching (TTL)
- User owned games + playtime (optional; requires Steam Web API key)

## Install / Build

```bash
cargo build
```

## Quick start

List and search tags:

```bash
steam-cli tags list --limit 20
steam-cli tags find "pixel graphics" --limit 10
```

Search the Steam Store using tag IDs:

```bash
steam-cli search --tags 3964,4182 --limit 25
steam-cli search --tags 3964 --with-facets --limit 25
```

Fetch app details (cached):

```bash
steam-cli app 413150
steam-cli app 413150 --ttl-sec 86400
```

(Optional) Load user library + playtime:

```bash
export STEAM_API_KEY="your_key_here"
steam-cli user owned --vanity gaben
steam-cli user owned --steamid 76561197960287930 --limit 50
```

## Commands

### Tags

```bash
steam-cli tags list [--limit N] [--offset M]
steam-cli tags find <query> [--limit N] [--offset M]
```

### Genres

```bash
steam-cli genres list [--limit N] [--offset M]
steam-cli genres find <query> [--limit N] [--offset M]
```

### Categories (Store "features")

```bash
steam-cli categories list [--limit N] [--offset M]
steam-cli categories find <query> [--limit N] [--offset M]
```

### Search (Steam Store)

```bash
steam-cli search --tags <id1,id2,...> [--term text] [--limit N] [--offset M] [--with-facets]
```

Notes:

- Search uses Steam Store endpoints and currently parses HTML results.
- `--with-facets` extracts related tag IDs from the response (useful for iterative discovery).

### App details

```bash
steam-cli app <appid> [--ttl-sec 86400]
```

### User library (optional)

```bash
steam-cli user owned --steamid <id> [--limit N] [--offset M]
steam-cli user owned --vanity <name> [--limit N] [--offset M]
```

## JSON mode

```bash
steam-cli tags list --json
steam-cli search --tags 4182,9 --format json
```

All JSON output uses a stable envelope:

```json
{
  "ok": true,
  "data": {},
  "pagination": {
    "limit": 25,
    "offset": 0,
    "returned": 25,
    "has_more": true,
    "total": null
  },
  "meta": {
    "version": "1.0.0",
    "source": "steam_store",
    "cached": false
  },
  "error": null
}
```

`meta.source` values:

- `local_db`
- `steam_store`
- `steam_webapi`
- `internal`

## Data model (local)

The CLI uses a small SQLite database containing tags/genres/categories:

- Runtime location: `~/.steam-cli-rs/steam.db`
- Search engine: SQLite FTS5

The seed DB is embedded from `assets/steam.db` and copied to `~/.steam-cli-rs/steam.db` if it does not exist.

## Steam Web API key

`steam-cli user owned` requires:

- `STEAM_API_KEY` environment variable
- User profile "Game details" visibility set to Public:
  - https://steamcommunity.com/my/edit/settings

## Development

Regenerate the seeded DB from JSON dictionaries:

```bash
cargo run --bin dev_seed_db
```

## Non-goals

- No built-in recommendation engine
- No server component
- No attempt to fully mirror SteamDB datasets
