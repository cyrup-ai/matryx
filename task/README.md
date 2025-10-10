# Production Readiness Tasks

This directory contains individual task files for resolving non-production code patterns identified in the MaxTryX codebase.

## Task Overview

### CRITICAL Priority

1. **TPBRIDGE_1** - Fix Third-Party Bridge Protocol Field Types (1-2 weeks)
   - Incomplete field type handling in bridge protocol metadata
   - Data loss risk if not fixed
   - Blocks IRC, Slack, Discord bridges

2. **STUB_A** - Implement Client Authentication APIs (1 week)
   - Replace login/register endpoint stubs
   - Foundation for all client operations
   - Blocks all client functionality

### HIGH Priority

3. **PAGEFIX_1** - Implement Pagination Token Validation (3-5 days)
   - Missing token validation in message pagination
   - Security issue: invalid tokens can crash server
   - Blocks proper room history navigation

4. **DEVMSG_1** - Implement To-Device Message Subscription (1 week)
   - Critical for E2EE functionality
   - Blocks key verification and device messaging
   - Required for cross-device encryption

5. **CPUCACHE_1** - Implement CPU Metrics Caching (2-3 days)
   - Performance issue under load
   - Thread pool exhaustion risk
   - Monitoring overhead

### MEDIUM Priority

6. **ERRFIX_1** - Improve Error Handling (2-3 days)
   - Generic fallback errors hide real issues
   - Poor debuggability in production
   - Error information loss

7. **MEDIAFIX_1** - Add Test Helper for Expired Media (1 day)
   - Incomplete test infrastructure
   - Expiration logic not properly tested

## Task Execution Order

**Recommended order for maximum impact**:

1. TPBRIDGE_1 (unblocks bridge functionality)
2. PAGEFIX_1 (security issue)
3. DEVMSG_1 (critical for E2EE)
4. STUB_A (enables basic client operations)
5. CPUCACHE_1 (performance improvement)
6. ERRFIX_1 (debugging improvements)
7. MEDIAFIX_1 (test quality)

## Task File Format

Each task file contains:

- **OBJECTIVE**: Clear goal statement
- **PROBLEM DESCRIPTION**: What's wrong and why
- **RESEARCH NOTES**: Background and specifications
- **SUBTASKS**: Numbered, actionable steps
- **CONSTRAINTS**: No tests, no benchmarks
- **DEPENDENCIES**: External resources needed
- **DEFINITION OF DONE**: Checklist of completion criteria
- **FILES TO MODIFY**: Exact file locations

## Important Constraints

⚠️ **ALL TASKS**:
- **NO TESTS**: Do not write unit tests, integration tests, or test fixtures
- **NO BENCHMARKS**: Do not write benchmark code
- **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories

## Estimated Total Effort

- **Critical**: 2-3 weeks (TPBRIDGE_1 + STUB_A)
- **High**: 2-3 weeks (PAGEFIX_1 + DEVMSG_1 + CPUCACHE_1)
- **Medium**: 1 week (ERRFIX_1 + MEDIAFIX_1)

**Total**: 5-7 weeks for all tasks

## Tasks NOT Created (Intentional)

The following items from the original analysis were intentionally excluded:

1. **Test code improvements** - Test team responsibility
2. **Documentation clarity** - Low priority, cosmetic only
3. **Legacy code comments** - Low priority, informational only
4. **Additional client stubs** - STUB_A covers authentication foundation, remaining stubs can be separate project

## Matrix Specification Resources

All tasks reference the Matrix specification:
- Repository: https://github.com/matrix-org/matrix-spec
- Clone locally to: `./tmp/matrix-spec/`
- Key sections:
  - Client-Server API
  - Application Service API
  - Federation API

## Getting Started

1. Choose a task based on priority and dependencies
2. Read the entire task file before starting
3. Clone Matrix spec if needed
4. Follow subtasks in order
5. Use definition of done checklist
6. No tests, no benchmarks - focus on functionality

## Questions or Issues

If a task is unclear, blocked, or needs revision:
1. Check the RESEARCH NOTES section
2. Review Matrix specification
3. Check existing similar code in codebase
4. Document any discovered issues for task refinement
