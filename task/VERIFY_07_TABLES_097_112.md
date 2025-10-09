# VERIFY Task 07: Table 112 Index Correction - CRITICAL BUG FIX

## Executive Summary

**Status:** CRITICAL BUG - Index references non-existent fields  
**Impact:** Production-blocking performance issue causing full table scans  
**Affected File:** [`packages/surrealdb/migrations/tables/112_push_notification.surql`](../packages/surrealdb/migrations/tables/112_push_notification.surql)  
**Repository File:** [`packages/surrealdb/src/repository/push_notification.rs`](../packages/surrealdb/src/repository/push_notification.rs)

---

## Problem Analysis

### Current Schema Structure (Lines 14-22)

The table 112 schema correctly defines nested fields under `notification_data`:

```sql
DEFINE FIELD notification_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data ON TABLE push_notification TYPE object;
DEFINE FIELD notification_data.user_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD notification_data.event_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD notification_data.room_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD notification_data.pusher_key ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data.status ON TABLE push_notification TYPE string DEFAULT 'Pending';
DEFINE FIELD notification_data.created_at ON TABLE push_notification TYPE datetime DEFAULT time::now();
DEFINE FIELD attempts ON TABLE push_notification TYPE int DEFAULT 0;
```

**Note:** Fields `user_id` and `status` are **nested under** `notification_data.*`, NOT at root level.

### Broken Index Definitions (Lines 24-25)

```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS status;
```

**THE BUG:** These indexes reference `user_id` and `status` as root-level fields, but they don't exist at root level!

### Why This Breaks

SurrealDB indexes must reference **actual field paths**. The fields referenced in the index don't exist:
- ❌ `user_id` does NOT exist (should be `notification_data.user_id`)
- ❌ `status` does NOT exist (should be `notification_data.status`)

**Result:** Queries using these fields perform **FULL TABLE SCANS** instead of index lookups, causing severe performance degradation at scale.

---

## Impact Analysis: Repository Queries Affected

All queries in [`push_notification.rs`](../packages/surrealdb/src/repository/push_notification.rs) correctly use nested paths and expect these indexes to work:

### Query 1: `get_pending_notifications()` - Line 127
```rust
"SELECT * FROM push_notification WHERE notification_data.status = 'Pending' ORDER BY notification_data.created_at ASC"
```
**Expected Index:** `pn_status_idx` on `notification_data.status`  
**Current Behavior:** Full table scan (index targets non-existent `status`)

### Query 2: `get_user_notifications()` - Line 143
```rust
"SELECT * FROM push_notification WHERE notification_data.user_id = $user_id ORDER BY notification_data.created_at DESC"
```
**Expected Index:** `pn_user_idx` on `notification_data.user_id`  
**Current Behavior:** Full table scan (index targets non-existent `user_id`)

### Query 3: `get_notification_statistics()` - Lines 230, 249, 287, 321
```rust
// Line 230
"SELECT count() AS pending {} AND notification_data.status = 'Pending'"

// Line 249
"SELECT count() AS sent {} AND notification_data.status = 'Sent'"

// Line 287
"SELECT count() AS delivered {} AND notification_data.status = 'Delivered'"

// Line 321
"SELECT count() AS failed {} AND notification_data.status = 'Failed'"
```
**Expected Index:** `pn_status_idx` on `notification_data.status`  
**Current Behavior:** Full table scan on every statistics query

### Query 4: `get_failed_notifications()` - Line 287
```rust
"SELECT * FROM push_notification WHERE notification_data.status = 'Failed' ORDER BY notification_data.created_at DESC"
```
**Expected Index:** `pn_status_idx` on `notification_data.status`  
**Current Behavior:** Full table scan

### Query 5: `cleanup_delivered_notifications()` - Line 412
```rust
"DELETE FROM push_notification WHERE notification_data.status = 'Delivered' AND delivered_at < $cutoff"
```
**Expected Index:** `pn_status_idx` on `notification_data.status`  
**Current Behavior:** Full table scan

**Performance Impact:** With thousands of push notifications, each query scans the entire table instead of using efficient index lookups. This impacts:
- Real-time push notification delivery
- User notification retrieval
- Statistics dashboards
- Cleanup operations

---

## Solution: Correct Index Field Paths

### Examples from Working Tables

Other tables in the migration set show the correct pattern for root-level field indexing:

#### Table 105: [`notifications.surql`](../packages/surrealdb/migrations/tables/105_notifications.surql) (Lines 22-26)
```sql
-- Root-level fields
DEFINE FIELD user_id ON TABLE notifications TYPE string ...;
DEFINE FIELD read ON TABLE notifications TYPE bool ...;
DEFINE FIELD created_at ON TABLE notifications TYPE datetime ...;

-- Indexes correctly reference root-level fields
DEFINE INDEX notif_user_idx ON TABLE notifications COLUMNS user_id;
DEFINE INDEX notif_user_read_idx ON TABLE notifications COLUMNS user_id, read;
DEFINE INDEX notif_user_created_idx ON TABLE notifications COLUMNS user_id, created_at;
```

