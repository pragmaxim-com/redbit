use axum::response::IntoResponse;
use redbit::axum_streams::StreamBodyAs;
use futures::Stream;
use redbit::{utoipa, AppError};
use std::time::Duration;
use tokio_stream::StreamExt;

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct NumberChunk {
    pub value: u64,
}
#[utoipa::path(
    get,
    path = "/events",
    responses(
        (status = 200, description = "SSE stream of heartbeat events", content_type = "text/event-stream",
         body = NumberChunk)
    )
)]
pub async fn test_json_nl_stream() -> impl IntoResponse {
    StreamBodyAs::json_nl_with_errors(number_stream(Duration::from_secs(1)))
}

fn number_stream(duration: Duration) -> impl Stream<Item = Result<NumberChunk, AppError>> {
    futures::stream::iter(0u64..).throttle(duration).map(|n| Ok(NumberChunk { value: n }) )
}
