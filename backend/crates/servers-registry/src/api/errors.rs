//! Erros no formato da especificação: `{"errors":[{"code","message"}]}`.

use axum::{
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use netmonitor_core::shared::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    BlobUnknown,
    BlobUploadInvalid,
    BlobUploadUnknown,
    DigestInvalid,
    ManifestBlobUnknown,
    ManifestInvalid,
    ManifestUnknown,
    NameInvalid,
    NameUnknown,
    SizeInvalid,
    Unauthorized,
    Denied,
    Unsupported,
    TooManyRequests,
    Unknown,
}

impl Code {
    const fn as_str(self) -> &'static str {
        match self {
            Self::BlobUnknown => "BLOB_UNKNOWN",
            Self::BlobUploadInvalid => "BLOB_UPLOAD_INVALID",
            Self::BlobUploadUnknown => "BLOB_UPLOAD_UNKNOWN",
            Self::DigestInvalid => "DIGEST_INVALID",
            Self::ManifestBlobUnknown => "MANIFEST_BLOB_UNKNOWN",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::ManifestUnknown => "MANIFEST_UNKNOWN",
            Self::NameInvalid => "NAME_INVALID",
            Self::NameUnknown => "NAME_UNKNOWN",
            Self::SizeInvalid => "SIZE_INVALID",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Denied => "DENIED",
            Self::Unsupported => "UNSUPPORTED",
            Self::TooManyRequests => "TOOMANYREQUESTS",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug)]
pub struct OciError {
    status: StatusCode,
    code: Code,
    message: String,
    headers: Vec<(HeaderName, String)>,
}

impl OciError {
    #[must_use]
    pub fn new(status: StatusCode, code: Code, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            headers: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_header(mut self, name: HeaderName, value: String) -> Self {
        self.headers.push((name, value));
        self
    }

    /// Erro interno (banco, disco): registrado, e a mensagem genérica.
    #[must_use]
    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::warn!(%error, "erro no registry");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Code::Unknown,
            "erro interno do registry",
        )
    }

    /// Um `AppError` do catálogo no vocabulário da especificação.
    #[must_use]
    pub fn from_app(error: AppError) -> Self {
        match error {
            AppError::Validation(message) if message.starts_with("MANIFEST_BLOB_UNKNOWN") => {
                Self::new(StatusCode::BAD_REQUEST, Code::ManifestBlobUnknown, message)
            }
            AppError::Validation(message) => {
                Self::new(StatusCode::BAD_REQUEST, Code::ManifestInvalid, message)
            }
            AppError::Conflict(message) => Self::new(StatusCode::CONFLICT, Code::Denied, message),
            AppError::BusinessRule(message) => {
                Self::new(StatusCode::INSUFFICIENT_STORAGE, Code::Denied, message)
            }
            AppError::NotFound(message) => {
                Self::new(StatusCode::NOT_FOUND, Code::NameUnknown, message)
            }
            other => Self::internal(other),
        }
    }
}

impl IntoResponse for OciError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(json!({"errors": [{"code": self.code.as_str(), "message": self.message}]})),
        )
            .into_response();
        for (name, value) in self.headers {
            if let Ok(value) = HeaderValue::from_str(&value) {
                response.headers_mut().insert(name, value);
            }
        }
        response
    }
}