#### Table 104: [`presence_events.surql`](../packages/surrealdb/migrations/tables/104_presence_events.surql) (Lines 20-22)
```sql
-- Root-level fields
DEFINE FIELD user_id ON TABLE presence_events TYPE string ...;
DEFINE FIELD updated_at ON TABLE presence_events TYPE datetime ...;

-- Indexes correctly reference root-level fields
DEFINE INDEX pe_user_idx ON TABLE presence_events COLUMNS user_id;
DEFINE INDEX pe_user_updated_idx ON TABLE presence_events COLUMNS user_id, updated_at;
```

**Key Pattern:** Indexes must reference the **exact field path** as defined in DEFINE FIELD statements.

### The Fix for Table 112

Since table 112 uses nested fields under `notification_data.*`, the indexes must use the **full nested path**:

#### BEFORE (Broken - Lines 24-25):
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS status;
```

#### AFTER (Fixed):
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS notification_data.user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS notification_data.status;
```

---

## Implementation Steps

### Step 1: Open the Migration File
```bash
$EDITOR /Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/tables/112_push_notification.surql
```

### Step 2: Locate Lines 24-25
Find the broken index definitions:
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS status;
```

### Step 3: Replace with Corrected Indexes
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS notification_data.user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS notification_data.status;
```

### Step 4: Save the File
The complete corrected migration file should be:

```sql
-- =====================================================
-- Migration: 112
-- Table: push_notification
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/push_notification.rs
-- =====================================================

DEFINE TABLE push_notification SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD notification_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data ON TABLE push_notification TYPE object;
DEFINE FIELD notification_data.user_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD notification_data.event_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD notification_data.room_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD notification_data.pusher_key ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data.status ON TABLE push_notification TYPE string DEFAULT 'Pending';
DEFINE FIELD notification_data.created_at ON TABLE push_notification TYPE datetime DEFAULT time::now();
DEFINE FIELD attempts ON TABLE push_notification TYPE int DEFAULT 0;

DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS notification_data.user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS notification_data.status;
```

---

## Definition of Done

This task is complete when:

1. ✅ Lines 24-25 in `112_push_notification.surql` reference nested field paths:
   - `pn_user_idx` uses `notification_data.user_id`
   - `pn_status_idx` uses `notification_data.status`

2. ✅ The migration file passes SurrealDB syntax validation (no parse errors)

3. ✅ Repository queries in `push_notification.rs` can utilize the corrected indexes for:
   - User-specific notification lookups (line 143)
   - Status-filtered queries (lines 127, 230, 249, 287, 321)
   - Pending notification retrieval (line 127)
   - Failed notification retrieval (line 287)
   - Cleanup operations (line 412)

---

## Additional Context

### Schema Design Note
Table 112 uses a nested `notification_data.*` structure, which is different from most other tables in the migration set (097-111) that use root-level fields. This was intentional to organize related notification fields, but requires corresponding index paths.

### No Code Changes Required
The repository implementation in `push_notification.rs` is **already correct** - it consistently uses the nested field paths in all queries. Only the migration file needs to be fixed.

### Related Files
- Migration: [`packages/surrealdb/migrations/tables/112_push_notification.surql`](../packages/surrealdb/migrations/tables/112_push_notification.surql)
- Repository: [`packages/surrealdb/src/repository/push_notification.rs`](../packages/surrealdb/src/repository/push_notification.rs)
- Reference Examples:
  - [`packages/surrealdb/migrations/tables/105_notifications.surql`](../packages/surrealdb/migrations/tables/105_notifications.surql)
  - [`packages/surrealdb/migrations/tables/104_presence_events.surql`](../packages/surrealdb/migrations/tables/104_presence_events.surql)
  - [`packages/surrealdb/migrations/tables/101_room_state_events.surql`](../packages/surrealdb/migrations/tables/101_room_state_events.surql)

---

## COMPLETED VERIFICATION ITEMS ✓

All other requirements from the original verification task have been successfully implemented:

### Critical Schema Fixes ✓
1. ✓ Table 108 - `suggested` field added (line 18)
2. ✓ Table 109 - `device_change_type` field name corrected (line 16)
3. ✓ Table 112 - nested `notification_data.*` structure implemented (lines 15-21)

### Matrix ID Validations ✓
All 39+ Matrix ID fields across tables 097-112 have proper validation assertions:
- ✓ Table 097: room_id (!), event_id ($), sender (@)
- ✓ Table 098: alias (#), room_id (!), creator (@)
- ✓ Table 099: sender_id (@), recipient_id (@)
- ✓ Table 100: room_id (!) optional, sender (@)
- ✓ Table 101: event_id ($), room_id (!), sender (@)
- ✓ Table 102: event_id ($), room_id (!), sender (@)
- ✓ Table 103: user_id (@), sender (@)
- ✓ Table 104: user_id (@)
- ✓ Table 105: user_id (@), event_id ($), room_id (!)
- ✓ Table 106: user_id (@), room_id (!)
- ✓ Table 107: room_id (!)
- ✓ Table 108: parent_room_id (!), child_room_id (!)
- ✓ Table 109: user_id (@)
- ✓ Table 110: room_id (!), user_id (@)
- ✓ Table 111: user_id (@)
- ✓ Table 112: notification_data.user_id (@), notification_data.event_id ($), notification_data.room_id (!)

### Performance Optimization ✓
- ✓ Table 104: Composite index `pe_user_updated_idx` on (user_id, updated_at) added (line 21)
