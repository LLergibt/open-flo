use std::{
    env,
    io::{self, Read},
    net::SocketAddr,
};

use flo_api::{AuthConfig, StartupError, app_with_config, bootstrap_admin, init_database};

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if !arguments.is_empty() {
        return command_line(&arguments).await;
    }
    let address: SocketAddr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://.data/flo.db".into());
    let pool = init_database(&database_url).await?;
    let config = AuthConfig::from_env()?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("flo-api listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app_with_config(pool.clone(), config))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    pool.close().await;
    Ok(())
}

async fn command_line(arguments: &[String]) -> Result<(), StartupError> {
    const USAGE: &str = "Usage: flo-api create-admin --username USER --display-name NAME [--password-stdin]\nWithout --password-stdin the password is read from the terminal without echo.";
    if arguments == ["--help"] || arguments == ["-h"] {
        println!("{USAGE}");
        return Ok(());
    }
    if arguments.first().map(String::as_str) != Some("create-admin") {
        return Err(USAGE.into());
    }
    let mut username = None;
    let mut display_name = None;
    let mut password_stdin = false;
    let mut iterator = arguments.iter().skip(1);
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--username" if username.is_none() => username = iterator.next().cloned(),
            "--display-name" if display_name.is_none() => display_name = iterator.next().cloned(),
            "--password-stdin" if !password_stdin => password_stdin = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(USAGE.into()),
        }
    }
    let username = username.ok_or(USAGE)?;
    let display_name = display_name.ok_or(USAGE)?;
    let password = if password_stdin {
        let mut value = String::new();
        io::stdin().lock().take(515).read_to_string(&mut value)?;
        if value.ends_with('\n') {
            value.pop();
            if value.ends_with('\r') {
                value.pop();
            }
        }
        if value.len() > 512 {
            return Err("password input is too long".into());
        }
        value
    } else {
        let password = rpassword::prompt_password("Password (12–128 characters): ")?;
        let confirm = rpassword::prompt_password("Confirm password: ")?;
        if password != confirm {
            return Err("passwords do not match".into());
        }
        password
    };
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://.data/flo.db".into());
    let pool = init_database(&database_url).await?;
    let user = bootstrap_admin(&pool, &username, &display_name, password).await?;
    println!("Administrator created: {} ({})", user.username, user.id);
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
