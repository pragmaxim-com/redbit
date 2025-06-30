use crate::{Hash, Transaction};
use redbit::utoipa;
use redbit::{AppError, AppJson, RequestState};

#[utoipa::path(
    get,
    path = "/foo_txs/{hash}",
    params(("hash" = Hash, Path, description = "Secondary index column")),
    responses((status = OK, body = Vec<Transaction>)),
    tag = "Transaction"
)]
#[axum::debug_handler]
pub async fn foo_txs(
    axum::extract::State(_state): axum::extract::State<RequestState>,
    axum::extract::Path(_hash): axum::extract::Path<Hash>,
) -> Result<AppJson<Vec<Transaction>>, AppError> {
    Ok(AppJson(vec![]))
}