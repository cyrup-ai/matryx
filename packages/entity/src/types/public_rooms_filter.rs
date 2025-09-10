use serde::{Deserialize, Serialize};

/// PublicRoomsFilter
/// Source: spec/server/13-public-md:137-145
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsFilter {
    pub generic_search_term: Option<String>,
    pub room_types: Option<Vec<Option<String>>>,
}

impl PublicRoomsFilter {
    pub fn new(
        generic_search_term: Option<String>,
        room_types: Option<Vec<Option<String>>>,
    ) -> Self {
        Self { generic_search_term, room_types }
    }
}
