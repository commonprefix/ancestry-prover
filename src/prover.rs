use crate::errors::AncestryProverError;
use crate::provider::{BlockRootsProof, ProofProvider, Verify};
use alloy_primitives::FixedBytes;
use ethereum_consensus::capella::presets::mainnet::{BeaconState, SLOTS_PER_HISTORICAL_ROOT};
use ethereum_consensus::ssz::prelude::*;

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

        // calculate gindex of the target block
        let index = target_block_slot % SLOTS_PER_HISTORICAL_ROOT as u64;
        let path = &["block_roots".into(), PathElement::Index(index as usize)];
        let gindex = BeaconState::generalized_index(path).unwrap() as u64;

        let state_root_str = &recent_block_state_root.to_string();
        // get proofs from provider
        let proof = self
            .proof_provider
            .get_state_proof(state_root_str.as_str(), gindex)
            .await?;

        Ok(proof)
    }
}

pub fn verify(
    proof: BlockRootsProof,
    target_block_slot: u64,
    target_block_hash: FixedBytes<32>,
    recent_block_slot: u64,
    recent_block_state_root: FixedBytes<32>,
) -> bool {
    if recent_block_slot.saturating_sub(target_block_slot) >= (SLOTS_PER_HISTORICAL_ROOT as u64) {
        // todo:  Historical root proofs
        unimplemented!()
    }

    // TODO remove from arguments
    _ = target_block_hash;

    return proof.verify(recent_block_state_root);

    // match proof.verify(recent_block_state_root) {
    //     Ok(_) => true,
    //     Err(_) => false,
    // }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::provider;
    use crate::LodestarProvider;
    use ethereum_consensus::capella::BeaconBlockHeader;

    use super::*;
    use httptest::{matchers::*, responders::*, Expectation, Server};

    fn get_test_block_for_slot(slot: u64) -> BeaconBlockHeader {
        let filename = format!("./src/testdata/beacon_block_headers/{}.json", slot);
        let file = File::open(filename).unwrap();
        let block: BeaconBlockHeader = serde_json::from_reader(file).unwrap();
        block
    }

    #[tokio::test]
    #[should_panic(expected = "not implemented")]
    async fn it_should_panic_for_old_blocks() {
        // 7879376 - 7862720 = 16656
        let target_block = get_test_block_for_slot(7_862_720);
        let recent_block = get_test_block_for_slot(7_879_376);

        let prover_api = LodestarProvider::new("mainnet".to_string(), "".to_string());
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
            .returning(|_block_id, _gindex| Ok(BlockRootsProof::default()));
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
        let prover_api = LodestarProvider::new("mainnet".to_string(), url.to_string());
        let prover = AncestryProver::new(prover_api);

        let expected_response = BlockRootsProof::SingleProof {
            gindex: expected_gindex,
            witnesses: vec![],
            leaf: Node::default(),
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

        match proof {
            BlockRootsProof::SingleProof { gindex, .. } => {
                assert_eq!(gindex, 309908);
            }
            _ => panic!("Invalid proof type"),
        }
    }

    #[tokio::test]
    async fn it_should_return_correct_proof_with_lodestar() {
        let target_block = get_test_block_for_slot(7_877_867);
        let recent_block = get_test_block_for_slot(7_878_867);

        let file = File::open("./src/testdata/state_prover/state_proof_0x044adfafd8b8a889ea689470f630e61dddba22feb705c83eec032fac075de2ec_g308459.json").unwrap();
        let expected_proof: BlockRootsProof = serde_json::from_reader(file).unwrap();

        let mut prover_api = provider::MockProofProvider::new();
        prover_api
            .expect_get_state_proof()
            .returning(|block_id, gindex| {
                let filename = format!(
                    "./src/testdata/state_prover/state_proof_{}_g{}.json",
                    block_id, gindex
                );
                let file = File::open(filename).unwrap();
                let proof: BlockRootsProof = serde_json::from_reader(file).unwrap();
                Ok(proof)
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
                let proof: BlockRootsProof = serde_json::from_reader(file).unwrap();
                Ok(proof)
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

        let target_block_hash = target_block.hash_tree_root().unwrap();

        assert!(verify(
            proof,
            target_block.slot,
            target_block_hash,
            recent_block.slot,
            recent_block.state_root
        ));
    }

    // #[tokio::test]
    // async fn it_should_work_grandpa() {
    //     let prover_api = LodestarProvider::new(
    //         "mainnet".to_string(),
    //         "http://108.61.210.145:3000".to_string(),
    //     );
    //     let prover = AncestryProver::new(prover_api);

    //     let proof = prover
    //         .prove(
    //             8784152,
    //             8784409,
    //             "0xfe208f4f3334cf033a4fed4e1b83191e54ec98e0731a08d4a57b901eb35d4964",
    //         )
    //         .await
    //         .unwrap();

    //     assert!(proof.verify(FixedBytes::from_str(
    //         "0xfe208f4f3334cf033a4fed4e1b83191e54ec98e0731a08d4a57b901eb35d4964"
    //     )));
    // }

    // #[tokio::test]
    // async fn it_should_work_with_loadstar_direct() {
    //     let prover_api =
    //         LodestarDirectProvider::new("https://lodestar-mainnet.chainsafe.io".to_string());
    //     let prover = AncestryProver::new(prover_api);

    //     let proof = prover
    //         .prove(
    //             8784152,
    //             8784409,
    //             "0xfe208f4f3334cf033a4fed4e1b83191e54ec98e0731a08d4a57b901eb35d4964",
    //         )
    //         .await
    //         .unwrap();

    //     assert!(verify(
    //         proof,
    //         8784152,
    //         8784409,
    //         FixedBytes::from_str(
    //             "0xfe208f4f3334cf033a4fed4e1b83191e54ec98e0731a08d4a57b901eb35d4964",
    //         )
    //         .unwrap(),
    //     ));

    //     assert!(verify(
    //         proof,
    //         8784152,
    //         FixedBytes::from_str(
    //             "0x22e5a0db0a3a4104996d140ba82ab4f2f94af20fba6da3408baa0dc87744dcef"
    //         )
    //         .unwrap(),
    //         8784409,
    //         FixedBytes::from_str(
    //             "0xfe208f4f3334cf033a4fed4e1b83191e54ec98e0731a08d4a57b901eb35d4964"
    //         )
    //         .unwrap(),
    //     ));
    // }
}
