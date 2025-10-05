# VERIFY Task 02: Tables 017-032 (Federation → Key)

## Tables to Verify (16 tables)
- 017_federation_event_queue.surql
- 018_federation_transaction_queue.surql
- 019_federation_transactions.surql
- 020_filter.surql
- 021_ignored_user.surql
- 022_invitation.surql
- 023_live_queries.surql
- 024_login_attempt.surql
- 025_matrix_sync_live_queries.surql
- 026_matrix_sync_notification.surql
- 027_matrix_sync_notification_count.surql
- 028_matrix_sync_notification_update.surql
- 029_matrix_sync_pdu.surql
- 030_matrix_sync_presence.surql
- 031_membership.surql
- 032_one_time_keys.surql

## Verification Steps
Same as Task 01 - verify field mappings, indexes, and permissions

## Key Focus Areas
- Federation tables: Check server_name indexes
- Matrix sync tables: Verify LiveQuery compatibility
- Membership: Critical for room access control
