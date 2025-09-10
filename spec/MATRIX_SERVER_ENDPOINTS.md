# Matrix Server REST API Endpoints

## Summary
- Client-Server API Endpoints: 162
- Server-Server Federation API Endpoints: 36
- **Total Unique Endpoints: 198**

## Client-Server API Endpoints

### Endpoint
- `POST /_matrix/client/v3/endpoint`

### Login
- `POST /_matrix/client/v3/login`

### Logout
- `POST /_matrix/client/v3/logout`
- `POST /_matrix/client/v3/logout/all`

### Media
- `POST /_matrix/media/v1/create`
- `POST /_matrix/media/v3/upload`
- `PUT /_matrix/media/v3/upload/{serverName}/{mediaId}`

### Other
- `DELETE /\_matrix/client/v3/devices/{deviceId}`
- `DELETE /\_matrix/client/v3/directory/room/{roomAlias}`
- `DELETE /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}`
- `DELETE /\_matrix/client/v3/room\_keys/keys`
- `DELETE /\_matrix/client/v3/room\_keys/keys/{roomId}`
- `DELETE /\_matrix/client/v3/room\_keys/keys/{roomId}/{sessionId}`
- `DELETE /\_matrix/client/v3/room\_keys/version/{version}`
- `DELETE /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}`
- `GET /.well-known/matrix/client`
- `GET /.well-known/matrix/support`
- `GET /\_matrix/client/v1/media/config`
- `GET /\_matrix/client/v1/media/download/{serverName}/{mediaId}`
- `GET /\_matrix/client/v1/media/download/{serverName}/{mediaId}/{fileName}`
- `GET /\_matrix/client/v1/media/preview\_url`
- `GET /\_matrix/client/v1/media/thumbnail/{serverName}/{mediaId}`
- `GET /\_matrix/client/v1/room\_summary/{roomIdOrAlias}`
- `GET /\_matrix/client/v1/rooms/{roomId}/hierarchy`
- `GET /\_matrix/client/v1/rooms/{roomId}/relations/{eventId}`
- `GET /\_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}`
- `GET /\_matrix/client/v1/rooms/{roomId}/relations/{eventId}/{relType}/{eventType}`
- `GET /\_matrix/client/v1/rooms/{roomId}/threads`
- `GET /\_matrix/client/v3/account/3pid`
- `GET /\_matrix/client/v3/account/whoami`
- `GET /\_matrix/client/v3/admin/whois/{userId}`
- `GET /\_matrix/client/v3/capabilities`
- `GET /\_matrix/client/v3/devices`
- `GET /\_matrix/client/v3/devices/{deviceId}`
- `GET /\_matrix/client/v3/directory/list/room/{roomId}`
- `GET /\_matrix/client/v3/directory/room/{roomAlias}`
- `GET /\_matrix/client/v3/events`
- `GET /\_matrix/client/v3/events/{eventId}`
- `GET /\_matrix/client/v3/initialSync`
- `GET /\_matrix/client/v3/joined\_rooms`
- `GET /\_matrix/client/v3/keys/changes`
- `GET /\_matrix/client/v3/login`
- `GET /\_matrix/client/v3/login/sso/redirect`
- `GET /\_matrix/client/v3/login/sso/redirect/{idpId}`
- `GET /\_matrix/client/v3/notifications`
- `GET /\_matrix/client/v3/presence/{userId}/status`
- `GET /\_matrix/client/v3/publicRooms`
- `GET /\_matrix/client/v3/pushers`
- `GET /\_matrix/client/v3/pushrules/`
- `GET /\_matrix/client/v3/pushrules/global/`
- `GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}`
- `GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions`
- `GET /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/enabled`
- `GET /\_matrix/client/v3/room\_keys/keys`
- `GET /\_matrix/client/v3/room\_keys/keys/{roomId}`
- `GET /\_matrix/client/v3/room\_keys/keys/{roomId}/{sessionId}`
- `GET /\_matrix/client/v3/room\_keys/version`
- `GET /\_matrix/client/v3/room\_keys/version/{version}`
- `GET /\_matrix/client/v3/rooms/{roomId}/aliases`
- `GET /\_matrix/client/v3/rooms/{roomId}/context/{eventId}`
- `GET /\_matrix/client/v3/rooms/{roomId}/event/{eventId}`
- `GET /\_matrix/client/v3/rooms/{roomId}/initialSync`
- `GET /\_matrix/client/v3/rooms/{roomId}/joined\_members`
- `GET /\_matrix/client/v3/rooms/{roomId}/members`
- `GET /\_matrix/client/v3/rooms/{roomId}/messages`
- `GET /\_matrix/client/v3/rooms/{roomId}/state`
- `GET /\_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}`
- `GET /\_matrix/client/v3/sync`
- `GET /\_matrix/client/v3/thirdparty/location`
- `GET /\_matrix/client/v3/thirdparty/location/{protocol}`
- `GET /\_matrix/client/v3/thirdparty/protocol/{protocol}`
- `GET /\_matrix/client/v3/thirdparty/protocols`
- `GET /\_matrix/client/v3/thirdparty/user`
- `GET /\_matrix/client/v3/thirdparty/user/{protocol}`
- `GET /\_matrix/client/v3/user/{userId}/account\_data/{type}`
- `GET /\_matrix/client/v3/user/{userId}/filter/{filterId}`
- `GET /\_matrix/client/v3/user/{userId}/rooms/{roomId}/account\_data/{type}`
- `GET /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags`
- `GET /\_matrix/client/v3/voip/turnServer`
- `GET /\_matrix/client/versions`
- `GET /\_matrix/media/v3/config`
- `GET /\_matrix/media/v3/download/{serverName}/{mediaId}`
- `GET /\_matrix/media/v3/download/{serverName}/{mediaId}/{fileName}`
- `GET /\_matrix/media/v3/preview\_url`
- `GET /\_matrix/media/v3/thumbnail/{serverName}/{mediaId}`
- `GET /_matrix/app/v1/thirdparty/protocol/{protocol}`
- `GET /_matrix/static/client/login/`
- `GET /_matrix/static/client/login/?device_id=GHTYAJCE`
- `POST /\_matrix/client/v1/login/get\_token`
- `POST /\_matrix/client/v3/account/3pid`
- `POST /\_matrix/client/v3/account/3pid/add`
- `POST /\_matrix/client/v3/account/3pid/bind`
- `POST /\_matrix/client/v3/account/3pid/delete`
- `POST /\_matrix/client/v3/account/3pid/email/requestToken`
- `POST /\_matrix/client/v3/account/3pid/msisdn/requestToken`
- `POST /\_matrix/client/v3/account/3pid/unbind`
- `POST /\_matrix/client/v3/account/deactivate`
- `POST /\_matrix/client/v3/account/password`
- `POST /\_matrix/client/v3/account/password/email/requestToken`
- `POST /\_matrix/client/v3/account/password/msisdn/requestToken`
- `POST /\_matrix/client/v3/createRoom`
- `POST /\_matrix/client/v3/delete\_devices`
- `POST /\_matrix/client/v3/join/{roomIdOrAlias}`
- `POST /\_matrix/client/v3/keys/claim`
- `POST /\_matrix/client/v3/keys/device\_signing/upload`
- `POST /\_matrix/client/v3/keys/query`
- `POST /\_matrix/client/v3/keys/signatures/upload`
- `POST /\_matrix/client/v3/keys/upload`
- `POST /\_matrix/client/v3/knock/{roomIdOrAlias}`
- `POST /\_matrix/client/v3/login`
- `POST /\_matrix/client/v3/logout`
- `POST /\_matrix/client/v3/logout/all`
- `POST /\_matrix/client/v3/publicRooms`
- `POST /\_matrix/client/v3/pushers/set`
- `POST /\_matrix/client/v3/refresh`
- `POST /\_matrix/client/v3/room\_keys/version`
- `POST /\_matrix/client/v3/rooms/{roomId}/ban`
- `POST /\_matrix/client/v3/rooms/{roomId}/forget`
- `POST /\_matrix/client/v3/rooms/{roomId}/invite`
- `POST /\_matrix/client/v3/rooms/{roomId}/join`
- `POST /\_matrix/client/v3/rooms/{roomId}/kick`
- `POST /\_matrix/client/v3/rooms/{roomId}/leave`
- `POST /\_matrix/client/v3/rooms/{roomId}/read\_markers`
- `POST /\_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`
- `POST /\_matrix/client/v3/rooms/{roomId}/report`
- `POST /\_matrix/client/v3/rooms/{roomId}/report/{eventId}`
- `POST /\_matrix/client/v3/rooms/{roomId}/unban`
- `POST /\_matrix/client/v3/rooms/{roomId}/upgrade`
- `POST /\_matrix/client/v3/search`
- `POST /\_matrix/client/v3/user/{userId}/filter`
- `POST /\_matrix/client/v3/user/{userId}/openid/request\_token`
- `POST /\_matrix/client/v3/user\_directory/search`
- `POST /\_matrix/client/v3/users/{userId}/report`
- `POST /\_matrix/media/v1/create`
- `POST /\_matrix/media/v3/upload`
- `PUT /\_matrix/client/v3/devices/{deviceId}`
- `PUT /\_matrix/client/v3/directory/list/room/{roomId}`
- `PUT /\_matrix/client/v3/directory/room/{roomAlias}`
- `PUT /\_matrix/client/v3/presence/{userId}/status`
- `PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}`
- `PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/actions`
- `PUT /\_matrix/client/v3/pushrules/global/{kind}/{ruleId}/enabled`
- `PUT /\_matrix/client/v3/room\_keys/keys`
- `PUT /\_matrix/client/v3/room\_keys/keys/{roomId}`
- `PUT /\_matrix/client/v3/room\_keys/keys/{roomId}/{sessionId}`
- `PUT /\_matrix/client/v3/room\_keys/version/{version}`
- `PUT /\_matrix/client/v3/rooms/{roomId}/redact/{eventId}/{txnId}`
- `PUT /\_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`
- `PUT /\_matrix/client/v3/rooms/{roomId}/state/{eventType}/{stateKey}`
- `PUT /\_matrix/client/v3/rooms/{roomId}/typing/{userId}`
- `PUT /\_matrix/client/v3/sendToDevice/{eventType}/{txnId}`
- `PUT /\_matrix/client/v3/user/{userId}/account\_data/{type}`
- `PUT /\_matrix/client/v3/user/{userId}/rooms/{roomId}/account\_data/{type}`
- `PUT /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}`
- `PUT /\_matrix/media/v3/upload/{serverName}/{mediaId}`

