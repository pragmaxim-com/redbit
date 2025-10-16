use std::collections::HashMap;
use std::sync::Arc;
use redb::Durability;
use crate::{AppError, FlushFuture, Storage, StartFuture, TaskResult, StopFuture};

#[derive(Copy, Clone, Debug)]
pub enum MutationType {
    Writes,
    Deletes,
}

pub trait WriteTxContext {
    fn new_write_ctx(storage: &Arc<Storage>, durability: Durability) -> redb::Result<Self, AppError> where Self: Sized;
    fn begin_writing_async(&self) -> redb::Result<Vec<StartFuture>, AppError>;
    fn stop_writing_async(self) -> redb::Result<Vec<StopFuture>, AppError>;
    fn commit_ctx_async(&self, mutation_type: MutationType) -> Result<Vec<FlushFuture>, AppError>;
    fn commit_ctx_deferred(&self, mutation_type: MutationType) -> Result<Vec<FlushFuture>, AppError>;

    fn begin_writing(&self) -> redb::Result<(), AppError> {
        let futures = self.begin_writing_async()?;
        for f in futures {
            f.wait()?;
        }
        Ok(())
    }
    fn stop_writing(self) -> redb::Result<(), AppError> where Self: Sized {
        let futures = self.stop_writing_async()?;
        for f in futures {
            f.wait()?;
        }
        Ok(())
    }
    fn begin_write_ctx(storage: &Arc<Storage>, durability: Durability) -> redb::Result<Self, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(storage, durability)?;
        ctx.begin_writing()?;
        Ok(ctx)
    }
    fn two_phase_commit(&self, mutation_type: MutationType) -> Result<HashMap<String, TaskResult>, AppError> {
        FlushFuture::dedup_tasks_keep_slowest(self.commit_ctx_async(mutation_type)?)
    }
    fn two_phase_commit_and_close(self, mutation_type: MutationType) -> Result<HashMap<String, TaskResult>, AppError> where Self: Sized {
        let tasks = self.two_phase_commit(mutation_type)?;
        self.stop_writing()?;
        Ok(tasks)
    }
}

pub trait ReadTxContext {
    fn begin_read_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
