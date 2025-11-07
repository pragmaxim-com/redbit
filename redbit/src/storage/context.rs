use crate::{AppError, FlushFuture, StartFuture, StopFuture, Storage, TaskResult};
use redb::Durability;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use crate::storage::table_writer_api::WriteComponentRef;

pub trait WriteTxContext {
    type Defs: TxContext<WriteCtx = Self>;

    fn new_write_ctx(defs: &Self::Defs, storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized;
    fn stop_writing_async(self) -> redb::Result<Vec<StopFuture>, AppError>;

    type WriterRefs<'a>: IntoIterator<Item = &'a dyn WriteComponentRef>
    where
        Self: 'a;

    fn writer_refs(&self) -> Self::WriterRefs<'_>;

    fn begin_writing_async(&self, d: Durability) -> redb::Result<Vec<StartFuture>, AppError> {
        let mut v = Vec::new();
        for c in self.writer_refs() {
            v.extend(c.begin_async_ref(d)?);
        }
        Ok(v)
    }

    fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        let mut v = Vec::new();
        for c in self.writer_refs() {
            v.extend(c.commit_with_ref()?);
        }
        Ok(v)
    }

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
    fn begin_write_ctx(defs: &Self::Defs, storage: &Arc<Storage>, durability: Durability) -> redb::Result<Self, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(defs, storage)?;
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

impl<C: WriteTxContext + Send + 'static> WriteComponentRef for C {
    fn begin_async_ref(&self, d: Durability) -> redb::Result<Vec<StartFuture>, AppError> {
        self.begin_writing_async(d)
    }
    fn commit_with_ref(&self) -> Result<Vec<FlushFuture>, AppError> {
        self.commit_ctx_async()
    }
}

pub trait ReadTxContext {
    type Defs: TxContext<ReadCtx = Self>;

    fn begin_read_ctx(defs: &Self::Defs, storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}

pub trait TxContext {
    type ReadCtx: ReadTxContext<Defs = Self>;
    type WriteCtx: WriteTxContext<Defs = Self>;

    fn definition() -> redb::Result<Self, AppError>
    where
        Self: Sized;

    fn begin_read_ctx(&self, storage: &Arc<Storage>) -> redb::Result<Self::ReadCtx, AppError> {
        <Self::ReadCtx as ReadTxContext>::begin_read_ctx(self, storage)
    }

    fn new_write_ctx(&self, storage: &Arc<Storage>) -> redb::Result<Self::WriteCtx, AppError> {
        <Self::WriteCtx as WriteTxContext>::new_write_ctx(self, storage)
    }
    fn begin_write_ctx(&self, storage: &Arc<Storage>, durability: Durability) -> redb::Result<Self::WriteCtx, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(self, storage)?;
        ctx.begin_writing(durability)?;
        Ok(ctx)
    }
}

pub trait ToReadField {
    type ReadField;
    fn to_read_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::ReadField, AppError>;
}

pub trait ToWriteField {
    type WriteField;
    fn to_write_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::WriteField, AppError>;
}

impl<C: TxContext> ToReadField for C {
    type ReadField = C::ReadCtx;
    fn to_read_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::ReadField, AppError> {
        self.begin_read_ctx(storage)
    }
}

impl<C: TxContext> ToWriteField for C {
    type WriteField = C::WriteCtx;
    fn to_write_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::WriteField, AppError> {
        self.new_write_ctx(storage)
    }
}
