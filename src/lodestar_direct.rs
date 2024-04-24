use crate::errors::ProofProviderError;
use crate::multiproof::compute_proof_descriptor;
use crate::provider::{Proof, ProofProvider};
use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use hex;
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

/// Provider that uses the Lodestar API directly.
// https://lodestar-sepolia.chainsafe.io/eth/v0/beacon/proof/state/latest
#[derive(Clone)]
pub struct LodestarDirectProvider {
    rpc: String,
}

impl LodestarDirectProvider {
    #[cfg(test)]
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
impl ProofProvider for LodestarDirectProvider {
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<Proof, ProofProviderError> {
        let descriptor = compute_proof_descriptor(&[gindex as usize]).map_err(|err| {
            ProofProviderError::InputError(format!("Failed to compute proof descriptor: {}", err))
        })?;
        println!("{:?}", descriptor);
        let format = hex::encode(descriptor);
        println!("{:?}", format);

        // https://lodestar-sepolia.chainsafe.io/eth/v0/beacon/proof/state/latest
        let req = format!(
            "{}/eth/v0/beacon/proof/state/{}?format={}",
            self.rpc, state_id, format,
        );

        let compact_proof = self.get(&req).await;
        match compact_proof {
            Ok(compact_proof) => {
                if compact_proof.len() % 32 != 0 {
                    return Err(ProofProviderError::InvalidProofError());
                }

                // Convert the body into an array of 32-byte chunks
                let mut leaves = Vec::new();
                for chunk in compact_proof.chunks(32) {
                    let mut leaf = Node::default();
                    leaf.copy_from_slice(chunk);
                    leaves.push(leaf);
                }

                // TEMP
                Ok(Proof {
                    index: gindex,
                    branch: leaves,
                    leaf: Node::default(),
                })
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lodestar_direct() {
        let lodestar =
            LodestarDirectProvider::new("https://lodestar-mainnet.chainsafe.io".to_string());

        let proof = lodestar
            .get_state_proof(
                "0x7903bc7cc62f3677c5c0e38562a122638a3627dd945d1f7992e4d32f1d4ef11e",
                42,
            )
            .await;

        println!("{:?}", proof);

        assert!(true);
    }
}
