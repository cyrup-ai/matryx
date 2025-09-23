# Matrix Server-Server API: Retrieving Events

*Federation protocol specification for single event and room state retrieval in Matrix.*

---

## Overview

Event retrieval APIs enable homeservers to fetch specific events and room state snapshots from other servers when backfilling is insufficient. This specification defines the endpoints for retrieving individual events and room state.

---

## Retrieving events

In some circumstances, a homeserver may be missing a particular event or information about the room which cannot be easily determined from backfilling. These APIs provide homeservers with the option of getting events and the state of the room at a given point in the timeline.

