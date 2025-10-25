use crate::{AppError, FlushFuture, StartFuture, StopFuture, Storage, TaskResult};
use redb::Durability;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub trait WriteTxContext {
    fn new_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized;
    fn begin_writing_async(&self, durability: Durability) -> redb::Result<Vec<StartFuture>, AppError>;
    fn stop_writing_async(self) -> redb::Result<Vec<StopFuture>, AppError>;
    fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn commit_ctx_deferred(&self) -> Result<Vec<FlushFuture>, AppError>;

    fn begin_writing(&self, durability: Durability) -> redb::Result<(), AppError> {
        let futures = self.begin_writing_async(durability)?;
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
        let ctx = Self::new_write_ctx(storage)?;
        ctx.begin_writing(durability)?;
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
    fn two_phase_commit_or_rollback_and_close_with<F, R>(self, f: F) -> Result<HashMap<String, TaskResult>, AppError>
    where
        F: FnOnce(&Self) -> Result<R, AppError>,
        Self: Sized
    {
        let master_start = Instant::now();
        match f(&self) {
            Ok(_) => {
                let master_took = master_start.elapsed().as_millis();
                let mut tasks = self.two_phase_commit_and_close()?;
                let master_task = TaskResult::master(master_took);
                tasks.insert(master_task.name.clone(), master_task);
                Ok(tasks)
            }
            Err(err) => {
                let _ = self.stop_writing()?;
                Err(err)
            }
        }
    }
    fn two_phase_commit_with<F, R>(&self, f: F) -> Result<HashMap<String, TaskResult>, AppError>
    where
        F: FnOnce(&Self) -> Result<R, AppError>,
        Self: Sized
    {
        let master_start = Instant::now();
        match f(&self) {
            Ok(_) => {
                let master_took = master_start.elapsed().as_millis();
                let mut tasks = self.two_phase_commit()?;
                let master_task = TaskResult::master(master_took);
                tasks.insert(master_task.name.clone(), master_task);
                Ok(tasks)
            }
            Err(err) => {
                Err(err)
            }
        }
    }

}

pub trait ReadTxContext {
    fn begin_read_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
