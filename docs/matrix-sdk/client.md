# Client

An async/await enabled Matrix client.

All of the state is held in an `Arc` so the `Client` can be cloned freely.

```rust
pub struct Client { /* private fields */ }
```

## Methods

### new
```rust
pub async fn new(homeserver_url: Url) -> Result<Self, ClientBuildError>
```
Create a new `Client` that will use the given homeserver.

### builder
```rust
pub fn builder() -> ClientBuilder
```
Create a new `ClientBuilder`.

### logged_in
```rust
pub fn logged_in(&self) -> bool
```
Is the client logged in.

### homeserver
```rust
pub fn homeserver(&self) -> Url
```
The homeserver of the client.

### user_id
```rust
pub fn user_id(&self) -> Option<&UserId>
```
Get the user id of the current owner of the client.

### device_id
```rust
pub fn device_id(&self) -> Option<&DeviceId>
```
Get the device ID that identifies the current session.

### access_token
```rust
pub fn access_token(&self) -> Option<String>
```
Get the current access token for this session, regardless of the authentication API used to log in.

### session
```rust
pub fn session(&self) -> Option<AuthSession>
```
Get the whole session info of this client.

### matrix_auth
```rust
pub fn matrix_auth(&self) -> MatrixAuth
```
Access the native Matrix authentication API with this client.

### account
```rust
pub fn account(&self) -> Account
```
Get the account of the current owner of the client.

### encryption
```rust
pub fn encryption(&self) -> Encryption
```
Get the encryption manager of the client.

### media
```rust
pub fn media(&self) -> Media
```
Get the media manager of the client.

### pusher
```rust
pub fn pusher(&self) -> Pusher
```
Get the pusher manager of the client.

### add_event_handler
```rust
pub fn add_event_handler<Ev, Ctx, H>(&self, handler: H) -> EventHandlerHandle
```
Register a handler for a specific event type.

### rooms
```rust
pub fn rooms(&self) -> Vec<Room>
```
Get all the rooms the client knows about.

### joined_rooms
```rust
pub fn joined_rooms(&self) -> Vec<Room>
```
Returns the joined rooms this client knows about.

### invited_rooms
```rust
pub fn invited_rooms(&self) -> Vec<Room>
```
Returns the invited rooms this client knows about.

### left_rooms
```rust
pub fn left_rooms(&self) -> Vec<Room>
```
Returns the left rooms this client knows about.

### get_room
```rust
pub fn get_room(&self, room_id: &RoomId) -> Option<Room>
```
Get a room with the given room id.

### create_room
```rust
pub async fn create_room(&self, request: Request) -> Result<Room>
```
Create a room with the given parameters.

### create_dm
```rust
pub async fn create_dm(&self, user_id: &UserId) -> Result<Room>
```
Create a DM room.

### sync_once
```rust
pub async fn sync_once(&self, sync_settings: SyncSettings) -> Result<SyncResponse>
```
Synchronize the client's state with the latest state on the server.

### sync
```rust
pub async fn sync(&self, sync_settings: SyncSettings) -> Result<(), Error>
```
Repeatedly synchronize the client state with the server.

### sliding_sync
```rust
pub fn sliding_sync(&self, id: impl Into<String>) -> Result<SlidingSyncBuilder>
```
Create a `SlidingSyncBuilder` tied to this client, with the given identifier.