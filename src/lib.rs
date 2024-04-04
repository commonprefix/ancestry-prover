mod errors;
mod provider;
use errors::AncestryProverError;
use ethereum_consensus::capella::presets::mainnet::{BeaconState, SLOTS_PER_HISTORICAL_ROOT};
use ethereum_consensus::capella::BeaconBlockHeader;
use ethereum_consensus::ssz::prelude::*;
use provider::ProofProvider;
use serde;

/// Necessary proofs to verify that a given block is an ancestor of another block.
/// In our case, it proves that the block that contains the event we are looking for, is an ancestor of the recent block that we got from the LightClientUpdate message.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct BlockRootsProof {
    /// Generalized index from a block_root that we care to the block_root to the state root.
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

pub struct AncestryProver<P: ProofProvider> {
    proof_provider: P,
}

impl<P: ProofProvider> AncestryProver<P> {
    pub fn new(proof_provider: P) -> Self {
        Self { proof_provider }
    }

    // This implementation generates an ancestry proof from the target block to a recent block.
    // Currently, the target block cannot be older than SLOTS_PER_HISTORICAL_ROOT (8192 blocks, ~27 hours).
    pub async fn prove(
        &self,
        target_block_slot: u64,
        recent_block_slot: u64,
        recent_block_state_root: &str,
    ) -> Result<BlockRootsProof, AncestryProverError> {
        if recent_block_slot.saturating_sub(target_block_slot) >= (SLOTS_PER_HISTORICAL_ROOT as u64)
        {
            // todo:  Historical root proofs
            unimplemented!()
        }

        let state_root_str = &recent_block_state_root.to_string();

        // calculate gindex of the target block
        let index = target_block_slot % SLOTS_PER_HISTORICAL_ROOT as u64;
        let path = &["block_roots".into(), PathElement::Index(index as usize)];
        let gindex = BeaconState::generalized_index(path).unwrap() as u64;

        // get proofs from provider
        let proof = self
            .proof_provider
            .get_state_proof(state_root_str.as_str(), gindex)
            .await?;

        Ok(BlockRootsProof {
            block_roots_index: gindex,
            block_root_proof: proof.witnesses,
        })
    }
}

