# STUB_8: Inaccessible Children in Room Hierarchy

## OBJECTIVE

Implement proper tracking and reporting of inaccessible children in room hierarchy responses. Currently, the inaccessible_children field is always empty, making the API response incomplete per Matrix specification and preventing clients from understanding the full room hierarchy structure.

## SEVERITY

**FUNCTIONALITY GAP**

## LOCATION

- **Primary File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/hierarchy/by_room_id.rs:217`

## CURRENT STUB CODE

```rust
inaccessible_children: vec![], // Empty for now - could be populated with rooms that exist but are not accessible
```

## SUBTASKS

### SUBTASK1: Understand Matrix Room Hierarchy Specification

**What:** Research Matrix space/hierarchy API requirements  
**Where:** Matrix Client-Server and Server-Server specifications  
**Why:** Need to understand what inaccessible_children represents  

**Requirements:**
- Download Matrix spec on spaces and room hierarchy
- Save to `/Volumes/samsung_t9/maxtryx/docs/matrix-room-hierarchy.md`
- Understand the difference between children vs inaccessible_children
- Document what makes a child "inaccessible"
- Understand client use cases for this information

### SUBTASK2: Review Hierarchy Query Logic

**What:** Understand the current hierarchy implementation  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/hierarchy/by_room_id.rs`  
**Why:** Need to understand what's already implemented  

**Requirements:**
- Read the complete file
- Understand how children are currently discovered
- Identify where access checking happens
- Document the data structures being used
- Understand the response format

### SUBTASK3: Define Inaccessible Child Detection

**What:** Determine when a child room is "inaccessible"  
**Where:** Design decision  
**Why:** Need clear criteria for classification  

**Requirements:**
- Document what makes a child inaccessible:
  - Room exists but user lacks join permission?
  - Room exists but is invite-only/private?
  - Room referenced but doesn't exist?
  - Federation error preventing access?
- Align with Matrix spec definition
- Consider security implications of revealing existence

### SUBTASK4: Implement Inaccessible Children Collection

**What:** Collect inaccessible children during hierarchy traversal  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/hierarchy/by_room_id.rs:217`  
**Why:** Populate the field with actual data  

**Requirements:**
- During hierarchy traversal, track rooms that fail access checks
- Store room IDs (or minimal info) for inaccessible children
- Don't expose sensitive information (room names, etc.) for private rooms
- Consider performance (don't slow down hierarchy query significantly)
- Remove stub comment

### SUBTASK5: Format Response Correctly

**What:** Include inaccessible_children in API response  
**Where:** Same file, response building  
**Why:** Clients need this information  

**Requirements:**
- Build inaccessible_children array in response
- Follow Matrix specification format
- Consider what information to include per child:
  - Room ID (required)
  - Room type (if known)
  - Any other spec-required fields
- Ensure proper serialization

## DEFINITION OF DONE

- [ ] Matrix spec requirements documented
- [ ] Inaccessible child criteria defined
- [ ] Collection logic implemented
- [ ] Response includes inaccessible_children
- [ ] Stub comment removed
- [ ] No sensitive information leaked for private rooms
- [ ] Code compiles without errors

## RESEARCH NOTES

### Matrix Hierarchy Response Format

**Expected structure:**
```json
{
  "room": { ... },
  "children": [ ... ],
  "inaccessible_children": [
    "!room_id:server.com"
  ]
}
```

### Use Cases

**Why clients need this:**
- Show grayed-out rooms in UI
- Indicate locked/invite-only spaces
- Display hierarchy structure even when some rooms are private
- Request invites to inaccessible rooms

### Security Considerations

**Privacy concerns:**
- Revealing room existence might be sensitive
- Balance between UX and privacy
- Follow Matrix spec guidance on what to expose
- Consider server configuration options

### Performance

**Considerations:**
- Don't make extra database queries just for inaccessible rooms
- Collect during normal hierarchy traversal
- Set reasonable limits on depth/breadth

## RELATED FUNCTIONALITY

This is part of the Matrix Spaces feature (MSC1772). Review:
- How children relationships are stored (m.space.child events)
- Access control for rooms
- Federation hierarchy queries

## NO TESTS OR BENCHMARKS

Do NOT write unit tests, integration tests, or benchmarks as part of this task. The testing team will handle test coverage separately.

---

## MATRIX SPECIFICATION REQUIREMENTS

### Space Hierarchy - Inaccessible Children

From `/spec/server/13-public-rooms.md` (Spaces section):

**GET /\_matrix/federation/v1/hierarchy/{roomId}**

Added in v1.2 - Federation version of the Client-Server hierarchy endpoint.

> Unlike the Client-Server API version, this endpoint does not paginate. Instead, all the space-room's children the requesting server could feasibly peek/join are returned. The requesting server is responsible for filtering the results further down for the user's request.

**Response Structure:**

```json
{
  "room": { /* SpaceHierarchyParentRoom */ },
  "children": [ /* Array of accessible rooms */ ],
  "inaccessible_children": [
    "!secret:example.org"
  ]
}
```

**Field Requirements:**

**`inaccessible_children` (array of strings, REQUIRED):**

> The list of room IDs the requesting server doesn't have a viable way to peek/join. Rooms which the responding server cannot provide details on will be outright excluded from the response instead.
>
> **Assuming both the requesting and responding server are well behaved, the requesting server should consider these room IDs as not accessible from anywhere. They should not be re-requested.**

**`children` (array, REQUIRED):**

> A summary of the space's children. Rooms which the requesting server cannot peek/join will be excluded.

**Behavior Specification:**

1. **Accessible Children**: Included in `children` array with full details
2. **Inaccessible Children**: Room IDs only in `inaccessible_children` array
3. **Unknown/Invalid Children**: Completely excluded from response

**What Makes a Child Inaccessible:**

Based on specification intent:
- Room exists but requesting server cannot peek
- Room exists but requesting server cannot join
- Private/invite-only rooms without access
- Rooms where join rules prevent access

**Security Considerations:**

- Only expose room IDs, not sensitive metadata
- Don't reveal room names or topics for private rooms
- Balance between UX (showing structure) and privacy

**m.space.child Events:**

> Only `m.space.child` state events of the room are considered. Invalid child rooms and parent events are not covered by this endpoint.

**Caching:**

> Responses to this endpoint should be cached for a period of time.

**Use Case:**

Clients use inaccessible_children to:
- Display grayed-out rooms in space hierarchy UI
- Show locked/private spaces in structure
- Indicate where access requests could be made
- Understand full space structure even with limited permissions
