use nm_servo::state::{AppState, Config};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[actix_web::test]
async fn bootstrap_and_legacy_migration_are_isolated_and_validated() {
    let (Ok(database_url), Ok(redis_url)) = (
        std::env::var("NM_TEST_DATABASE_URL"),
        std::env::var("NM_TEST_REDIS_URL"),
    ) else {
        eprintln!("Integration test skipped: set NM_TEST_DATABASE_URL and NM_TEST_REDIS_URL");
        return;
    };
    let pool = PgPoolOptions::new().connect(&database_url).await.unwrap();
    let schema = format!("test_{}", Uuid::new_v4().simple());
    // Schema is generated exclusively from a UUID, not external input.
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&pool)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&database_url).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let directory = std::env::temp_dir().join(format!("nm-bootstrap-{}", Uuid::new_v4()));
    let config = Config {
        database_url: url.into(),
        redis_url,
        bind_addr: "127.0.0.1:0".into(),
        upload_dir: directory.clone(),
        session_ttl_secs: 3600,
        user_upload_quota: 536_870_912,
    };
    let state = AppState::connect(&config).await.unwrap();
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(actix_web::web::Data::new(state.clone()))
            .configure(nm_servo::configure),
    )
    .await;
    let request = || {
        actix_web::test::TestRequest::post()
            .uri("/v1/auth/register")
            .set_json(&nm_protocol::RegisterRequest {
                email: format!("{}@nm.test", Uuid::new_v4()),
                password: "test password 123".into(),
                display_name: "Test user".into(),
                device_name: "Test device".into(),
            })
            .to_request()
    };
    let (a, b) = futures_util::join!(
        actix_web::test::call_and_read_body_json::<_, _, nm_protocol::AuthResponse>(
            &app,
            request()
        ),
        actix_web::test::call_and_read_body_json::<_, _, nm_protocol::AuthResponse>(
            &app,
            request()
        ),
    );
    let admins: Vec<Uuid> = sqlx::query_scalar("SELECT user_id FROM app_admins")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(admins.len(), 1);
    assert!(admins[0] == a.user.id || admins[0] == b.user.id);
    sqlx::query("DELETE FROM app_admins")
        .execute(&state.db)
        .await
        .unwrap();
    let _: nm_protocol::AuthResponse =
        actix_web::test::call_and_read_body_json(&app, request()).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM app_admins")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);

    let old = hex::decode("a1c922ec218d0e5acae77a6c65f4698e801e22e25c0d59255bb4084b6b2af05017670560f5aa6c8d6901d5bf32906250").unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=1")
        .bind(old)
        .execute(&state.db)
        .await
        .unwrap();
    let upgraded = AppState::connect(&config).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=1")
        .bind(vec![0u8; 48])
        .execute(&state.db)
        .await
        .unwrap();
    assert!(AppState::connect(&config).await.is_err());
    upgraded.db.close().await;
    state.db.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&pool)
        .await
        .unwrap();
    let _ = tokio::fs::remove_dir_all(directory).await;
}
