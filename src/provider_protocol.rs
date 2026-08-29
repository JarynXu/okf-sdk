//! Language-neutral request/response model for external Library providers.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::library::{
    KnowledgeUri, LibraryError, LibraryId, LibraryQuery, LibraryResult,
};

/// Stable provider protocol identifier.
pub const PROVIDER_PROTOCOL_V1: &str = "okf-provider/1";

/// Provider operation transported over process or remote boundaries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderOperation {
    /// Return the Library semantic catalog.
    Catalog,
    /// List direct children below a logical path.
    List,
    /// Read one canonical knowledge URI.
    Read,
    /// Execute a Library query.
    Query,
    /// Refresh provider-derived state.
    Refresh,
}

/// Portable request envelope used by external provider transports.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProviderRequest {
    /// Protocol identifier. Must be [`PROVIDER_PROTOCOL_V1`].
    pub protocol: String,
    /// Requested capability operation.
    pub operation: ProviderOperation,
    /// Active Library identity.
    pub library: LibraryId,
    /// Logical path for `list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Canonical URI for `read`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<KnowledgeUri>,
    /// Query payload for `query`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<LibraryQuery>,
}

impl ProviderRequest {
    /// Creates a catalog request.
    pub fn catalog(library: LibraryId) -> Self {
        Self::new(library, ProviderOperation::Catalog)
    }

    /// Creates a list request.
    pub fn list(library: LibraryId, path: impl Into<String>) -> Self {
        let mut request = Self::new(library, ProviderOperation::List);
        request.path = Some(path.into());
        request
    }

    /// Creates a read request.
    pub fn read(uri: KnowledgeUri) -> Self {
        let mut request = Self::new(uri.library().clone(), ProviderOperation::Read);
        request.uri = Some(uri);
        request
    }

    /// Creates a query request.
    pub fn query(library: LibraryId, query: LibraryQuery) -> Self {
        let mut request = Self::new(library, ProviderOperation::Query);
        request.query = Some(query);
        request
    }

    /// Creates a refresh request.
    pub fn refresh(library: LibraryId) -> Self {
        Self::new(library, ProviderOperation::Refresh)
    }

    fn new(library: LibraryId, operation: ProviderOperation) -> Self {
        Self {
            protocol: PROVIDER_PROTOCOL_V1.to_owned(),
            operation,
            library,
            path: None,
            uri: None,
            query: None,
        }
    }
}

/// Portable provider error returned across an external transport.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderProtocolError {
    /// Stable machine-readable error code.
    pub code: String,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Portable response envelope used by process and HTTP provider transports.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProviderResponse {
    /// Whether the provider operation succeeded.
    pub ok: bool,
    /// Successful operation payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Failure diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ProviderProtocolError>,
}

impl ProviderResponse {
    /// Creates a successful response from a serializable payload.
    pub fn success<T: Serialize>(value: &T) -> LibraryResult<Self> {
        Ok(Self {
            ok: true,
            data: Some(serde_json::to_value(value).map_err(protocol_error)?),
            error: None,
        })
    }

    /// Creates a failed response.
    pub fn failure(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ProviderProtocolError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    /// Decodes a successful payload into a typed Provider contract value.
    pub fn into_typed<T: DeserializeOwned>(self) -> LibraryResult<T> {
        if !self.ok {
            let error = self.error.unwrap_or(ProviderProtocolError {
                code: "provider-error".to_owned(),
                message: "external provider failed without a diagnostic".to_owned(),
            });
            return Err(LibraryError::Provider(format!(
                "{}: {}",
                error.code, error.message
            )));
        }
        let data = self.data.unwrap_or(Value::Null);
        serde_json::from_value(data).map_err(protocol_error)
    }
}

/// Parses and validates a provider request received by an external provider implementation.
pub fn decode_provider_request(bytes: &[u8]) -> LibraryResult<ProviderRequest> {
    let request: ProviderRequest = serde_json::from_slice(bytes).map_err(protocol_error)?;
    if request.protocol != PROVIDER_PROTOCOL_V1 {
        return Err(LibraryError::Provider(format!(
            "unsupported provider protocol '{}'",
            request.protocol
        )));
    }
    Ok(request)
}

/// Parses a provider response returned by an external transport.
pub fn decode_provider_response(bytes: &[u8]) -> LibraryResult<ProviderResponse> {
    serde_json::from_slice(bytes).map_err(protocol_error)
}

fn protocol_error(error: impl std::fmt::Display) -> LibraryError {
    LibraryError::Provider(format!("provider protocol error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_response_round_trip() {
        let id = LibraryId::parse("demo").expect("id");
        let request = ProviderRequest::query(id, LibraryQuery::new("hello").limit(3));
        let bytes = serde_json::to_vec(&request).expect("request json");
        assert_eq!(decode_provider_request(&bytes).expect("decode"), request);

        let response = ProviderResponse::success(&vec!["a", "b"]).expect("response");
        let values: Vec<String> = response.into_typed().expect("payload");
        assert_eq!(values, vec!["a", "b"]);
    }
}
