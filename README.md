# ARES-Database — Automated Ranking & Evaluation System

Welcome to **ARES-Database**, the high-performance backend engine for managing and updating team performance metrics for FTC competitions. Built in Rust for speed and reliability, this tool connects to the official FTC API, calculates OPR-based statistics, ranks teams, and stores everything in a Supabase database.

---

## Setup Instructions

### 1. Clone the repository

```bash
git clone https://github.com/ares-hq/ares.git
cd ares/db
```

### 2. Create a `.env` file

Copy and paste this template into a new file called `.env`:

```env
SUPABASE_URL=https://your-project.supabase.co
SUPABASE_KEY=your-anon-or-service-role-key
FIRST_USERNAME=your-first-api-username
FIRST_PASS=your-first-api-password
SEASON_TABLE=season_2025
MATCH_TABLE=matches_2025
```

### 3. Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 4. Build and run

```bash
cargo build --release
./target/release/db
```

Or run directly:

```bash
cargo run --release
```

#### Command-line flags:
- `--all-events` — Process all events (default: future events only)
- `--force-update` — Force overwrite existing database values

---

## Features

- **High Performance**: Rust implementation with async/await and bounded concurrency
- **OPR Calculation**: Least-squares solver using faer for Auto, TeleOp, Endgame, and Penalties
- **Smart Merging**: Only updates Supabase if data improves or when forced
- **Year Adapters**: Automatically handles scoring rule changes across seasons (2019-2025)
- **Team Matrix Builder**: Optimized with HashSet deduplication for O(1) lookups
- **Robust Error Handling**: Retry logic with exponential backoff for API calls
- **Structured Logging**: Uses tracing for clear operation visibility

---

## Deployment

### Run with auto-update monitoring:
```bash
chmod +x ./update_database.sh
nohup ./update_database.sh > monitor.log 2>&1 &
```

This script will:
1. Build the Rust binary
2. Run the pipeline
3. Check for git updates every 5 minutes
4. Rebuild and restart on code changes

### View live logs:
```bash
tail -f monitor.log
```

### Stop the process:
```bash
pkill -f "target/release/db"
```

---

## Project Structure

```
db/
│
├── src/
│   ├── main.rs              # Entry point and orchestration
│   ├── api_client.rs        # HTTP client with retry logic
│   ├── api_params.rs        # URL parameter builder
│   ├── config.rs            # Environment variable loading
│   ├── first_api.rs         # FTC API orchestration
│   ├── processor.rs         # Database merge/rank/upsert logic
│   ├── year_adapters.rs     # Season-specific scoring rules
│   └── utils/
│       ├── matrix_math.rs   # Least-squares solver (faer)
│       └── team_builder.rs  # Matrix construction from match data
│
├── Cargo.toml               # Rust dependencies
├── update_database.sh       # Auto-update monitoring script
├── .env                     # Environment variables (keep secret!)
└── README.md                # You're here!
```

---

## Architecture

1. **Config Loading**: Reads environment variables for API credentials and database connection
2. **Event Fetching**: Retrieves events from FTC API with bounded concurrency (16 events parallel)
3. **Match Processing**: For each event, fetches matches and scores
4. **Matrix Building**: Constructs alliance matrices and score vectors
5. **OPR Solving**: Uses QR decomposition least-squares solver (faer)
6. **Database Merge**: Preserves existing metadata (founded, website, events_attended)
7. **Ranking**: Assigns ranks across all metrics with tie handling
8. **Upsert**: Bulk updates Supabase with latest data

---

## Dependencies

Core libraries:
- **faer**: Pure-Rust linear algebra for least-squares solving
- **ndarray**: Matrix and vector storage
- **reqwest**: Async HTTP client with connection pooling
- **tokio**: Async runtime
- **postgrest**: Supabase client
- **cache**: Shared domain types (Team, Event, Match, etc.)

See [Cargo.toml](Cargo.toml) for full dependency list.

---

## Tips

- Use `--force-update` flag to overwrite all database values, otherwise only improvements are merged
- Supabase conflicts are handled via `upsert()` using `team_number` as the key
- Set `RUST_LOG=debug` environment variable for verbose logging
- The cache dependency is pulled from the `ares-hq/cache` GitHub repository
- Year adapters automatically handle scoring rule changes - add new years as needed
- Connection pooling and async concurrency are built-in for optimal performance

---

## Author

**Henry Bonomolo**  
Email: hbono@berkeley.edu  
GitHub: [@henrybono](https://github.com/henrybono)

---

## License

MIT License. Feel free to use, improve, and share — just credit where credit is due!

---

_The ARES system — built to empower teams through stats, structure, and strategy._
