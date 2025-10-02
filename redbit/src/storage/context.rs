use std::collections::HashMap;
use std::sync::Arc;
use crate::{AppError, FlushFuture, Storage};
use crate::storage::table_writer::TaskResult;

pub trait WriteTxContext {
    fn new_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized;
    fn begin_writing(&self) -> redb::Result<(), AppError>;
    fn stop_writing(self) -> redb::Result<(), AppError> where Self: Sized;
    fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn begin_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(storage)?;
        ctx.begin_writing()?;
        Ok(ctx)
    }
    fn two_phase_commit(&self) -> Result<HashMap<String, TaskResult>, AppError> {
        FlushFuture::dedup_tasks_keep_slowest(self.commit_ctx_async()?)
    }
    fn two_phase_commit_and_close(self) -> Result<HashMap<String, TaskResult>, AppError> where Self: Sized {
        let tasks = self.two_phase_commit()?;
        self.stop_writing()?;
        Ok(tasks)
    }
}

pub trait ReadTxContext {
    fn begin_read_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