pub fn verify(
    proof: BlockRootsProof,
    target_block: &mut BeaconBlockHeader,
    recent_block: &BeaconBlockHeader,
) -> bool {
    if recent_block.slot.saturating_sub(target_block.slot) >= (SLOTS_PER_HISTORICAL_ROOT as u64) {
        // todo:  Historical root proofs
        unimplemented!()
    }

    let merkle_proof = ssz_rs::proofs::Proof {
        leaf: target_block.hash_tree_root().unwrap(),
        index: proof.block_roots_index as usize,
        branch: proof.block_root_proof,
    };

    match merkle_proof.verify(recent_block.state_root) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::provider::Proof;

    use self::provider::LoadstarProver;

    use super::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use std::fs::File;

    fn get_test_block_for_slot(slot: u64) -> BeaconBlockHeader {
        let filename = format!("./src/testdata/beacon_block_headers/{}.json", slot);
        let file = File::open(filename).unwrap();
        let block: BeaconBlockHeader = serde_json::from_reader(file).unwrap();
        block
    }

    // fn setup<'a>() -> (Server, AncestryProver<LoadstarProver>) {
    //     let server = Server::run();
    //     let url = server.url("");
    //     let prover_api = LoadstarProver::new("mainnet".to_string(), url.to_string());
    //     let prover = AncestryProver::new(prover_api);
    //     (server, prover)
    // }

    #[tokio::test]
    #[should_panic(expected = "not implemented")]
    async fn it_should_panic_for_old_blocks() {
        // 7879376 - 7862720 = 16656
        let target_block = get_test_block_for_slot(7_862_720);
        let recent_block = get_test_block_for_slot(7_879_376);

        let prover_api = LoadstarProver::new("mainnet".to_string(), "".to_string());
        let prover = AncestryProver::new(prover_api);
        _ = prover
            .prove(
                target_block.slot,
                recent_block.slot,
                recent_block.state_root.to_string().as_str(),
            )
            .await;
    }

    #[tokio::test]
    async fn it_should_provide_proof_for_recent_blocks() {
        // 7879323 - 7879316 = 7
        let target_block = get_test_block_for_slot(7_879_316);
        let recent_block = get_test_block_for_slot(7_879_323);

        let mut prover_api = provider::MockProofProvider::new();
        prover_api
            .expect_get_state_proof()
            .returning(|_block_id, _gindex| Ok(Proof::default()));
        let prover = AncestryProver::new(prover_api);
        _ = prover
            .prove(
                target_block.slot,
                recent_block.slot,
                recent_block.state_root.to_string().as_str(),
            )
            .await;
    }

    #[tokio::test]
    async fn it_should_return_correct_block_roots_index() {
        let target_block = get_test_block_for_slot(7_879_316);
        let recent_block = get_test_block_for_slot(7_879_323);
        let expected_gindex = 309_908;

        let server = Server::run();
        let url = server.url("");
        let prover_api = LoadstarProver::new("mainnet".to_string(), url.to_string());
        let prover = AncestryProver::new(prover_api);

        let expected_response = Proof {
            gindex: expected_gindex,
            ..Default::default()
        };
        let json_response = serde_json::to_string(&expected_response).unwrap();

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains((
                    "state_id",
                    "0xa16855f71e99a620029e6b7c683abab542f66ee87c3dd8c72424568348f28b33"
                )))),
                request::query(url_decoded(contains(("gindex", "309908")))),
            ])
            .respond_with(status_code(200).body(json_response)),
        );

        let proof = prover
            .prove(
                target_block.slot,
                recent_block.slot,
                recent_block.state_root.to_string().as_str(),
            )
            .await
            .unwrap();
        assert_eq!(proof.block_roots_index, 309908);
    }

    #[tokio::test]
    async fn it_should_return_correct_proof() {
        let target_block = get_test_block_for_slot(7_877_867);
        let recent_block = get_test_block_for_slot(7_878_867);

        let file = File::open("./src/testdata/state_prover/state_proof_0x044adfafd8b8a889ea689470f630e61dddba22feb705c83eec032fac075de2ec_g308459.json").unwrap();
        let expected_proof: Proof = serde_json::from_reader(file).unwrap();
        let expected_proof = BlockRootsProof {
            block_roots_index: expected_proof.gindex,
            block_root_proof: expected_proof.witnesses,
        };

        let mut prover_api = provider::MockProofProvider::new();
        prover_api
            .expect_get_state_proof()
            .returning(|block_id, gindex| {
                let filename = format!(
                    "./src/testdata/state_prover/state_proof_{}_g{}.json",
                    block_id, gindex
                );
                let file = File::open(filename).unwrap();
                let res: Proof = serde_json::from_reader(file).unwrap();

                Ok(res)
            });
        let prover = AncestryProver::new(prover_api);
        let proof = prover
            .prove(
                target_block.slot,
                recent_block.slot,
                recent_block.state_root.to_string().as_str(),
            )
            .await
            .unwrap();

        assert_eq!(proof, expected_proof);
    }

    #[tokio::test]
    async fn it_should_verify_correct_proof() {
        let mut target_block = get_test_block_for_slot(7_877_867);
        let recent_block = get_test_block_for_slot(7_878_867);

        let mut prover_api = provider::MockProofProvider::new();
        prover_api
            .expect_get_state_proof()
            .returning(|block_id, gindex| {
                let filename = format!(
                    "./src/testdata/state_prover/state_proof_{}_g{}.json",
                    block_id, gindex
                );
                let file = File::open(filename).unwrap();
                let res: Proof = serde_json::from_reader(file).unwrap();

                Ok(res)
            });
        let prover = AncestryProver::new(prover_api);

        let proof = prover
            .prove(
                target_block.slot,
                recent_block.slot,
                recent_block.state_root.to_string().as_str(),
            )
            .await
            .unwrap();

        assert!(verify(proof, &mut target_block, &recent_block));
    }
}
