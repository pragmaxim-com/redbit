#[cfg(all(test, feature = "integration"))]
mod manual_ctx_lifecycle {
    use chain::settings::AppConfig;
    use demo::manual_chain::build_block_chain_auto;
    use demo::model_v1::*;
    use redbit::Durability;
    use redbit::manual_entity::ManualTestScope;
    use std::sync::Arc;

    #[tokio::test]
    async fn manual_ctx_begin_commit_stop_across_batches() {
        let target_height = 5u32;
        let (owner, storage, _path) = ManualTestScope::temp_storage(Block::db_defs()).await.expect("temp storage");
        let chain = build_block_chain_auto(Arc::clone(&storage));
        let mut ctx = chain.new_indexing_ctx().expect("ctx");

        let config: AppConfig = chain_config::load_config("config/settings", "REDBIT").expect("config");
        let mut batches: Vec<Vec<Block>> = Vec::new();
        for h in 1..=target_height {
            let block = Block {
                height: Height(h),
                header: Header { height: Height(h), hash: BlockHash([h as u8; 32]), ..Default::default() },
                ..Default::default()
            };
            batches.push(vec![block]);
        }

        for (i, batch) in batches.into_iter().enumerate() {
            let durability = if i % 2 == 0 { Durability::None } else { Durability::Immediate };
            ctx.begin_writing(durability).expect("begin");
            let res = chain.store_blocks(&ctx, batch, durability);
            assert!(res.is_ok(), "store_blocks failed: {res:?}");
            let tasks = ctx.two_phase_commit().expect("commit");
            assert!(!tasks.is_empty(), "expected commit tasks");
        }

        ctx.stop_writing().expect("stop");
        owner.assert_last_refs();
    }
}
