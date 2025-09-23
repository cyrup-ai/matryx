# Room Invitation Federation

This document covers the federation aspects of Matrix room invitations, including both direct invites and third-party identifier invites.

## Overview

When a user on a given homeserver invites another user on the same homeserver, the homeserver may sign the membership event itself and skip the federation process. However, when a user invites another user on a different homeserver, a request to that homeserver to have the event signed and verified must be made.

Note that invites are used to indicate that knocks were accepted. As such, receiving servers should be prepared to manually link up a previous knock to an invite if the invite event does not directly reference the knock.

