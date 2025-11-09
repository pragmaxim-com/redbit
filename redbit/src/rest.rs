use crate::{info, AppError, Deserialize, IntoResponse, Response, Serialize, Storage, StructInfo, ToSchema};
use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::routing::MethodRouter;
use axum::Router;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower_http::cors::CorsLayer;
use utoipa::openapi::extensions::Extensions;
use utoipa::openapi::schema::SchemaType;
use utoipa::openapi::{ObjectBuilder, Paths, RefOr, Schema};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

// Create our own JSON extractor by wrapping `axum::Json`. This makes it easy to override the
// rejection and provide our own which formats errors to match our application.
#[derive(FromRequest, Deserialize)]
#[from_request(via(crate::axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

#[derive(Deserialize)]
pub struct MaybeJson<T>(pub Option<T>);

impl<S, T> FromRequest<S> for MaybeJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body_bytes = match Bytes::from_request(req, state).await {
            Ok(bytes) => bytes,
            Err(_) => return Ok(MaybeJson(None)),
        };

        if body_bytes.is_empty() {
            return Ok(MaybeJson(None));
        }

        let parsed = serde_json::from_slice::<T>(&body_bytes)?;

        Ok(MaybeJson(Some(parsed)))
    }
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
    pub code: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let message = match self {
            AppError::JsonRejection(rej) => rej.body_text(),
            other                        => other.to_string(),
        };
        (status, AppJson(ErrorResponse { message, code: status.as_u16() })).into_response()
    }
}

#[derive(Clone)]
pub struct RequestState {
    pub storage: Arc<Storage>,
}

#[derive(OpenApi)]
#[openapi(info(license(name = "MIT")))]
pub struct ApiDoc;

#[derive(Debug, Clone, ToSchema, Serialize, Deserialize)]
pub enum FilterOp<T> {
    Eq(T),
    Ne(T),
    Lt(T),
    Le(T),
    Gt(T),
    Ge(T),
    In(Vec<T>),
}

impl<T: PartialOrd + PartialEq> FilterOp<T> {
    pub fn matches(&self, value: &T) -> bool {
        match self {
            FilterOp::Eq(expected) => value == expected,
            FilterOp::Ne(expected) => value != expected,
            FilterOp::Lt(expected) => value < expected,
            FilterOp::Le(expected) => value <= expected,
            FilterOp::Gt(expected) => value > expected,
            FilterOp::Ge(expected) => value >= expected,
            FilterOp::In(options) => options.contains(value),        }
    }
}


pub fn build_router(state: RequestState, extras: Option<OpenApiRouter<RequestState>>, cors: Option<CorsLayer>) -> Router<()> {
    let mut router: OpenApiRouter<RequestState> = OpenApiRouter::with_openapi(ApiDoc::openapi());
    for info in inventory::iter::<StructInfo> {
        router = router.merge((info.routes_fn)());
    }

    if let Some(extra) = extras {
        router = router.merge(extra);
    }
    let (r, openapi) = router.split_for_parts();

    let merged = r
        .merge(SwaggerUi::new("/swagger-ui").url("/apidoc/openapi.json", openapi))
        .with_state(state);
    if let Some(cors_layer) = cors {
        merged.layer(cors_layer)
    } else {
        merged
    }
}

pub async fn serve(
    state: RequestState,
    socket_addr: SocketAddr,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
    shutdown: watch::Receiver<bool>,
) {
    let router: Router<()> = build_router(state, extras, cors);
    let tcp = TcpListener::bind(socket_addr).await.unwrap();

    let mut shutdown = shutdown.clone();
    axum::serve(tcp, router)
        .with_graceful_shutdown(async move {
            if shutdown.changed().await.is_ok() {
                info!("Shutting down server...");
            }
        })
        .await
        .unwrap();
}


pub fn merge_route_sets<State: Clone + Send + Sync + 'static, I>(
    route_sets: I,
) -> OpenApiRouter<State>
where
    I: IntoIterator<
        Item = (
            Vec<(String, RefOr<Schema>)>,
            Paths,
            MethodRouter<State>,
        ),
    >,
{
    route_sets.into_iter().fold(OpenApiRouter::new(), |acc, r| {
        acc.merge(OpenApiRouter::new().routes(r))
    })
}


pub fn schema<I: IntoIterator<Item = V>, V: Into<Value>>(schema_type: SchemaType, examples: I, extensions: Option<Extensions>) -> RefOr<Schema> {
    Schema::Object(
        ObjectBuilder::new().schema_type(schema_type).examples(examples).extensions(extensions).build()
    ).into()
}
