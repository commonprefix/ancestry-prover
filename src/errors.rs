use reqwest;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AncestryProverError {
    #[error("ProofProvider error: {0}")]
    ProofProviderError(#[from] ProofProviderError),
}

#[derive(Error, Debug)]
pub enum ProofProviderError {
    #[error("State or block not found: {0}")]
    NotFoundError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Invalid proof error")]
    InvalidProofError(),
}
