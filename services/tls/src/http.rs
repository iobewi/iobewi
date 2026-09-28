//! HTTPS provisioning API for the portable TLS identity and trust service.
//!
//! The application attaches these handlers to its shared HTTP router and
//! supplies authorization plus the platform-backed durable TLS service.
//! The caller must expose the routes only on an authenticated TLS listener.

use alloc::string::String;
use iobewi_http::json::{json_error, json_ok, JsonResponse};
use iobewi_http::response::StatusCode;
use serde::Deserialize;

use crate::SaveCertError;

pub const CERT_PATH: &str = "/v1alpha1/tls/cert";
pub const CA_PATH: &str = "/v1alpha1/tls/ca";

#[derive(Deserialize)]
struct CertBody {
    cert_pem: String,
    key_pem: String,
}

#[derive(Deserialize)]
struct CaBody {
    ca_pem: String,
}

/// The application supplies its existing Bearer policy; the platform
/// supplies certificate validation and persistent storage through the TLS
/// service. No hardware or application configuration type enters this API.
#[allow(async_fn_in_trait)]
pub trait ProvisioningBackend {
    async fn authorize(&self, token: &str) -> bool;
    async fn save_cert(&self, cert_pem: &str, key_pem: &str) -> Result<(), SaveCertError>;
    async fn save_ca(&self, ca_pem: &str) -> Result<(), SaveCertError>;
}

pub async fn cert_response<B: ProvisioningBackend>(backend: &B, bearer: &str, body: &str) -> JsonResponse {
    if !backend.authorize(bearer).await {
        return json_error(StatusCode::UNAUTHORIZED, "{\"error\":\"unauthorized\"}");
    }
    let Ok(request) = serde_json::from_str::<CertBody>(body) else {
        return json_error(StatusCode::BAD_REQUEST, "{\"error\":\"missing_cert_or_key\"}");
    };
    match backend.save_cert(&request.cert_pem, &request.key_pem).await {
        Ok(()) => json_ok(String::from("{\"status\":\"saved\"}")),
        Err(SaveCertError::Invalid) => json_error(StatusCode::BAD_REQUEST, "{\"error\":\"invalid_certificate\"}"),
        Err(SaveCertError::Mismatch) => json_error(StatusCode::BAD_REQUEST, "{\"error\":\"cert_key_mismatch\"}"),
        Err(SaveCertError::Storage) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"error\":\"nvs_write_failed\"}"),
    }
}

pub async fn ca_response<B: ProvisioningBackend>(backend: &B, bearer: &str, body: &str) -> JsonResponse {
    if !backend.authorize(bearer).await {
        return json_error(StatusCode::UNAUTHORIZED, "{\"error\":\"unauthorized\"}");
    }
    let Ok(request) = serde_json::from_str::<CaBody>(body) else {
        return json_error(StatusCode::BAD_REQUEST, "{\"error\":\"missing_ca\"}");
    };
    match backend.save_ca(&request.ca_pem).await {
        Ok(()) => json_ok(String::from("{\"status\":\"saved\"}")),
        Err(SaveCertError::Storage) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"error\":\"nvs_write_failed\"}"),
        Err(SaveCertError::Invalid | SaveCertError::Mismatch) =>
            json_error(StatusCode::BAD_REQUEST, "{\"error\":\"invalid_certificate\"}"),
    }
}
