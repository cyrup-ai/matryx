# Matrix SDK Modules

The Matrix SDK is organized into the following modules:

- [attachment](attachment/index.html) - Types and traits for attachments.
- [authentication](authentication/index.html) - Types and functions related to authentication in Matrix.
- [config](config/index.html) - Configuration to change the behaviour of the Client.
- [debug](debug/index.html) - Helpers for creating `std::fmt::Debug` implementations.
- [deserialized_responses](deserialized_responses/index.html) - SDK-specific variations of response types from Ruma.
- [encryption](encryption/index.html) - End-to-end encryption related types.
- [event_cache](event_cache/index.html) - Abstraction layer for gathering and inferring room information.
- [event_handler](event_handler/index.html) - Types and traits related for event handlers.
- [executor](executor/index.html) - Abstraction over an executor for spawning tasks.
- [failures_cache](failures_cache/index.html) - A TTL cache for timing out repeated operations experiencing intermittent failures.
- [futures](futures/index.html) - Named futures returned from methods on types in the crate root.
- [linked_chunk](linked_chunk/index.html) - A linked chunk is the underlying data structure that holds all events.
- [live_location_share](live_location_share/index.html) - Types for live location sharing.
- [locks](locks/index.html) - Simplified locks that panic instead of returning a `Result` when poisoned.
- [media](media/index.html) - High-level media API.
- [notification_settings](notification_settings/index.html) - High-level push notification settings API.
- [pusher](pusher/index.html) - High-level pusher API.
- [ring_buffer](ring_buffer/index.html) - Ring buffer implementation.
- [room](room/index.html) - High-level room API.
- [room_directory_search](room_directory_search/index.html) - Types for searching the public room directory.
- [room_preview](room_preview/index.html) - Preview of a room.
- [ruma](ruma/index.html) - Types and traits for working with the Matrix protocol.
- [send_queue](send_queue/index.html) - A send queue facility for serializing queuing and sending of messages.
- [sleep](sleep/index.html) - Sleep utilities.
- [sliding_sync](sliding_sync/index.html) - Sliding Sync Client implementation of MSC3575 & extensions.
- [store_locks](store_locks/index.html) - Collection of small helpers that implement store-based locks.
- [sync](sync/index.html) - The SDK's representation of the result of a `/sync` request.
- [timeout](timeout/index.html) - Timeout utilities.
- [tracing_timer](tracing_timer/index.html) - Tracing timer utilities.
- [utils](utils/index.html) - Utility types and traits.