use super::cardano_client::{CardanoCBOR, CardanoClient};
use crate::config::CardanoConfig;
use crate::model_v1::*;
use async_stream::stream;
use async_trait::async_trait;
use chain::api::{BlockProvider, ChainError};
use chain::batcher::SyncMode;
use chain::monitor::BoxWeight;
use futures::{Stream, StreamExt};
use pallas::codec::minicbor::{Encode, Encoder};
use pallas::ledger::traverse::{MultiEraBlock, MultiEraInput, MultiEraOutput};
use pallas::network::miniprotocols::chainsync::{N2CClient, NextResponse};
use pallas::network::miniprotocols::Point;
use pallas_traverse::wellknown::GenesisValues;
use std::{pin::Pin, sync::Arc};
use crate::{AssetType, ExplorerError};

pub struct CardanoBlockProvider {
    client: CardanoClient,
    genesis: Arc<GenesisValues>,
}

impl CardanoBlockProvider {
    pub async fn new() -> Arc<Self> {
        let cardano_config = CardanoConfig::new("config/cardano").expect("Failed to load Cardano configuration");
        let client = CardanoClient::new(&cardano_config).await;
        let genesis = Arc::new(GenesisValues::mainnet());
        Arc::new(CardanoBlockProvider { client, genesis })
    }

    pub fn process_block_pure(cbor: &CardanoCBOR, genesis: &GenesisValues) -> Result<Block, ChainError> {
        let b = MultiEraBlock::decode(&cbor.0).map_err(ExplorerError::from)?;
        let header = b.header();
        let hash: [u8; 32] = *header.hash();
        let prev_h = header.previous_hash().unwrap_or(pallas::crypto::hash::Hash::new([0u8; 32]));
        let prev_hash: [u8; 32] = *prev_h;
        let height = Height(header.number() as u32);
        let mut block_weight = 0;
        let txs: Vec<pallas::ledger::traverse::MultiEraTx> = b.txs();
        let mut result_txs = Vec::with_capacity(txs.len());

        for (tx_index, tx) in txs.iter().enumerate() {
            let tx_hash: [u8; 32] = *tx.hash();
            let tx_id = BlockPointer::from_parent(height, tx_index as u16);
            let inputs = Self::process_inputs(&tx.inputs());
            let (box_weight, outputs) = Self::process_outputs(&tx.outputs(), tx_id);
            block_weight += box_weight;
            block_weight += inputs.len();
            result_txs.push(Transaction { id: tx_id, hash: TxHash(tx_hash), utxos: outputs, inputs: vec![], temp_input_refs: inputs })
        }

        let header = BlockHeader {
            height,
            timestamp: Timestamp(b.wallclock(genesis) as u32),
            slot: Slot(b.slot() as u32),
            hash: BlockHash(hash),
            prev_hash: BlockHash(prev_hash),
            weight: Weight(block_weight as u32)
        };

        Ok(Block { height: header.height, header, transactions: result_txs }) // usize
    }

