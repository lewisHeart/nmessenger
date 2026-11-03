use actix_web::{App, HttpServer, middleware, web};
use nm_servo::{
    configure,
    state::{AppState, Config},
    workers,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nm_servo=info,actix_web=info".into()),
        )
        .init();
    let config = Config::from_env().map_err(std::io::Error::other)?;
    let state = AppState::connect(&config)
        .await
        .map_err(std::io::Error::other)?;
    let _workers = workers::start(&state)
        .await
        .map_err(std::io::Error::other)?;
    let data = web::Data::new(state);
    tracing::info!(address=%config.bind_addr,"NMessenger backend ready");
    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(middleware::DefaultHeaders::new().add(("Cache-Control", "no-store")))
            .configure(configure)
    })
    .client_request_timeout(std::time::Duration::from_secs(15))
    .client_disconnect_timeout(std::time::Duration::from_secs(5))
    .max_connections(1024)
    .bind(&config.bind_addr)?
    .run()
    .await
}
