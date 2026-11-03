mod activities;
mod admin;
mod announcements;
mod assets;
pub mod auth;
mod authz;
pub mod badges;
mod bots;
mod contacts;
mod conversations;
mod encryption;
pub mod error;
mod files;
mod identity;
mod messages;
mod models;
mod outbox;
mod presence;
mod privacy;
mod realtime;
mod releases;
pub mod state;
mod validation;
pub mod workers;
mod ws;

use actix_web::{
    HttpResponse, ResponseError,
    dev::ServiceResponse,
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    web,
};
use serde_json::json;
use state::AppState;

async fn health(state: web::Data<AppState>) -> HttpResponse {
    let mut cache = state.health.lock().await;
    if let Some(status) = cache.as_ref()
        && status.checked_at.elapsed() < std::time::Duration::from_secs(2)
    {
        return health_response(status.postgres, status.redis);
    }
    let (pg, redis) = tokio::join!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db)
        ),
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            redis::cmd("PING")
                .query_async::<String>(&mut state.redis.clone())
                .await
        })
    );
    let postgres = matches!(pg, Ok(Ok(1)));
    let redis = matches!(redis,Ok(Ok(pong)) if pong=="PONG");
    *cache = Some(state::HealthStatus {
        checked_at: std::time::Instant::now(),
        postgres,
        redis,
    });
    health_response(postgres, redis)
}

fn health_response(postgres: bool, redis: bool) -> HttpResponse {
    let status = if postgres && redis {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::SERVICE_UNAVAILABLE
    };
    HttpResponse::build(status)
        .json(json!({"ready":postgres && redis,"postgres":postgres,"redis":redis}))
}

async fn missing() -> HttpResponse {
    error::Error::missing().error_response()
}

fn json_errors<B>(response: ServiceResponse<B>) -> actix_web::Result<ErrorHandlerResponse<B>> {
    if response
        .headers()
        .get(actix_web::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("application/json"))
    {
        return Ok(ErrorHandlerResponse::Response(
            response.map_into_left_body(),
        ));
    }
    let status = response.status();
    let code = match status.as_u16() {
        400 => "invalid_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        413 => "payload_too_large",
        429 => "rate_limited",
        _ => "request_failed",
    };
    let (request, _) = response.into_parts();
    let body = HttpResponse::build(status).json(nm_protocol::ApiError {
        code: code.into(),
        message: status.canonical_reason().unwrap_or("Request failed").into(),
    });
    Ok(ErrorHandlerResponse::Response(
        ServiceResponse::new(request, body).map_into_right_body(),
    ))
}

/// Mount on an App with web::Data<AppState>. Shared by the binary and real HTTP tests.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(
        web::JsonConfig::default()
            .limit(512 * 1024)
            .error_handler(|error, _| {
                use actix_web::error::JsonPayloadError;
                let error = match error {
                    JsonPayloadError::Overflow { .. }
                    | JsonPayloadError::OverflowKnownLength { .. } => error::Error::too_large(),
                    _ => error::Error::bad("JSON inválido ou campos obrigatórios ausentes"),
                };
                error.into()
            }),
    )
    .app_data(
        web::PathConfig::default()
            .error_handler(|_, _| error::Error::bad("Identificador inválido").into()),
    )
    .app_data(
        web::QueryConfig::default()
            .error_handler(|_, _| error::Error::bad("Parâmetros inválidos").into()),
    )
    .app_data(web::PayloadConfig::new(
        nm_protocol::MAX_UPLOAD_BYTES + 64 * 1024,
    ))
    .route("/health", web::get().to(health))
    .service(
        web::scope("/v1")
            .wrap(
                actix_web::middleware::DefaultHeaders::new()
                    .add(("Cache-Control", "no-store"))
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "no-referrer"))
                    .add((
                        "Content-Security-Policy",
                        "default-src 'none'; frame-ancestors 'none'; sandbox",
                    )),
            )
            .wrap(
                ErrorHandlers::new()
                    .default_handler_client(json_errors)
                    .default_handler_server(json_errors),
            )
            .route("/health", web::get().to(health))
            .route("/events", web::get().to(ws::upgrade))
            .configure(auth::routes)
            .configure(admin::routes)
            .configure(bots::routes)
            .configure(encryption::routes)
            .configure(identity::routes)
            .configure(contacts::routes)
            .configure(conversations::routes)
            .configure(messages::routes)
            .configure(files::routes)
            .configure(assets::routes)
            .configure(activities::routes)
            .configure(announcements::routes)
            .configure(releases::routes)
            .default_service(web::route().to(missing)),
    )
    .default_service(web::route().to(missing));
}