    fn process_inputs(ins: &[MultiEraInput<'_>]) -> Vec<TempInputRef> {
        let mut out = Vec::with_capacity(ins.len());
        for input in ins {
            // MultiEraInput::hash returns &Hash<32> (Copy under the hood).
            let tx_hash: [u8; 32] = **input.hash();
            out.push(TempInputRef { tx_hash: TxHash(tx_hash), index: input.index() as u32 });
        }
        out
    }

    fn process_outputs(outs: &[MultiEraOutput<'_>], tx_pointer: BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        let mut asset_count = 0usize;

        let mut script_buf = Vec::with_capacity(64);
        let mut ctx = ();

        for (out_index, out) in outs.iter().enumerate() {
            let address_opt = out.address().ok().map(|a| a.to_vec());

            let script_hash_opt = out.script_ref().map(|h| {
                script_buf.clear();
                let mut enc = Encoder::new(&mut script_buf);
                h.encode(&mut enc, &mut ctx).unwrap();
                script_buf.clone() // keep a copy per output
            });

            let utxo_pointer = TransactionPointer::from_parent(tx_pointer, out_index as u16);

            let mut result_assets = Vec::with_capacity(16);

            // start your pointer index at 0
            let mut idx: u16 = 0;

            for policy_assets in out.value().assets() {
                // clone the policy‐id bytes once
                let pid_bytes: [u8; 28] = policy_assets.policy().as_ref().try_into().unwrap();
                let policy_id = PolicyId(pid_bytes);

                for asset in policy_assets.assets() {
                    let any_coin = asset.any_coin();
                    let action = match (asset.is_mint(), any_coin < 0) {
                        (true, _) => AssetType::Mint,
                        (_, true) => AssetType::Burn,
                        _ => AssetType::Transfer,
                    };

                    result_assets.push(Asset {
                        id: UtxoPointer::from_parent(utxo_pointer, idx),
                        amount: any_coin.abs() as u64,
                        name: AssetName(asset.name().to_vec()),
                        policy_id: policy_id.clone(),
                        asset_action: AssetAction(action.into()),
                    });

                    idx += 1;
                }
            }

            asset_count += result_assets.len();
            result_outs.push(Utxo {
                id: utxo_pointer,
                amount: out.value().coin().into(),
                address: Address(address_opt.unwrap_or_default()),
                script_hash: ScriptHash(script_hash_opt.unwrap_or_default()),
                assets: result_assets,
            })
        }
        (asset_count + result_outs.len(), result_outs)
    }
}

#[async_trait]
impl BlockProvider<CardanoCBOR, Block> for CardanoBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&CardanoCBOR) -> Result<Block, ChainError> + Send + Sync> {
        let genesis = Arc::clone(&self.genesis);
        // capture Arc<GenesisValues>; closure itself is zero-alloc per call
        Arc::new(move |cbor: &CardanoCBOR| {
            CardanoBlockProvider::process_block_pure(cbor, &genesis)
        })
    }

    fn get_processed_block(&self, _h: BlockHash) -> Result<Option<Block>, ChainError> {
        Ok(None) // pallas chain sync rolls back, this method is not needed
    }

    async fn get_chain_tip(&self) -> Result<BlockHeader, ChainError> {
        let best_block = self.client.get_best_block().await?;
        let genesis = Arc::clone(&self.genesis);
        let best_header = Self::process_block_pure(&best_block, &genesis)?;
        Ok(best_header.header)
    }

    fn stream(&self, _remote_chain_tip: BlockHeader, last_persisted_header: Option<BlockHeader>, _mode: SyncMode) -> Pin<Box<dyn Stream<Item = CardanoCBOR> + Send + 'static>> {
        let node_client = Arc::clone(&self.client.node_client);
        let last_point = last_persisted_header.as_ref().map_or(Point::Origin, |h| Point::new(h.slot.0 as u64, h.hash.0.to_vec()));

        stream! {
            let mut guard = node_client.lock().await;
            let cs: &mut N2CClient = guard.chainsync();
            // find_intersect mutates the client state
            let (intersected, tip) = cs.find_intersect(vec![last_point]).await.expect("chainsync find_intersect failed");
            info !("Cardano intersection point: {:?} and tip {:?}", intersected, tip);

            loop {
                match cs.request_next().await.expect("chainsync request_next failed") {
                    NextResponse::RollForward(bytes, _new_tip) => {
                        yield CardanoCBOR(bytes.0);
                    }
                    NextResponse::RollBackward(_point, new_tip) => {
                        info !("Cardano roll backward to: {:?} ", new_tip);
                        continue;
                    }
                    NextResponse::Await => {
                        if let Err(e) = cs.send_done().await {
                            error!("chainsync send_done failed (ignoring): {e}");
                        };
                        break
                    },
                }
            }
        }.boxed()
    }
}
