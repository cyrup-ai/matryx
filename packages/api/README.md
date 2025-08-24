# cyrup-matrix

A wrapper around the matrix-sdk providing synchronous interfaces with hidden async complexity.

This crate follows the Cyrup development conventions:
- No async_trait or async fn in traits
- No Box<dyn Future> or Pin<Box<dyn Future>> in public interfaces
- Synchronous interfaces with .await() called internally
- Async complexity hidden behind channels and task spawn
- Domain-specific return types that are easy to use

## Example

```rust
// Instead of this:
async fn get_sync_token(&self) -> Result<Option<String>, Error>;

// We provide this:
fn get_sync_token(&self) -> MatrixFuture<Option<String>>;
```

Where `MatrixFuture<T>` is a convenient wrapper that can be directly awaited by the consumer
while hiding all the async complexity.