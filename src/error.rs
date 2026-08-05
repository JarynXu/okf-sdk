#[derive(Debug, thiserror::Error)]
pub enum OkfError {
    #[error("invalid OKF bundle: {0}")]
    InvalidBundle(String),
}
