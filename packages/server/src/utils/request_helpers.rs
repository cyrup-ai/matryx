use axum::extract::Request;

/// Extract full URI path with query parameters from Axum request
///
/// This function extracts the complete URI path including query parameters
/// for Matrix federation authentication. According to Matrix specification,
/// the URI field in X-Matrix authorization must include the full path starting
/// with `/_matrix/...`, including the `?` and any query parameters if present.
///
/// # Arguments
/// * `request` - The Axum HTTP request
///
/// # Returns
/// Full URI path with query parameters, or "/" if extraction fails
///
/// # Example
/// ```rust
/// let uri = extract_request_uri(&request);
/// // Returns: "/_matrix/federation/v1/media/download/example?timeout_ms=5000"
/// ```
pub fn extract_request_uri(request: &Request) -> &str {
    request.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Uri};

    #[test]
    fn test_extract_request_uri_with_query_params() {
        let uri: Uri = "/_matrix/federation/v1/media/download/test?timeout_ms=5000&animated=true"
            .parse()
            .unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let extracted_uri = extract_request_uri(&request);
        assert_eq!(
            extracted_uri,
            "/_matrix/federation/v1/media/download/test?timeout_ms=5000&animated=true"
        );
    }

    #[test]
    fn test_extract_request_uri_without_query_params() {
        let uri: Uri = "/_matrix/federation/v1/media/download/test".parse().unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let extracted_uri = extract_request_uri(&request);
        assert_eq!(extracted_uri, "/_matrix/federation/v1/media/download/test");
    }

    #[test]
    fn test_extract_request_uri_root_path() {
        let uri: Uri = "/".parse().unwrap();

        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let extracted_uri = extract_request_uri(&request);
        assert_eq!(extracted_uri, "/");
    }
}
