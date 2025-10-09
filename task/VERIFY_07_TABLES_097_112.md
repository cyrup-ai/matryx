# VERIFY Task 07: Table 112 Index Correction

## Executive Summary

**Status:** CRITICAL BUG - Index references non-existent fields
**Impact:** Production-blocking performance issue causing full table scans
**Affected File:** `packages/surrealdb/migrations/tables/112_push_notification.surql`

---

## CRITICAL BUG: Table 112 - Incorrect Index Field References

### Problem
After restructuring table 112 to use nested `notification_data.*` fields, the indexes on lines 24-25 still reference the old root-level field names that no longer exist.

**Current Broken Indexes (Lines 24-25):**
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS status;
```

**Issue:** Fields `user_id` and `status` do NOT exist at root level. They are now nested as:
- `notification_data.user_id` (line 16)
- `notification_data.status` (line 20)

### Repository Impact

All repository queries in `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/push_notification.rs` correctly use nested paths:
- Line 127: `WHERE notification_data.status = 'Pending'`
- Line 143: `WHERE notification_data.user_id = $user_id`
- Line 230: `WHERE notification_data.status = 'Sent'`
- Line 249: `WHERE notification_data.status = 'Delivered'`
- Line 287: `WHERE notification_data.status = 'Failed'`
- Line 321: `WHERE notification_data.status = 'Failed'`

**Result:** These queries will perform FULL TABLE SCANS instead of using indexes, causing severe performance degradation.

### Required Fix

**File:** `/Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/tables/112_push_notification.surql`

Replace lines 24-25:
```sql
DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS notification_data.user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS notification_data.status;
```

### Current Schema Structure (Correct)
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

---

## COMPLETED ITEMS (Verified ✓)

All other requirements from the original task have been successfully implemented:

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

---

## DEFINITION OF DONE

Task is complete when:

1. Table 112 indexes correctly reference nested field paths:
   - `pn_user_idx` uses `notification_data.user_id`
   - `pn_status_idx` uses `notification_data.status`

2. Schema file passes SurrealDB syntax validation (no migration errors)

3. Repository queries can utilize indexes for performance optimization

---

## VERIFICATION

After fixing the indexes, verify:
1. Migration applies successfully (check SurrealDB logs)
2. No schema validation errors
3. Indexes are created on the correct nested fields
