use crate::errors::ProofProviderError;
use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use ethereum_consensus::ssz::prelude::Node;
use mockall::automock;
use serde::{Deserialize, Serialize};
use ssz_rs::compact_multiproofs::verify_compact_merkle_multiproof;

// TODO Deserialize
pub trait Verify: std::fmt::Debug + PartialEq + Serialize + Default + Clone {
    fn verify(&self, root: FixedBytes<32>) -> bool;
}

#[derive(PartialEq, Deserialize, Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum BlockRootsProof {
    SingleProof {
        gindex: u64,
        witnesses: Vec<Node>,
        leaf: Node,
    },
    CompactProof {
        descriptor: Vec<u8>,
        nodes: Vec<Node>,
    },
}

impl Default for BlockRootsProof {
    fn default() -> Self {
        BlockRootsProof::SingleProof {
            gindex: 0,
            witnesses: vec![],
            leaf: Node::default(),
        }
    }
}

impl Verify for BlockRootsProof {
    fn verify(&self, root: FixedBytes<32>) -> bool {
        match self {
            BlockRootsProof::SingleProof {
                gindex,
                witnesses,
                leaf,
            } => {
                let merkle_proof = ssz_rs::proofs::Proof {
                    leaf: leaf.clone(),
                    index: *gindex as usize,
                    branch: witnesses.clone(),
                };
                match merkle_proof.verify(root) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
            BlockRootsProof::CompactProof { descriptor, nodes } => {
                match verify_compact_merkle_multiproof(nodes, descriptor, root) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
        }
    }
}

#[automock]
#[async_trait]
pub trait ProofProvider: Sync + Send + 'static {
    /// Fetches a proof from a specific g_index or a path to the beacon state of a specific block.
    async fn get_state_proof(
        &self,
        state_id: &str,
        gindex: u64,
    ) -> Result<BlockRootsProof, ProofProviderError>;
}
