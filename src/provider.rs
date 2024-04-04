use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use mockall::automock;
use serde::{Deserialize, Serialize};

use crate::errors::ProofProviderError;

#[derive(PartialEq, Deserialize, Debug, Serialize, Default, Clone)]
pub struct Proof {
    pub gindex: u64,
    pub witnesses: Vec<Node>,
    pub leaf: Node,
}

#[automock]
#[async_trait]
pub trait ProofProvider: Sync + Send + 'static {
    /// Fetches a proof from a specific g_index or a path to the beacon state of a specific block.
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError>;
    /// Fetches a proof from a specific g_index or a path to the beacon root of a specific block.
    async fn get_block_proof(
        &self,
        block_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError>;
}

#[derive(Clone)]
pub struct LoadstarProver {
    network: String,
    rpc: String,
}

/// A wrapper around the state [`prover`](https://github.com/commonprefix/state-prover)
impl LoadstarProver {
    pub fn new(network: String, rpc: String) -> Self {
        Self { network, rpc }
    }

    async fn get(&self, req: &str) -> Result<Proof, ProofProviderError> {
        let response = reqwest::get(req)
            .await
            .map_err(ProofProviderError::NetworkError)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ProofProviderError::NotFoundError(req.into()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(ProofProviderError::NetworkError)?;

        serde_json::from_slice(&bytes).map_err(ProofProviderError::SerializationError)
    }
}

#[automock]
#[async_trait]
impl ProofProvider for LoadstarProver {
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError> {
        let req = format!(
            "{}/state_proof?state_id={}&gindex={}&network={}",
            self.rpc, state_id, gindex, self.network
        );

        self.get(&req).await
    }

    async fn get_block_proof(
        &self,
        block_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError> {
        let req = format!(
            "{}/block_proof/?block_id={}&gindex={}&network={}",
            self.rpc, block_id, gindex, self.network
        );

        self.get(&req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};

    fn setup_server_and_prover() -> (Server, LoadstarProver) {
        let server = Server::run();
        let url = server.url("");
        let rpc = LoadstarProver::new("mainnet".to_string(), url.to_string());
        (server, rpc)
    }

    #[tokio::test]
    async fn test_get_state_proof() {
        let (server, prover) = setup_server_and_prover();
        let expected_response = Proof::default();
        let json_response = serde_json::to_string(&expected_response).unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains(("state_id", "state_id")))),
                request::query(url_decoded(contains(("gindex", "1")))),
            ])
            .respond_with(status_code(200).body(json_response)),
        );

        let result = prover.get_state_proof("state_id", 1).await.unwrap();
        assert_eq!(result, expected_response);
    }

    #[tokio::test]
    async fn test_get_state_proof_error() {
        let (server, prover) = setup_server_and_prover();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains(("state_id", "state_id")))),
                request::query(url_decoded(contains(("gindex", "1")))),
            ])
            .respond_with(status_code(400).body("Error")),
        );

        let result = prover.get_state_proof("state_id", 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_block_proof() {
        let (server, prover) = setup_server_and_prover();
        let json_response = serde_json::to_string(&Proof::default()).unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains(("block_id", "block_id")))),
                request::query(url_decoded(contains(("gindex", "1")))),
            ])
            .respond_with(status_code(200).body(json_response)),
        );

        let result = prover.get_block_proof("block_id", 1).await.unwrap();
        assert_eq!(result, Proof::default());
    }

    #[tokio::test]
    async fn test_get_block_proof_error() {
        let (server, prover) = setup_server_and_prover();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains(("block_id", "block_id")))),
                request::query(url_decoded(contains(("gindex", "1")))),
            ])
            .respond_with(status_code(400)),
        );

        let result = prover.get_block_proof("block_id", 1).await;

        assert!(result.is_err());
    }
}
