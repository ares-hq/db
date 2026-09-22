# ARES — Database Pipeline

Fetches a season from the FIRST API, solves per-team OPR, ranks every team, and upserts to
Supabase. One run is one pass: fetch, solve, write, exit. The Discord bot reads what this
writes; table names are shared through [`model::tables`](https://github.com/ares-hq/model).

## Tables

| Table | Key | Contents |
|---|---|---|
| `season_<year>` | `teamNumber` | OPR by phase, ranks, metadata, events attended |
| `matches_<year>` | `matchcode` | Two rows per played match, one per alliance |

`matchcode` hashes match identity, never score, so a rescored match updates in place.

`overallOPR` is `auto + teleop`. Endgame and penalties rank on their own axes, penalties
ascending.

## Run

```env
SUPABASE_URL=
SUPABASE_KEY=
FIRST_USERNAME=
FIRST_PASS=
```

```bash
cargo run --release --bin db -- --year 2025 --all-events
```

| Flag | Effect |
|---|---|
| `--year <YEAR>` | Season. Defaults to the current one, rolling over in August. |
| `--all-events` | Whole schedule, not just the last week onward. Implied for past seasons. |
| `--force-update` | Overwrite stored figures even when the stored event was stronger. |

`RUST_LOG=debug` turns up the logging.

## Deploy

A batch job, so it runs one-shot on a timer rather than as a supervised loop. Setup in
[`deploy/README.md`](../deploy/README.md).

```bash
docker compose run --rm db --year 2025 --all-events
```

## Tests

```bash
cargo test -p db
```
