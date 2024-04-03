mod errors;
mod provider;
use async_trait::async_trait;
use errors::AncestryProverError;
use ethereum_consensus::capella::presets::mainnet::BeaconState;
use ethereum_consensus::capella::presets::mainnet::SLOTS_PER_HISTORICAL_ROOT;
use ethereum_consensus::capella::BeaconBlockHeader;
use ethereum_consensus::ssz::prelude::*;
use provider::ProofProvider;
use serde;

/// Necessary proofs to verify that a given block is an ancestor of another block.
/// In our case, it proves that the block that contains the event we are looking for, is an ancestor of the recent block that we got from the LightClientUpdate message.
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

pub struct AncestryProver<P: ProofProvider> {
    prover_api: P,
}

impl<P: ProofProvider> AncestryProver<P> {
    pub fn new(prover_api: P) -> Self {
        Self { prover_api }
    }

    // This implementation generates an ancestry proof from the target block to a recent block.
    // Currently, the target block cannot be older than SLOTS_PER_HISTORICAL_ROOT (8192 blocks, ~27 hours).
    pub async fn proof(
        &self,
        target_block: &mut BeaconBlockHeader,
        recent_block: &mut BeaconBlockHeader,
    ) -> Result<BlockRootsProof, AncestryProverError> {
        if recent_block.slot.saturating_sub(target_block.slot) >= (SLOTS_PER_HISTORICAL_ROOT as u64)
        {
            // todo:  Historical root proofs
            unimplemented!()
        }

        let recent_block_hash = recent_block.hash_tree_root().unwrap();
        let hash_str = serde_json::to_string(&recent_block_hash).unwrap();

        // println!("{}", hash_str);

        let index = target_block.slot % SLOTS_PER_HISTORICAL_ROOT as u64;
        let path = &["block_roots".into(), PathElement::Index(index as usize)];
        let gindex = BeaconState::generalized_index(path).unwrap() as u64;

        // get proofs from loadstar/state prover
        let proof = self
            .prover_api
            .get_block_proof(hash_str.as_str(), gindex)
            .await?;

        Ok(BlockRootsProof {
            block_roots_index: gindex,
            block_root_proof: proof.witnesses,
        })
    }

    pub fn verify(&self) {
        todo!()
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

    fn setup<'a>() -> (Server, AncestryProver<LoadstarProver>) {
        let server = Server::run();
        let url = server.url("");
        let prover_api = LoadstarProver::new("mainnet".to_string(), url.to_string());
        let prover = AncestryProver::new(prover_api);
        (server, prover)
    }

    #[tokio::test]
    #[should_panic(expected = "not implemented")]
    async fn it_should_panic_for_old_blocks() {
        // 7879376 - 7862720 = 16656
        let mut target_block = get_test_block_for_slot(7_862_720);
        let mut recent_block = get_test_block_for_slot(7_879_376);

        let prover_api = LoadstarProver::new("mainnet".to_string(), "".to_string());
        let prover = AncestryProver::new(prover_api);
        _ = prover.proof(&mut target_block, &mut recent_block).await;
    }

    #[tokio::test]
    async fn it_should_not_panic_for_recent_blocks() {
        // 7879323 - 7879316 = 7
        let mut target_block = get_test_block_for_slot(7_879_316);
        let mut recent_block = get_test_block_for_slot(7_879_323);

        let mut prover_api = provider::MockProofProvider::new();
        prover_api
            .expect_get_state_proof()
            .returning(|block_id, gindex| {
                let filename = format!("state_proof_{}_g{}.json", block_id, gindex);
                let filename = format!("./src/testdata/state_prover/{}", filename);
                let file = File::open(filename).unwrap();
                let res: Proof = serde_json::from_reader(file).unwrap();

                Ok(res)
            });
        let prover = AncestryProver::new(prover_api);
        _ = prover.proof(&mut target_block, &mut recent_block).await;
    }

    #[tokio::test]
    async fn it_should_return_correct_block_roots_index() {
        let mut target_block = get_test_block_for_slot(7_879_316);
        let mut recent_block = get_test_block_for_slot(7_879_323);

        // let (server, prover_api, prover) = setup();
        let server = Server::run();
        let url = server.url("");
        let prover_api = LoadstarProver::new("mainnet".to_string(), url.to_string());
        let prover = AncestryProver::new(prover_api);
        let expected_response = Proof::default();
        let json_response = serde_json::to_string(&expected_response).unwrap();

        println!("{:?}", json_response);

        server.expect(
            Expectation::matching(all_of![
                request::query(url_decoded(contains((
                    "state_id",
                    "0x8187c32a9a82f6666b5d70ad9d0a3a63fa35f3c8e42ce3fc9546d59a3c9abbd1"
                )))),
                request::query(url_decoded(contains(("gindex", "309908")))),
            ])
            .respond_with(status_code(200).body(json_response)),
        );
        // server.expect(
        //     Expectation::matching(request::path("/foo"))
        //         .times(1..)
        //         .respond_with(status_code(200).body(json_response)),
        // );

        let proof = prover
            .proof(&mut target_block, &mut recent_block)
            .await
            .unwrap();
        assert_eq!(proof.block_roots_index, 309908);
    }
}
