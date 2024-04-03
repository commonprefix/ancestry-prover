use ethereum_consensus::capella::{BeaconBlockHeader, BeaconState};
use ethereum_consensus::phase0::mainnet::SLOTS_PER_HISTORICAL_ROOT;
use ethereum_consensus::ssz::prelude::*;
use serde;

/// Necessary proofs to verify that a given block is an ancestor of another block.
/// In our case, it proves that the block that contains the event we are looking for, is an ancestor of the recent block that we got from the LightClientUpdate message.
// #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BlockRootsProof {
    /// Generalized index from a block_root that we care to the block_root to the state root.
    // No need to provide that, since it can be calculated on-chain.
    block_roots_index: u64,
    block_root_proof: Vec<Node>,
}

impl Default for BlockRootsProof {
    fn default() -> Self {
        Self {
            block_roots_index: 0,
            block_root_proof: vec![],
        }
    }
}

pub fn proof(
    target_block: &mut BeaconBlockHeader,
    recent_block: &mut BeaconBlockHeader,
) -> BlockRootsProof {
    if recent_block.slot.saturating_sub(target_block.slot) >= (SLOTS_PER_HISTORICAL_ROOT as u64) {
        // todo:  Historical root proofs
        unimplemented!()
    }

    println!("target {:?}", target_block.hash_tree_root());
    println!("recent {:?}", recent_block.hash_tree_root());

    // calc gindex/path
    // get proofs from loadstar/state prover

    BlockRootsProof::default()
}

pub fn verify() {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn get_test_block_for_slot(slot: u64) -> BeaconBlockHeader {
        let filename = format!("./src/testdata/beacon_block_headers/{}.json", slot);
        let file = File::open(filename).unwrap();
        let block: BeaconBlockHeader = serde_json::from_reader(file).unwrap();
        block
    }

    #[test]
    #[should_panic(expected = "not implemented")]
    fn it_should_panic_for_old_blocks() {
        // 7879376 - 7862720 = 16656
        let mut target_block = get_test_block_for_slot(7_862_720);
        let mut recent_block = get_test_block_for_slot(7_879_376);

        _ = proof(&mut target_block, &mut recent_block);
    }

    #[test]
    fn it_should_not_panic_for_recent_blocks() {
        // 7879323 - 7879316 = 7
        let mut target_block = get_test_block_for_slot(7_879_316);
        let mut recent_block = get_test_block_for_slot(7_879_323);

        _ = proof(&mut target_block, &mut recent_block);
    }
}
