use std::borrow::Borrow;
use std::cmp::Ordering;
use std::net::SocketAddr;
use axum::Router;
use axum::routing::MethodRouter;
use redb::{Key, MultimapValue};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower_http::cors::CorsLayer;
use utoipa::openapi;
use utoipa::OpenApi;
use utoipa::openapi::extensions::Extensions;
use utoipa::openapi::schema::*;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;
use utoipa::openapi::path::Paths;
use utoipa::openapi::{RefOr, Schema};

use crate::{info, ApiDoc, AppError, RequestState, StructInfo};

pub fn schema<I: IntoIterator<Item = V>, V: Into<Value>>(schema_type: SchemaType, examples: I, extensions: Option<Extensions>) -> openapi::RefOr<Schema> {
    Schema::Object(
        ObjectBuilder::new().schema_type(schema_type).examples(examples).extensions(extensions).build()
    ).into()
}

pub fn assert_sorted<T, I>(items: &[T], label: &str, mut extract: impl FnMut(&T) -> &I)
where
    I: Key + Borrow<I::SelfType<'static>> + 'static,
{
    for w in items.windows(2) {
        let ia = extract(&w[0]);
        let ib = extract(&w[1]);
        let ord = <I as Key>::compare(
            <I as redb::Value>::as_bytes(ia.borrow()).as_ref(),
            <I as redb::Value>::as_bytes(ib.borrow()).as_ref(),
        );
        assert!(matches!(ord, Ordering::Less | Ordering::Equal), "{} must be sorted by key", label);
    }
}

pub fn collect_multimap_value<'a, V: Key + 'a>(mut mmv: MultimapValue<'a, V>) -> Result<Vec<V>, AppError>
where
        for<'b> <V as redb::Value>::SelfType<'b>: ToOwned<Owned = V>,
{
    let mut results = Vec::new();
    while let Some(item_res) = mmv.next() {
        let guard = item_res?;
        results.push(guard.value().to_owned());
    }
    Ok(results)
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

pub fn inc_le(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        if *b != 0xFF {
            *b = b.wrapping_add(1);
            return;
        }
        *b = 0;
    }
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