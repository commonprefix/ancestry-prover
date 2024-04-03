use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::*;
use mockall::automock;
use serde::{Deserialize, Serialize};

use crate::errors::ProverAPIError;

#[derive(PartialEq, Deserialize, Debug, Serialize, Default, Clone)]
pub struct Proof {
    pub gindex: u64,
    pub witnesses: Vec<Node>,
    pub leaf: Node,
}

/// A wrapper around the state [`prover`](https://github.com/commonprefix/state-prover)
#[automock]
#[async_trait]
pub trait ProverAPI: Sync + Send + 'static {
    /// Fetches a proof from a specific g_index or a path to the beacon root of a specific block.
    async fn get_block_proof(&self, block_id: &str, gindex: u64) -> Result<Proof, ProverAPIError>;
}

#[derive(Clone)]
pub struct LoadstarProver {
    network: String,
    rpc: String,
}
