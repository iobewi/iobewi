//! Shared JSON response and Bearer header mechanics. The service using them
//! decides who is authorized and which status/body its API returns.

use alloc::string::String;
use picoserve::response::{ContentBody, ContentHeaders, Response, StatusCode};

pub type JsonResponse = Response<ContentHeaders, ContentBody<String>>;

pub fn json_ok(body: String) -> JsonResponse {
    Response::ok(body).with_content_type("application/json")
}

pub fn json_error(status: StatusCode, body: &str) -> JsonResponse {
    Response::new(status, String::from(body)).with_content_type("application/json")
}

/// Read a Bearer credential without applying any authorization policy.
pub fn bearer_token(value: Option<&str>) -> &str {
    value.and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("")
}
