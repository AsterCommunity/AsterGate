//! Axum HTTP runtime component construction.

use aster_forge_runtime::{
    RuntimeComponentKind, RuntimeServiceComponent, TryRuntimeComponentWithShutdown,
};
use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub struct HttpRuntimeConfig<'a> {
    pub host: &'a str,
    pub port: u16,
}
type HttpFuture = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

pub fn http_component(
    config: HttpRuntimeConfig<'_>,
    state: Arc<crate::runtime::AppState>,
) -> TryRuntimeComponentWithShutdown<
    RuntimeServiceComponent<HttpFuture>,
    impl FnOnce(CancellationToken) -> io::Result<RuntimeServiceComponent<HttpFuture>>,
    io::Error,
> {
    aster_forge_runtime::try_runtime_component_with_shutdown(move |shutdown| {
        build_http_service_component(config, state, shutdown)
    })
}

fn build_http_service_component(
    config: HttpRuntimeConfig<'_>,
    state: Arc<crate::runtime::AppState>,
    shutdown: CancellationToken,
) -> io::Result<RuntimeServiceComponent<HttpFuture>> {
    let address: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let listener = std::net::TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(listener)?;
    tracing::info!(address = %listener.local_addr()?, "HTTP listener bound");
    let router = crate::api::router(state);
    let stop = shutdown.clone();
    let service: HttpFuture = Box::pin(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move { stop.cancelled().await })
            .await
    });
    Ok(RuntimeServiceComponent::new(
        "http",
        RuntimeComponentKind::Core,
        service,
        shutdown,
        || async {},
    ))
}
