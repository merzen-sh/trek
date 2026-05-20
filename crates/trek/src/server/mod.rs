mod health;
mod proxy;
mod router;

use hyper::Request;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_service::Service;

pub struct Server {
    port: u16,
}

impl Server {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let app = router::create();
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;

        #[cfg(all(not(feature = "swagger"), debug_assertions))]
        tracing::warn!("!!! swagger disabled. Enable swagger feature for debug builds");

        tracing::info!("listening on {addr}");

        loop {
            let (stream, _peer) = listener.accept().await?;
            let app = app.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let svc = service_fn(move |req: Request<Incoming>| {
                    let mut app = app.clone();
                    async move { app.call(req).await }
                });

                if let Err(err) = http1::Builder::new().serve_connection(io, svc).await {
                    tracing::warn!("connection error: {err}");
                }
            });
        }
    }
}

pub fn run_server(port: u16) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    rt.block_on(Server::new(port).run())?;
    Ok(())
}
