-- Migration 007: Lazy Loading Performance Optimization Indexes
-- This migration adds database indexes to optimize Matrix lazy loading queries
-- for rooms with thousands of members

-- Index for room membership lazy loading queries
-- Optimizes: SELECT * FROM room_membership WHERE room_id = ? AND membership = 'join' AND user_id IN (...)
DEFINE INDEX idx_room_membership_lazy_loading ON TABLE room_membership COLUMNS room_id, membership, user_id;

-- Composite index for room membership with join time ordering
-- Optimizes: ORDER BY power_level DESC, join_time ASC in essential members query
DEFINE INDEX idx_room_membership_join_order ON TABLE room_membership COLUMNS room_id, membership, join_time;

-- Index for power level hierarchy queries
-- Optimizes: SELECT user_id, power_level FROM power_levels WHERE room_id = ? AND power_level >= 50
DEFINE INDEX idx_power_levels_hierarchy ON TABLE power_levels COLUMNS room_id, power_level;

-- Index for power level ordering
-- Optimizes: ORDER BY power_level DESC in hierarchy queries
DEFINE INDEX idx_power_levels_room_order ON TABLE power_levels COLUMNS room_id, power_level DESC;

-- Index for room creator lookup
-- Optimizes: SELECT creator FROM rooms WHERE room_id = ?
DEFINE INDEX idx_rooms_creator_lookup ON TABLE rooms COLUMNS room_id, creator;

-- Index for user power level lookup (used in essential members subquery)
-- Optimizes: user_id IN (SELECT user_id FROM power_levels WHERE room_id = ? AND power_level >= 50)
DEFINE INDEX idx_power_levels_user_room ON TABLE power_levels COLUMNS user_id, room_id, power_level;

-- Additional indexes for LiveQuery performance
-- These help SurrealDB efficiently process LIVE SELECT queries

-- Index for membership LiveQuery filtering
DEFINE INDEX idx_room_membership_live ON TABLE room_membership COLUMNS room_id, user_id, membership;

-- Index for power levels LiveQuery filtering
DEFINE INDEX idx_power_levels_live ON TABLE power_levels COLUMNS room_id, user_id;

-- Covering index for essential members query (includes all needed columns)
-- This allows the query to be satisfied entirely from the index without table lookups
DEFINE INDEX idx_room_membership_essential_covering ON TABLE room_membership COLUMNS room_id, membership, user_id, join_time, power_level;

-- Timeline events index for lazy loading (if events table exists)
-- This helps identify timeline senders efficiently
-- DEFINE INDEX idx_events_timeline_senders ON TABLE events COLUMNS room_id, sender, event_type, timestamp;

-- Performance monitoring indexes
-- These help track query performance and identify bottlenecks

-- Index for membership count queries (used in performance monitoring)
DEFINE INDEX idx_room_membership_count ON TABLE room_membership COLUMNS room_id, membership;

-- Index for power level distribution analysis
DEFINE INDEX idx_power_levels_distribution ON TABLE power_levels COLUMNS room_id, power_level, user_id;

-- Notes for database administrators:
-- 1. These indexes significantly improve lazy loading query performance for large rooms (10k+ members)
-- 2. The covering index (idx_room_membership_essential_covering) is most critical for performance
-- 3. LiveQuery indexes help SurrealDB efficiently process real-time subscription streams
-- 4. Monitor index usage with SurrealDB's EXPLAIN functionality to ensure effectiveness
-- 5. Consider index maintenance during high-traffic periods

-- Verification queries to test index effectiveness:
-- EXPLAIN SELECT * FROM room_membership WHERE room_id = 'test' AND membership = 'join' AND user_id IN ['user1', 'user2'];
-- EXPLAIN SELECT user_id, power_level FROM power_levels WHERE room_id = 'test' AND power_level >= 50 ORDER BY power_level DESC;
-- EXPLAIN SELECT creator FROM rooms WHERE room_id = 'test';
