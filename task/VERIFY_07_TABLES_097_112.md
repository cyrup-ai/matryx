# VERIFY Task 07: Tables 097-112 (NEW P0-P1 Critical)

## Tables to Verify (16 tables) - NEWLY CREATED
- 097_room_aliases.surql ⭐ P0
- 098_direct_to_device_messages.surql ⭐ P0
- 099_ephemeral_events.surql ⭐ P0
- 100_room_state_events.surql ⭐ P0
- 101_room_timeline_events.surql ⭐ P0
- 102_to_device_events.surql ⭐ P0
- 103_presence_events.surql ⭐ P0
- 104_notifications.surql ⭐ P0
- 105_room_account_data.surql ⭐ P0
- 106_room_summaries.surql ⭐ P0
- 107_room_hierarchy.surql ⭐ P0
- 108_device_list_updates.surql 🔸 P1
- 109_lazy_loading.surql 🔸 P1
- 110_notification_settings.surql 🔸 P1
- 111_push_notification.surql 🔸 P1
- 112_push_attempt.surql 🔸 P1

## Critical Verification Points
- These are P0/P1 tables - highest priority for correctness
- Verify all Matrix ID validation patterns (@, !, $, #)
- Check query patterns from room.rs, sync.rs, device.rs, notification.rs
- Ensure indexes match WHERE clauses in repository queries
