# Polymarket P&L Substreams

<p align="center">
  <img src="./polymarket-pnl-icon.png" alt="Polymarket P&L" width="120"/>
</p>

<p align="center">
  <strong>Real-time Profit & Loss tracking for Polymarket prediction markets on Polygon</strong>
</p>

<p align="center">
  <a href="https://substreams.dev/packages/polymarket-pnl/v1.0.1">
    <img src="https://img.shields.io/badge/substreams.dev-v1.0.1-blue" alt="Substreams"/>
  </a>
  <a href="https://polygon.technology/">
    <img src="https://img.shields.io/badge/network-Polygon-8247E5" alt="Polygon"/>
  </a>
  <a href="https://www.postgresql.org/">
    <img src="https://img.shields.io/badge/sink-PostgreSQL-336791" alt="PostgreSQL"/>
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-green" alt="License"/>
  </a>
</p>

---

## Overview

Comprehensive Substreams package for tracking Polymarket P&L with **SQL sink support** for persistent state accumulation. Tracks all trading activity from both CTF Exchange and Neg Risk Exchange contracts.

### Key Features

| Feature | Description |
|---------|-------------|
| **Real P&L Tracking** | Realized & unrealized P&L with cost basis |
| **SQL Sink** | PostgreSQL/Clickhouse for persistent state |
| **Trader Analytics** | Volume, win rate, max drawdown |
| **Market Stats** | Price, volume, trade counts per market |
| **Whale Detection** | Large trade tracking with trader context |

---

## Quick Start

### Stream Data (No Database)

```bash
# Install CLI
brew install streamingfast/tap/substreams

# Authenticate
substreams auth

# Stream order fills
substreams run https://spkg.io/PaulieB14/polymarket-pnl-v1.0.1.spkg \
  map_order_fills \
  -e polygon.substreams.pinax.network:443 \
  -s 65000000 -t +1000

# Stream user P&L
substreams run https://spkg.io/PaulieB14/polymarket-pnl-v1.0.1.spkg \
  map_user_pnl \
  -e polygon.substreams.pinax.network:443 \
  -s 65000000 -t +1000
```

### Sink to PostgreSQL (Required for P&L)

> **Important:** P&L requires accumulated state. Use the SQL sink for accurate calculations.

```bash
# Install sink
brew install streamingfast/tap/substreams-sink-sql

# Create database
createdb polymarket_pnl

# Setup schema
substreams-sink-sql setup \
  "psql://localhost:5432/polymarket_pnl?sslmode=disable" \
  https://spkg.io/PaulieB14/polymarket-pnl-v1.0.1.spkg

# Run sink (start from beginning for full history)
substreams-sink-sql run \
  "psql://localhost:5432/polymarket_pnl?sslmode=disable" \
  https://spkg.io/PaulieB14/polymarket-pnl-v1.0.1.spkg \
  -e polygon.substreams.pinax.network:443
```

### Query Your Data

```sql
-- Top traders by P&L
SELECT * FROM leaderboard_pnl LIMIT 20;

-- Whale trades with trader stats
SELECT * FROM whale_trades;

-- User positions
SELECT * FROM user_positions
WHERE user_address = '0x...' AND quantity > 0;

-- Daily stats
SELECT date, total_volume, total_trades
FROM daily_stats ORDER BY date DESC;
```

---

## Architecture

```
                    Polygon Blockchain
                           │
                           ▼
              ┌─────────────────────────┐
              │     Firehose Blocks     │
              └─────────────────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ map_order_fills │ │map_token_transf │ │map_usdc_transf  │
│  (CTF + NegRisk)│ │    (ERC1155)    │ │    (USDC)       │
└─────────────────┘ └─────────────────┘ └─────────────────┘
         │                 │                   │
         └─────────────────┼───────────────────┘
                           ▼
              ┌─────────────────────────┐
              │    STORES (State)       │
              │  • user_positions       │
              │  • user_cost_basis      │
              │  • user_realized_pnl    │
              │  • latest_prices        │
              └─────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │      map_user_pnl       │
              │   (Computed Analytics)  │
              └─────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │         db_out          │
              │      (SQL Sink)         │
              └─────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │       PostgreSQL        │
              └─────────────────────────┘
```

---

## Modules

### Layer 1: Event Extraction

| Module | Description |
|--------|-------------|
| `map_order_fills` | OrderFilled events from CTF & NegRisk exchanges |
| `map_token_transfers` | ERC1155 TransferSingle events |
| `map_usdc_transfers` | USDC transfer events |

### Layer 2: State Stores

| Store | Key | Description |
|-------|-----|-------------|
| `store_user_positions` | `{user}:{token}` | Position quantities |
| `store_user_cost_basis` | `{user}:{token}` | Total cost basis |
| `store_user_realized_pnl` | `{user}` | Realized P&L |
| `store_user_volume` | `{user}` | Trading volume |
| `store_user_trade_count` | `{user}` | Trade count |
| `store_market_volume` | `{token}` | Market volume |
| `store_latest_prices` | `{token}` | Latest prices |

### Layer 3: Analytics

| Module | Description |
|--------|-------------|
| `map_user_pnl` | Real-time P&L calculations |
| `map_market_stats` | Market-level statistics |

### Layer 4: Sink

| Module | Description |
|--------|-------------|
| `db_out` | Database changes for SQL sink |

---

## Database Schema

### Tables

| Table | Description |
|-------|-------------|
| `trades` | All order fills with price, amount, side |
| `user_pnl` | Aggregated P&L per user |
| `user_positions` | Current positions with cost basis |
| `markets` | Market statistics |
| `daily_stats` | Daily aggregates |

### Views

| View | Description |
|------|-------------|
| `leaderboard_pnl` | Top 1000 by P&L |
| `leaderboard_volume` | Top 1000 by volume |
| `whale_trades` | Trades >$10K |

---

## Contract Addresses

| Contract | Address | Start Block |
|----------|---------|-------------|
| CTF Exchange | `0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e` | 33,605,403 |
| NegRisk Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` | 50,505,492 |
| Conditional Tokens | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` | 4,023,686 |
| USDC | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` | 4,023,686 |

---

## Build from Source

```bash
# Clone
git clone https://github.com/PaulieB14/Polymarket-P-L-Substreams
cd Polymarket-P-L-Substreams

# Build
substreams build

# Test
substreams run substreams.yaml map_order_fills \
  -e polygon.substreams.pinax.network:443 \
  -s 65000000 -t +100

# Package & publish
substreams pack substreams.yaml -o polymarket-pnl-v1.0.1.spkg
substreams publish polymarket-pnl-v1.0.1.spkg
```

---

## Why SQL Sink?

P&L calculation requires **state accumulation** over time:

| Without Sink | With SQL Sink |
|--------------|---------------|
| No history | Full history persisted |
| P&L = $0 | Accurate P&L |
| Stateless | Tracks cost basis |
| Demo only | Production ready |

---

## Related

- [polymarket-orderbook-substreams](https://substreams.dev/packages/polymarket-orderbook-substreams/v0.2.0) - Order flow & trader leaderboards

---

## License

MIT
