use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub mod cron;
pub mod s3;

#[derive(Deserialize)]
pub struct CreateGroupRequest {
    group_name: String,
}

#[derive(Serialize)]
pub struct CreateGroupResponse {
    group_id: Uuid,
    group_name: String,
}

pub async fn create_job_group(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateGroupRequest>,
) -> impl IntoResponse {
    // Returning *both* group_id and group_name from SQL
    let result = sqlx::query!(
        r#"
        INSERT INTO job_groups (group_name)
        VALUES ($1)
        RETURNING group_id, group_name
        "#,
        payload.group_name
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(record) => {
            let response = CreateGroupResponse {
                group_id: record.group_id,
                group_name: record.group_name,
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            eprintln!("Error creating group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create group" })),
            )
                .into_response()
        }
    }
}

// 425faa70-d201-457a-bc3f-7a93b077c86a
// 2c52d2bb-e9a2-4580-82f0-d49748da5eda