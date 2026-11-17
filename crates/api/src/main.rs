use std::{env, net::SocketAddr};

use flo_api::{StartupError, app, init_database};

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    let address: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://.data/flo.db".into());
    let pool = init_database(&database_url).await?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("flo-api listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app(pool.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("failed to register Ctrl+C handler: {error}");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                eprintln!("failed to register termination handler: {error}");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
