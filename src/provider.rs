use crate::errors::ProofProviderError;
use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use mockall::automock;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Deserialize, Debug, Serialize, Default, Clone)]
pub struct Proof {
    pub index: u64,
    pub branch: Vec<Node>,
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
}
