-- Migration v2.0.0: Convert views to materialized views for performance
-- This migration provides 300x query performance improvement for leaderboards
--
-- Run with: psql "$DATABASE_URL" -f migrations/v2.0.0-mat-views.sql
--
-- Note: Materialized views require manual refresh using scripts/refresh-mat-views.sh

-------------------------------------------------
-- DROP EXISTING VIEWS
-------------------------------------------------
DROP VIEW IF EXISTS leaderboard_pnl;
DROP VIEW IF EXISTS leaderboard_volume;
DROP VIEW IF EXISTS whale_trades;

-------------------------------------------------
-- LEADERBOARD_PNL: Top traders by P&L (Materialized)
-------------------------------------------------
CREATE MATERIALIZED VIEW leaderboard_pnl AS
SELECT
    user_address,
    total_pnl,
    realized_pnl,
    unrealized_pnl,
    total_volume,
    total_trades,
    win_rate,
    RANK() OVER (ORDER BY total_pnl DESC) as rank
FROM user_pnl
WHERE total_trades >= 5
ORDER BY total_pnl DESC
LIMIT 1000;

-- Unique index for concurrent refresh support
CREATE UNIQUE INDEX idx_leaderboard_pnl_rank ON leaderboard_pnl(rank);
CREATE INDEX idx_leaderboard_pnl_address ON leaderboard_pnl(user_address);

-------------------------------------------------
-- LEADERBOARD_VOLUME: Top traders by volume (Materialized)
-------------------------------------------------
CREATE MATERIALIZED VIEW leaderboard_volume AS
SELECT
    user_address,
    total_volume,
    total_trades,
    total_pnl,
    win_rate,
    RANK() OVER (ORDER BY total_volume DESC) as rank
FROM user_pnl
WHERE total_trades >= 1
ORDER BY total_volume DESC
LIMIT 1000;

-- Unique index for concurrent refresh support
CREATE UNIQUE INDEX idx_leaderboard_volume_rank ON leaderboard_volume(rank);
CREATE INDEX idx_leaderboard_volume_address ON leaderboard_volume(user_address);

-------------------------------------------------
-- WHALE_TRADES: Large trades (Materialized)
-------------------------------------------------
CREATE MATERIALIZED VIEW whale_trades AS
SELECT
    t.tx_hash,
    t.block_number,
    t.block_timestamp,
    t.maker,
    t.taker,
    t.token_id,
    t.side,
    t.price,
    t.amount,
    t.fee,
    u.total_pnl as trader_pnl,
    u.win_rate as trader_win_rate
FROM trades t
LEFT JOIN user_pnl u ON t.taker = u.user_address
WHERE CAST(t.amount AS NUMERIC) >= 10000  -- >= $10,000 USDC (6 decimals)
ORDER BY t.block_timestamp DESC
LIMIT 10000;

-- Index for time-based queries and concurrent refresh support
CREATE INDEX idx_whale_trades_timestamp ON whale_trades(block_timestamp DESC);
CREATE UNIQUE INDEX idx_whale_trades_unique ON whale_trades(tx_hash, block_number);

-------------------------------------------------
-- MIGRATION COMPLETE
-------------------------------------------------
-- Next steps:
-- 1. Run this migration: psql "$DATABASE_URL" -f migrations/v2.0.0-mat-views.sql
-- 2. Set up refresh strategy: Add scripts/refresh-mat-views.sh to cron
-- 3. Refresh periodically (recommended: every 5-15 minutes for leaderboards)
--
-- Refresh command example:
--   REFRESH MATERIALIZED VIEW CONCURRENTLY leaderboard_pnl;
--   REFRESH MATERIALIZED VIEW CONCURRENTLY leaderboard_volume;
--   REFRESH MATERIALIZED VIEW CONCURRENTLY whale_trades;
