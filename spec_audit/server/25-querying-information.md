# Matrix Server-Server API: Querying for Information

*Federation protocol specification for information queries in Matrix.*

---

## Overview

Queries enable homeservers to retrieve information about resources such as users and rooms from other servers. This specification defines the federation endpoints for various information queries.

---

## Querying for information

Queries are a way to retrieve information from a homeserver about a resource, such as a user or room. The endpoints here are often called in conjunction with a request from a client on the client-server API in order to complete the call.

There are several types of queries that can be made. The generic endpoint to represent all queries is described first, followed by the more specific queries that can be made.