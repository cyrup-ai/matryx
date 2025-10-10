pub mod by_server_name;

use serde::Deserialize;

/// Matrix media thumbnail query parameters
/// Used by both client and federation thumbnail endpoints
#[allow(dead_code)] // Used in thumbnail endpoint handlers across client and federation APIs
#[derive(Deserialize)]
pub struct ThumbnailQuery {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_method")]
    pub method: String,
    pub timeout_ms: Option<u64>,
    pub animated: Option<bool>,
}

#[allow(dead_code)] // Used by serde default
fn default_method() -> String {
    "scale".to_string()
}
