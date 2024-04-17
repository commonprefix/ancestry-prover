use crate::errors::ProofProviderError;
use crate::provider::{Proof, ProofProvider};
use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use mockall::automock;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Deserialize, Debug, Serialize, Default, Clone)]
pub(crate) struct LodestarProof {
    pub gindex: u64,
    pub witnesses: Vec<Node>,
    pub leaf: Node,
}

impl From<LodestarProof> for Proof {
    fn from(lodestar_proof: LodestarProof) -> Self {
        Proof {
            index: lodestar_proof.gindex,
            branch: lodestar_proof.witnesses,
            leaf: lodestar_proof.leaf,
        }
    }
}

/// Provider that uses [`state prover`](https://github.com/commonprefix/state-prover) to interact with the Lodestar API.
#[derive(Clone)]
pub struct LodestarProvider {
    network: String,
    rpc: String,
}

impl LodestarProvider {
    #[cfg(test)]
    pub fn new(network: String, rpc: String) -> Self {
        Self { network, rpc }
    }

    async fn get(&self, req: &str) -> Result<LodestarProof, ProofProviderError> {
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
impl ProofProvider for LodestarProvider {
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError> {
        let req = format!(
            "{}/state_proof?state_id={}&gindex={}&network={}",
            self.rpc, state_id, gindex, self.network
        );

        let lodestar_proof = self.get(&req).await;
        match lodestar_proof {
            Ok(lodestar_proof) => {
                let proof: Proof = lodestar_proof.into();
                Ok(proof)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};

    fn setup_server_and_prover() -> (Server, LodestarProvider) {
        let server = Server::run();
        let url = server.url("");
        let rpc = LodestarProvider::new("mainnet".to_string(), url.to_string());
        (server, rpc)
    }

    #[tokio::test]
    async fn test_get_state_proof() {
        let (server, prover) = setup_server_and_prover();
        let expected_response = LodestarProof::default();
        let json_response = serde_json::to_string(&expected_response).unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains(("state_id", "state_id")))),
                request::query(url_decoded(contains(("gindex", "1")))),
            ])
            .respond_with(status_code(200).body(json_response)),
        );

        let result = prover.get_state_proof("state_id", 1).await.unwrap();
        assert_eq!(result, expected_response.into());
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
}
