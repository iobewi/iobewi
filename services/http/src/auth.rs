//! HTTP credential parsing; authorization policy belongs to each service.

/// Read a Bearer credential without applying any authorization policy.
pub fn bearer_token(value: Option<&str>) -> &str {
    value.and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("")
}
