use std::sync::Arc;
use crate::{AppError, FlushFuture, Storage};

pub trait WriteTxContext {
    fn new_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized;
    fn begin_writing(&self) -> redb::Result<(), AppError>;
    fn stop_writing(self) -> redb::Result<(), AppError> where Self: Sized;
    fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn two_phase_commit(&self) -> Result<(), AppError>;

    fn begin_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(storage)?;
        ctx.begin_writing()?;
        Ok(ctx)
    }
    fn commit_and_close_ctx(self) -> Result<(), AppError> where Self: Sized {
        let _ = self.commit_ctx_async()?.into_iter().map(|f| f.wait()).collect::<Result<Vec<_>, _>>();
        self.stop_writing()
    }
    fn two_phase_commit_and_close(self) -> Result<(), AppError> where Self: Sized {
        self.two_phase_commit()?;
        self.stop_writing()
    }
}

pub trait ReadTxContext {
    fn begin_read_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
