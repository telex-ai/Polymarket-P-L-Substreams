-- Polymarket P&L Substreams - Composite Indexes Migration
-- Version: 2.0.0
--
-- Purpose: Add composite indexes for optimized query performance
-- Run with: psql -f migrations/v2.0.0-indexes.sql postgres://...
--
-- Note: Uses CONCURRENTLY to avoid blocking writes during index creation
-- This requires PostgreSQL 12+. For earlier versions, remove CONCURRENTLY.

-------------------------------------------------
-- COMPOSITE INDEXES FOR TRADES TABLE
-------------------------------------------------

-- Index: User trade history with time filtering
-- Purpose: Optimizes queries fetching a user's trades ordered by time
-- Query pattern: WHERE taker = ? ORDER BY block_timestamp DESC
-- Performance benefit: 10-100x faster than sequential scans on large datasets
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_taker_timestamp
    ON trades(taker, block_timestamp DESC);

-- Index: Token market activity with time filtering
-- Purpose: Optimizes queries for market-level trade history
-- Query pattern: WHERE token_id = ? ORDER BY block_timestamp DESC
-- Use case: Market analytics, recent activity feeds, token performance
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_token_timestamp
    ON trades(token_id, block_timestamp DESC);

-- Index: User positions in specific markets
-- Purpose: Optimizes queries for user's activity in particular markets
-- Query pattern: WHERE taker = ? AND token_id = ? ORDER BY block_timestamp DESC
-- Use case: User position analysis, market-specific trading history
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_taker_token_timestamp
    ON trades(taker, token_id, block_timestamp DESC);

-------------------------------------------------
-- COMPOSITE INDEX FOR USER_PNL TABLE
-------------------------------------------------

-- Index: Covering index for leaderboard queries
-- Purpose: Optimizes leaderboard queries without requiring table access
-- Query pattern: ORDER BY total_trades, total_pnl DESC
--
-- INCLUDE clause adds non-key columns to the index for covering queries:
-- - Eliminates table lookups when these columns are all that's needed
-- - PostgreSQL 11+ feature - remove INCLUDE clause for earlier versions
-- - Fallback for PostgreSQL < 11: Use standard multi-column index
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_pnl_leaderboard
    ON user_pnl(total_trades, total_pnl DESC)
    INCLUDE (user_address, realized_pnl, unrealized_pnl, total_volume, win_rate);

-------------------------------------------------
-- VERIFICATION QUERIES
-------------------------------------------------

-- After running this migration, verify indexes were created:
-- SELECT indexname, indexdef FROM pg_indexes WHERE tablename IN ('trades', 'user_pnl');

-- Check index sizes:
-- SELECT
--     schemaname,
--     tablename,
--     indexname,
--     pg_size_pretty(pg_relation_size(indexrelid::regclass)) as size
-- FROM pg_stat_user_indexes
-- WHERE tablename IN ('trades', 'user_pnl')
-- ORDER BY pg_relation_size(indexrelid::regclass) DESC;

-------------------------------------------------
-- ROLLBACK (if needed)
-------------------------------------------------

-- To remove these indexes:
-- DROP INDEX CONCURRENTLY IF EXISTS idx_trades_taker_timestamp;
-- DROP INDEX CONCURRENTLY IF EXISTS idx_trades_token_timestamp;
-- DROP INDEX CONCURRENTLY IF EXISTS idx_trades_taker_token_timestamp;
-- DROP INDEX CONCURRENTLY IF EXISTS idx_user_pnl_leaderboard;
