use serde::{Deserialize, Serialize};

/// SpaceHierarchyRequest
/// Source: spec/client/07_relationship_md:264-268
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyRequest {
    pub from: Option<String>,
    pub limit: Option<i64>,
    pub max_depth: Option<i64>,
    pub suggested_only: Option<bool>,
}

impl SpaceHierarchyRequest {
    pub fn new(
        from: Option<String>,
        limit: Option<i64>,
        max_depth: Option<i64>,
        suggested_only: Option<bool>,
    ) -> Self {
        Self { from, limit, max_depth, suggested_only }
    }
}