### Profile
- `PUT /_matrix/client/v3/profile/{userId}/{keyName}`

### Publicrooms
- `POST /_matrix/client/v3/publicRooms`

### Room_Keys
- `GET /_matrix/client/v3/room_keys/version/{version}`
- `POST /_matrix/client/v3/room_keys/version`

### Rooms
- `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`

### Sendtodevice
- `PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}`

### Thirdparty
- `GET /_matrix/client/v3/thirdparty/protocols`

## Server-Server Federation API Endpoints

### Other
- `GET /.well-known/matrix/server`
- `GET /\_matrix/federation/v1/backfill/{roomId}`
- `GET /\_matrix/federation/v1/event/{eventId}`
- `GET /\_matrix/federation/v1/event\_auth/{roomId}/{eventId}`
- `GET /\_matrix/federation/v1/hierarchy/{roomId}`
- `GET /\_matrix/federation/v1/make\_join/{roomId}/{userId}`
- `GET /\_matrix/federation/v1/make\_knock/{roomId}/{userId}`
- `GET /\_matrix/federation/v1/make\_leave/{roomId}/{userId}`
- `GET /\_matrix/federation/v1/media/download/{mediaId}`
- `GET /\_matrix/federation/v1/media/thumbnail/{mediaId}`
- `GET /\_matrix/federation/v1/openid/userinfo`
- `GET /\_matrix/federation/v1/publicRooms`
- `GET /\_matrix/federation/v1/query/directory`
- `GET /\_matrix/federation/v1/query/{queryType}`
- `GET /\_matrix/federation/v1/state/{roomId}`
- `GET /\_matrix/federation/v1/state\_ids/{roomId}`
- `GET /\_matrix/federation/v1/user/devices/{userId}`
- `GET /\_matrix/federation/v1/version`
- `GET /\_matrix/key/v2/query/{serverName}`
- `GET /\_matrix/key/v2/server`
- `GET /_matrix/client/v3/thirdparty/protocols`
- `POST /\_matrix/federation/v1/get\_missing\_events/{roomId}`
- `POST /\_matrix/federation/v1/publicRooms`
- `POST /\_matrix/federation/v1/user/keys/claim`
- `POST /\_matrix/federation/v1/user/keys/query`
- `POST /\_matrix/key/v2/query`
- `PUT /\_matrix/federation/v1/3pid/onbind`
- `PUT /\_matrix/federation/v1/exchange\_third\_party\_invite/{roomId}`
- `PUT /\_matrix/federation/v1/invite/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v1/send/{txnId}`
- `PUT /\_matrix/federation/v1/send\_join/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v1/send\_knock/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v1/send\_leave/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v2/invite/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v2/send\_join/{roomId}/{eventId}`
- `PUT /\_matrix/federation/v2/send\_leave/{roomId}/{eventId}`
