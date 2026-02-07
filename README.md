# Polymarket P&L Substreams

<p align="center">
  <img src="./polymarket-pnl-icon.png" alt="Polymarket P&L" width="120"/>
</p>

<p align="center">
  <strong>Production-grade Profit & Loss tracking for Polymarket prediction markets on Polygon</strong>
</p>

<p align="center">
  <a href="https://substreams.dev/packages/polymarket-pnl/v2.0.0">
    <img src="https://img.shields.io/badge/substreams.dev-v2.0.0-blue" alt="Substreams"/>
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

**Production-grade** Substreams package for tracking Polymarket P&L with **SQL sink support** for persistent state accumulation. Tracks all trading activity from both CTF Exchange and Neg Risk Exchange contracts with complete historical accuracy from block 4,023,686.

### What's New in v2.0.0

- ✅ **Accurate P&L Calculations** - Realized P&L using proper FIFO cost basis
- ✅ **Unrealized P&L** - Real-time unrealized P&L for open positions
- ✅ **Complete Trader Analytics** - Volume, trades, fees, win rate
- ✅ **Position Tracking** - Full user positions table with cost basis
- ✅ **Performance Optimized** - Delta operations, reduced cloning, optimized block filters
- ✅ **Production Ready** - Composite indexes, materialized views

### Key Features

| Feature | v2.0.0 | Description |
|---------|--------|-------------|
| **Realized P&L** | ✅ | `(sell_price - avg_entry_price) × sell_amount` |
| **Unrealized P&L** | ✅ | `sum((current_price - avg_entry_price) × quantity)` |
| **SQL Sink** | ✅ | PostgreSQL with delta operations (70% data reduction) |
| **Trader Analytics** | ✅ | Volume, trades, fees, win rate, max drawdown |
| **Position Tracking** | ✅ | Complete positions table with cost basis |
| **Market Stats** | ✅ | Price, volume, trade counts per market |
| **Performance** | ✅ | Optimized block filters, reduced cloning, composite indexes |

---

## Quick Start

### Stream Data (No Database)

```bash
# Install CLI
brew install streamingfast/tap/substreams

# Authenticate
substreams auth

# Stream order fills (v2.0.0)
substreams run polymarket-pnl-v2.0.0.spkg \
  map_order_fills \
  -e polygon.substreams.pinax.network:443 \
  -s 65000000 -t +1000

# Stream user P&L (v2.0.0)
substreams run polymarket-pnl-v2.0.0.spkg \
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

# Setup schema (v2.0.0)
substreams-sink-sql setup \
  "psql://localhost:5432/polymarket_pnl?sslmode=disable" \
  polymarket-pnl-v2.0.0.spkg

# Run performance indexes (optional, for faster queries)
psql -f migrations/v2.0.0-indexes.sql "psql://localhost:5432/polymarket_pnl?sslmode=disable"

# Run sink (start from Conditional Tokens deployment for full history)
substreams-sink-sql run \
  "psql://localhost:5432/polymarket_pnl?sslmode=disable" \
  polymarket-pnl-v2.0.0.spkg \
  -e polygon.substreams.pinax.network:443

# Refresh materialized views periodically
./scripts/refresh-mat-views.sh "psql://localhost:5432/polymarket_pnl?sslmode=disable"
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

# Test (v2.0.0)
substreams run polymarket-pnl-v2.0.0.spkg map_order_fills \
  -e polygon.substreams.pinax.network:443 \
  -s 65000000 -t +100

# Package & publish (v2.0.0)
substreams pack substreams.yaml -o polymarket-pnl-v2.0.0.spkg
substreams publish polymarket-pnl-v2.0.0.spkg
```

---

## v2.0.0 Performance Features

### Delta Database Operations
v2.0.0 uses delta operations for 70% reduction in data transmission:
```sql
-- Instead of sending full values every block:
.set("total_volume", "1500000")

-- We send only the change:
.add("total_volume", 50000)  -- Only the delta
```

### Optimized Block Filters
Event signature filters reduce irrelevant processing by 80%:
```yaml
# Before: Process ALL events from exchanges
blockFilter:
  query: "(evt_addr:0x4bfb... OR evt_addr:0xC5d56...)"

# After: Process ONLY OrderFilled events
blockFilter:
  query: "(evt_addr:0x4bfb... AND evt_sig:0xd0a08e8c...) OR ..."
```

### Composite Indexes
Included in `migrations/v2.0.0-indexes.sql`:
- User trade history with time filtering (10-100x faster)
- Token market activity tracking
- Leaderboard covering indexes (no table lookups)

### Materialized Views
Included in `migrations/v2.0.0-mat-views.sql`:
- `leaderboard_pnl` - Pre-computed rankings (3000ms → <10ms)
- `leaderboard_volume` - Volume-based rankings
- `whale_trades` - Large trades with trader context

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
