# Room Knocking Federation

This document covers the federation aspects of Matrix room knocking, allowing users to request access to rooms with knock-enabled join rules.

## Overview

Rooms can permit knocking through the join rules, and if permitted this gives users a way to request to join (be invited) to the room. Users who knock on a room where the server is already a resident of the room can just send the knock event directly without using this process, however much like [joining rooms](https://spec.matrix.org/unstable/server-server-api/#joining-rooms) the server must handshake their way into having the knock sent on its behalf.

The handshake is largely the same as the joining rooms handshake, where instead of a "joining server" there is a "knocking server", and the APIs to be called are different (`/make_knock` and `/send_knock`).

Servers can retract knocks over federation by leaving the room, as described below for rejecting invites.

