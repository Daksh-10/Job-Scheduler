use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::{CorsLayer, Any};
use server::scheduler::{
    create_job_group,
    cron::{
        add_cron_job,
        execute_cron_jobs_for_group,
        get_cron_job_status,
        get_groups,
        get_jobs_for_group,
    },
};
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let pool = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/group", post(create_job_group))
        .route("/groups", get(get_groups))
        .route("/cron_job/{group_id}", post(add_cron_job))
        .route("/cron_jobs/{group_id}", get(get_jobs_for_group))
        .route(
            "/execute/cron_job/{group_id}",
            get(execute_cron_jobs_for_group),
        )
        .route(
            "/cron_job_status/{group_id}/{job_id}",
            get(get_cron_job_status),
        )
        .with_state(pool.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers(Any)
        );
    // .route("/ad_hoc", post(add_ad_hoc_job));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
// 545c6238-da90-4ef8-8e3c-a7aab9f3c883
