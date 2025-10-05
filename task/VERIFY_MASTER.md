# MASTER VERIFICATION: All 156 SurrealDB Tables

## Overview
Comprehensive verification of all 156 SurrealDB migration tables for correct field mappings and optimal indexes.

## Sub-Tasks (10 total)
- ✅ [VERIFY_01](./VERIFY_01_TABLES_001_016.md): Tables 001-016 (16 tables)
- ✅ [VERIFY_02](./VERIFY_02_TABLES_017_032.md): Tables 017-032 (16 tables)
- ✅ [VERIFY_03](./VERIFY_03_TABLES_033_048.md): Tables 033-048 (16 tables)
- ✅ [VERIFY_04](./VERIFY_04_TABLES_049_064.md): Tables 049-064 (16 tables)
- ✅ [VERIFY_05](./VERIFY_05_TABLES_065_080.md): Tables 065-080 (16 tables)
- ✅ [VERIFY_06](./VERIFY_06_TABLES_081_096.md): Tables 081-096 (16 tables)
- ✅ [VERIFY_07](./VERIFY_07_TABLES_097_112.md): Tables 097-112 (16 tables) ⭐ P0-P1 NEW
- ✅ [VERIFY_08](./VERIFY_08_TABLES_113_128.md): Tables 113-128 (16 tables) 🔸 P1-P2 NEW
- ✅ [VERIFY_09](./VERIFY_09_TABLES_129_142.md): Tables 129-142 (14 tables) 📊 P3 NEW
- ✅ [VERIFY_10](./VERIFY_10_TABLES_143_156.md): Tables 143-156 (14 tables) 🔧 P4-P5 NEW

## Verification Methodology

### Phase 1: Field Mapping Verification
For each table:
1. Read entity definition: `packages/entity/src/types/`
2. Read repository usage: `packages/surrealdb/src/repository/`
3. Compare fields: entity struct → table DEFINE FIELD
4. Verify types: Rust type → SurrealDB type
5. Check Matrix ID validation patterns

### Phase 2: Index Verification
For each table:
1. Extract all queries: `grep -rn "FROM table_name" packages/surrealdb/src/repository/`
2. Identify WHERE clauses
3. Identify JOIN patterns
4. Verify indexes exist for query patterns
5. Check for UNIQUE constraints where needed

### Phase 3: Permission Verification
For each table:
1. Verify user-scoped permissions: `$auth.user_id`
2. Verify admin permissions: `$auth.admin`
3. Verify federation permissions: `$auth.server_name`
4. Verify monitoring permissions: `$auth.monitoring`

## Common Issues to Check

### Field Mapping Issues
- [ ] Missing optional fields (`TYPE option<type>`)
- [ ] Wrong type mapping (int vs float, string vs datetime)
- [ ] Missing Matrix ID validation (@user, !room, $event, #alias)
- [ ] Missing DEFAULT values for non-nullable fields
- [ ] Wrong field names (snake_case vs camelCase)

### Index Issues
- [ ] Missing index on WHERE clause columns
- [ ] Missing composite index for multi-column WHERE
- [ ] Missing UNIQUE constraint on natural keys
- [ ] Over-indexing (indexes never used in queries)
- [ ] Wrong index order for composite indexes

### Permission Issues
- [ ] Too permissive (FOR select WHERE true on private data)
- [ ] Too restrictive (blocking legitimate access)
- [ ] Missing federation permissions
- [ ] Inconsistent patterns across related tables

## Tools and Commands

### Find entity definition
```bash
find /Volumes/samsung_t9/maxtryx/packages/entity/src/types -name "*entity*.rs"
```

### Find repository usage
```bash
cd /Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository
grep -rn "FROM table_name" . | head -20
grep -rn "WHERE.*column_name" . | head -20
```

### Analyze query patterns
```bash
# Extract WHERE clauses for a table
grep -A 5 "FROM table_name" packages/surrealdb/src/repository/*.rs | grep WHERE
```

## Priority Order
1. **P0 Critical** (Tables 097-107): Immediate production blockers
2. **P1 High** (Tables 108-117): Core functionality
3. **P2 Medium** (Tables 118-128): Important features
4. **Existing** (Tables 001-096): Legacy verification
5. **P3 Monitoring** (Tables 129-146): Observability
6. **P4-P5 Optional** (Tables 147-156): Nice-to-have features

## Success Criteria
- ✅ All entity fields mapped correctly
- ✅ All query patterns have appropriate indexes
- ✅ No missing UNIQUE constraints
- ✅ Permissions follow security best practices
- ✅ Matrix ID validation on all ID fields
- ✅ Zero index warnings in production logs

## Deliverables
For each sub-task, create a report with:
1. Issues found (with table, field, issue type)
2. Proposed fixes (specific SQL changes)
3. Priority (Critical, High, Medium, Low)
4. Verification status (Pass/Fail per table)
