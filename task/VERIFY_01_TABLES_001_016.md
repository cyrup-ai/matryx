# VERIFY Task 01: Tables 001-016 (Account → Bridge)

## Objective
Verify indexes and field mappings for tables 001-016

## Tables to Verify (16 tables)
- 001_account_data.surql
- 002_access_token.surql
- 003_client_ip_address_mapping.surql
- 004_content_reports.surql
- 005_cross_signing_keys.surql
- 006_crypto_context.surql
- 007_crypto_device_info.surql
- 008_crypto_one_time_keys.surql
- 009_device_keys.surql
- 010_device_list_stream.surql
- 011_event.surql
- 012_event_auth.surql
- 013_event_dag.surql
- 014_event_json.surql
- 015_event_relations.surql
- 016_event_replacement.surql

## Verification Steps

For each table:

### 1. Field Mapping Verification
- [ ] Read corresponding entity file from `packages/entity/src/types/`
- [ ] Read corresponding repository file(s) from `packages/surrealdb/src/repository/`
- [ ] Verify all entity fields are present in table definition
- [ ] Verify field types match (String→string, DateTime→datetime, etc.)
- [ ] Check for missing optional fields (TYPE option<type>)
- [ ] Verify Matrix ID validation patterns where applicable

### 2. Index Verification
- [ ] Search repository for all queries on this table
- [ ] Extract WHERE clauses to identify query patterns
- [ ] Verify existing indexes cover query patterns
- [ ] Check for missing indexes on frequently queried columns
- [ ] Verify UNIQUE constraints where appropriate
- [ ] Look for JOIN patterns requiring relationship indexes

### 3. Permission Verification
- [ ] Check if permissions match security requirements
- [ ] Verify $auth.user_id patterns for user-specific data
- [ ] Verify $auth.admin patterns for admin-only operations
- [ ] Check $auth.server_name for federation tables

## Commands to Run

```bash
# For each table, check repository usage:
cd /Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository
grep -rn "FROM table_name" . | head -20

# Check entity definition:
find /Volumes/samsung_t9/maxtryx/packages/entity/src/types -name "*entity_name*"
```

## Report Format

For each issue found:
```
Table: XXX_table_name.surql
Issue: [Missing Index | Wrong Type | Missing Field]
Details: [Specific description]
Fix: [Proposed change]
```
