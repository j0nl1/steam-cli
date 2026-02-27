---
name: steam-cli
description: Use this skill when you need Steam discovery primitives for tags, genres, categories, store search, app details, and optional user-owned library signals.
---

# steam-cli

## What this skill does

Provides a local-first Steam CLI for LLM/tool workflows.
It exposes stable primitives (no recommendation engine) so an LLM can:

- discover Steam tags / genres / categories
- compose Steam Store searches
- fetch structured app details
- optionally enrich with user library + playtime (Steam Web API)

## Requirements

- Binary available on PATH: `steam-cli`
- Local DB directory: `~/.steam-cli-rs/`
- Optional for user endpoints: `STEAM_API_KEY`

## Output formats

- Default: human-readable
- Machine: `--json` or `--format json`

## Stable JSON envelope

All JSON responses use this shape:

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
    "source": "local_db",
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

## Core commands (LLM-facing primitives)

### 1) Tags

```bash
steam-cli tags list --limit 50 --offset 0 --json
steam-cli tags find "pixel graphics" --limit 10 --json
steam-cli tags find "couch coop" --limit 10 --json
```

Use `tags find` to map natural language to tag IDs.

### 2) Genres

```bash
steam-cli genres list --limit 50 --json
steam-cli genres find "roguelite" --limit 10 --json
```

### 3) Categories (Store features)

Use for features like Local Co-op, Online Co-op, Single-player.

```bash
steam-cli categories list --limit 50 --json
steam-cli categories find "local co-op" --limit 10 --json
```

### 4) Search (Steam Store)

```bash
steam-cli search --tags 3964,4182 --limit 25 --offset 0 --json
steam-cli search --tags 3964 --with-facets --limit 25 --json
steam-cli search --tags 3964 --term "co-op" --limit 25 --json
```

Notes:

- Search uses Steam Store endpoints and parses HTML results.
- Prefer `--with-facets` to discover adjacent tag IDs iteratively.
- For feature constraints (for example Local Co-op), validate with `steam-cli app <appid>` and filter with `categories`.

### 5) App details (structured)

```bash
steam-cli app 413150 --json
steam-cli app 413150 --ttl-sec 86400 --json
```

Use to validate:

- categories/features
- genres
- languages/platforms
- metadata needed for ranking/shortlisting

### 6) User library (optional)

```bash
export STEAM_API_KEY="..."
steam-cli user owned --vanity gaben --limit 200 --offset 0 --json
steam-cli user owned --steamid 76561197960287930 --limit 200 --json
```

## Recommended LLM workflow patterns

### Pattern A - Find the right tag IDs

1. `tags find "<query>"`
2. `search --tags ...`
3. `app <appid>` for top candidates to validate categories/features
4. Return shortlisted results

### Pattern B - Personalize with playtime

1. `user owned --steamid ...`
2. Infer preferences from owned/playtime
3. Choose tags
4. `search --tags ...`

## Tooling and limits

- Per-command `limit` is clamped to max 100.
- Use pagination (`limit` + `offset`) instead of huge single calls.
- `search` can return partial/unstable store-side ordering over time.

## Rate limiting / pacing

- Steam Store endpoints may throttle.
- When chaining many calls, add small randomized delays (300-1200ms).
- Filter early (`search --tags ... --limit ...`) before calling `app` for many appids.

## Safety / privacy notes

- `user owned` requires public game details on Steam privacy settings.
- Never log or print `STEAM_API_KEY`.

## Known limitations

- Search parsing depends on current Steam HTML structure.
- Categories/genres dictionaries are local and may drift until seed DB is refreshed.
- This skill intentionally does not produce recommendations.

## Troubleshooting

- If JSON mode fails, retry with smaller limits or without `--with-facets`.
- If Store search seems noisy, fetch `app` details and re-filter via categories/genres.
