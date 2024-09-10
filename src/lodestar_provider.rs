use crate::errors::ProofProviderError;
use crate::provider::{BlockRootsProof, ProofProvider};
use ::ssz_rs::compact_multiproofs::compute_proof_descriptor;
use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use hex;
use mockall::automock;
use serde::{Deserialize, Serialize};

/// Provider that uses the [Lodestar](http://lodestar.chainsafe.io/) API directly.
#[derive(Clone)]
pub struct LodestarProvider {
    rpc: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProofResponse {
    data: ProofData,
    version: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProofData {
    leaves: Vec<Node>,
    descriptor: String,
}

impl LodestarProvider {
    pub fn new(rpc: String) -> Self {
        Self { rpc }
    }

    async fn get(&self, req: &str) -> Result<Vec<u8>, ProofProviderError> {
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

        Ok(bytes.to_vec())
    }
}

#[automock]
#[async_trait]
impl ProofProvider for LodestarProvider {
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<BlockRootsProof, ProofProviderError> {
        let descriptor = compute_proof_descriptor(&[gindex as usize]).map_err(|err| {
            ProofProviderError::InputError(format!("Failed to compute proof descriptor: {}", err))
        })?;
        let format = hex::encode(&descriptor);

        // Example URL: https://lodestar-sepolia.chainsafe.io/eth/v0/beacon/proof/state/latest?format=...
        let req_url = format!(
            "{}/eth/v0/beacon/proof/state/{}?format={}",
            self.rpc, state_id, format,
        );

        let response = self.get(&req_url).await;

        match response {
            Ok(compact_proof) => {
                let proof_response: ProofResponse = serde_json::from_slice(&compact_proof)
                    .map_err(|_| ProofProviderError::InvalidProofError())?;

                Ok(BlockRootsProof::CompactProof {
                    descriptor,
                    nodes: proof_response.data.leaves,
                })
            }
            Err(e) => Err(e),
        }
    }
}
