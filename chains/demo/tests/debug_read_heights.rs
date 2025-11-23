use std::fs;
use std::path::PathBuf;
use demo::model_v1::{Header, Height};
use redbit::storage::init::StorageOwner;
use redbit::StructInfo;

#[tokio::test]
#[ignore]
async fn debug_print_latest_manual_sync_heights() {
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir("/tmp/redbit").expect("read_dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().into_string().unwrap_or_default();
        if !name.starts_with("manual_chain_sync_test") {
            continue;
        }
        let meta = entry.metadata().expect("metadata");
        let mtime = meta.modified().expect("modified");
        if latest.as_ref().map_or(true, |(t, _)| mtime > *t) {
            latest = Some((mtime, entry.path()));
        }
    }
    let (_, path) = latest.expect("no manual_chain_sync_test dir");
    println!("Inspecting {:?}", path);
    let mut db_defs = Vec::new();
    for info in redbit::inventory::iter::<StructInfo> {
        db_defs.extend((info.db_defs)());
    }
    let (_exists, owner, storage) = StorageOwner::init(path.clone(), db_defs, 1, false).await.expect("init");
    let ctx = Header::begin_read_ctx(&storage).expect("read ctx");
    let last = Header::last(&ctx).expect("last");
    let range = Header::range(&ctx, Height(0), Height(u32::MAX), None).expect("range");
    println!("last={:?} count={}", last, range.len());
    drop(storage);
    owner.assert_last_refs();
}
