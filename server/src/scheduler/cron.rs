use crate::scheduler::s3::run_group_jobs_with_command;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool};
use std::fs;
use uuid::Uuid;

/// Handler to execute all cron jobs for a group (stub: prints what would be scheduled)
use std::collections::{HashMap, VecDeque};

/// Represents a dependency for a cron job: parent job and required epoch.
#[derive(Debug, Deserialize, Serialize)]
pub struct DependencyEntry {
    pub parent_id: i32, // parent cron_job_id
    pub epoch: i32,     // how many attempts before last success
}

/// Request body for adding a cron job to a group.
#[derive(Debug, Deserialize)]
pub struct AddCronJobRequest {
    pub cron_job_name: String,
    pub timings: DateTime<Utc>,
    pub children_names: Option<Vec<String>>, // downstream jobs (by name)
    pub dependencies_names: Option<Vec<(String, i32)>>, // (parent_name, epoch)
    pub s3_link: Option<String>,
}

/// Response body after adding a cron job.
#[derive(Debug, Serialize)]
pub struct AddCronJobResponse {
    pub cron_job_id: i32,
    pub group_id: Uuid,
}

/// Handler to add a cron job to a group, storing DAG structure in the database.
///
/// - group_id: The group to which the job belongs.
/// - timings: Cron syntax for scheduling.
/// - children: List of downstream job IDs (edges in DAG).
/// - dependencies: List of parent jobs and their required epochs.
pub async fn add_cron_job(
    State(pool): State<PgPool>,
    Path(group_id): Path<Uuid>,
    Json(payload): Json<AddCronJobRequest>,
) -> Result<Json<AddCronJobResponse>, (StatusCode, String)> {
    // Resolve children by name to IDs
    let children: Vec<i32> = if let Some(names) = &payload.children_names {
        if names.is_empty() {
            vec![]
        } else {
            let rows = sqlx::query!(
                "SELECT cron_job_id, cron_job_name FROM cron_jobs WHERE group_id = $1 AND cron_job_name = ANY($2)",
                group_id,
                names
            )
            .fetch_all(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            rows.into_iter().map(|r| r.cron_job_id).collect()
        }
    } else {
        vec![]
    };

    // Insert the new job first (so it can be referenced by dependencies)
    let rec = sqlx::query!(
        r#"
        INSERT INTO cron_jobs (group_id, cron_job_name, timings, children, s3_link)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING cron_job_id
        "#,
        group_id,
        payload.cron_job_name,
        payload.timings,
        &children[..],
        payload.s3_link,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Insert dependencies by resolving parent names to IDs
    if let Some(deps) = &payload.dependencies_names {
        for (parent_name, epoch) in deps {
            let parent = sqlx::query!(
                "SELECT cron_job_id FROM cron_jobs WHERE group_id = $1 AND cron_job_name = $2",
                group_id,
                parent_name
            )
            .fetch_optional(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if let Some(parent) = parent {
                let _ = sqlx::query!(
                    "INSERT INTO cron_job_dependencies (cron_job_id, parent_id, epoch) VALUES ($1, $2, $3)",
                    rec.cron_job_id,
                    parent.cron_job_id,
                    epoch
                )
                .execute(&pool)
                .await;
            }
        }
    }

    Ok(Json(AddCronJobResponse {
        cron_job_id: rec.cron_job_id,
        group_id,
    }))
}

#[derive(Debug, Deserialize, Serialize, FromRow, Clone)]
pub struct CronJob {
    pub cron_job_id: i32,
    pub group_id: Uuid,
    pub cron_job_name: String,
    pub timings: DateTime<Utc>,
    pub children: Option<Vec<i32>>,
    pub s3_link: Option<String>,
}
// List all groups
pub async fn get_groups(State(pool): State<PgPool>) -> Result<Json<Vec<(Uuid, String)>>, (StatusCode, String)> {
    let rows = sqlx::query!("SELECT group_id, group_name FROM job_groups")
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows.into_iter().map(|r| (r.group_id, r.group_name)).collect()))
}

// List all jobs for a group, with children and dependencies by name
#[derive(Serialize)]
pub struct JobWithNames {
    pub cron_job_id: i32,
    pub cron_job_name: String,
    pub timings: DateTime<Utc>,
    pub children: Vec<String>,
    pub dependencies: Vec<String>,
    pub s3_link: Option<String>,
}

pub async fn get_jobs_for_group(
    State(pool): State<PgPool>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<JobWithNames>>, (StatusCode, String)> {
    // Get all jobs for the group
    let jobs: Vec<CronJob> = sqlx::query_as::<_, CronJob>(
        "SELECT * FROM cron_jobs WHERE group_id = $1"
    )
    .bind(group_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Map job_id to name
    let id_to_name: std::collections::HashMap<i32, String> = jobs.iter().map(|j| (j.cron_job_id, j.cron_job_name.clone())).collect();

    // Get all dependencies for these jobs
    let deps: Vec<CronJobDependency> = sqlx::query_as::<_, CronJobDependency>(
        "SELECT * FROM cron_job_dependencies WHERE cron_job_id = ANY($1)"
    )
    .bind(jobs.iter().map(|j| j.cron_job_id).collect::<Vec<_>>())
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Build job_id -> Vec<parent_id>
    let mut dep_map: std::collections::HashMap<i32, Vec<i32>> = std::collections::HashMap::new();
    for dep in &deps {
        dep_map.entry(dep.cron_job_id).or_default().push(dep.parent_id);
    }

    // Build output
    let jobs_with_names = jobs
        .iter()
        .map(|job| {
            let children = job
                .children
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| id_to_name.get(&id).cloned())
                .collect();
            let dependencies = dep_map
                .get(&job.cron_job_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| id_to_name.get(&id).cloned())
                .collect();
            JobWithNames {
                cron_job_id: job.cron_job_id,
                cron_job_name: job.cron_job_name.clone(),
                timings: job.timings,
                children,
                dependencies,
                s3_link: job.s3_link.clone(),
            }
        })
        .collect();
    Ok(Json(jobs_with_names))
}

#[derive(Debug, FromRow, Copy, Clone)]
pub struct CronJobDependency {
    pub cron_job_id: i32,
    pub parent_id: i32,
    pub epoch: i32,
}

/// Topological sort using Kahn's algorithm
fn topological_sort(jobs: &[CronJob]) -> Vec<i32> {
    let mut in_degree: HashMap<i32, usize> = HashMap::new();
    let mut graph: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut all_ids: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for job in jobs {
        all_ids.insert(job.cron_job_id);
        in_degree.entry(job.cron_job_id).or_insert(0);
        if let Some(children) = &job.children {
            for &child in children {
                all_ids.insert(child);
                *in_degree.entry(child).or_insert(0) += 1;
                graph.entry(job.cron_job_id).or_default().push(child);
            }
        }
    }
    // Ensure all nodes are present in in_degree
    for &id in &all_ids {
        in_degree.entry(id).or_insert(0);
    }
    let mut queue: VecDeque<i32> = in_degree
        .iter()
        .filter(|item| *item.1 == 0)
        .map(|item| *item.0)
        .collect();
    let mut order = Vec::new();
    let mut visited: std::collections::HashSet<i32> = std::collections::HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        order.push(id);
        if let Some(children) = graph.get(&id) {
            for &child in children {
                if let Some(e) = in_degree.get_mut(&child) {
                    if *e > 0 {
                        *e -= 1;
                        if *e == 0 {
                            queue.push_back(child);
                        }
                    }
                }
            }
        }
    }
    // If there are nodes not in order, append them (to avoid missing jobs)
    for &id in &all_ids {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    order
}
pub async fn execute_cron_jobs_for_group(
    State(pool): State<PgPool>,
    Path(group_id): Path<Uuid>,
) -> Result<String, String> {
    println!("{:?}", pool);
    println!("Executing cron jobs for group: {}", group_id);
    let jobs = sqlx::query_as::<_, CronJob>(r#"SELECT * FROM cron_jobs WHERE group_id = $1"#)
        .bind(group_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())).unwrap();

    println!("Got all the jobs: {:?}", jobs);

    // Build job_id -> job map
    let jobs_map: HashMap<i32, CronJob> = jobs.iter().cloned().map(|j| (j.cron_job_id, j)).collect();
    
    // Load all dependencies for jobs in this group
    let deps: Vec<CronJobDependency> = sqlx::query_as::<_, CronJobDependency>(
        "SELECT * FROM cron_job_dependencies WHERE cron_job_id = ANY($1)",
    )
    .bind(jobs.iter().map(|j| j.cron_job_id).collect::<Vec<_>>())
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut deps_map: HashMap<i32, Vec<CronJobDependency>> = HashMap::new();
    for dep in &deps {
        deps_map.entry(dep.cron_job_id).or_default().push(*dep);
    }

    // Topological sort to get execution order
    let order = topological_sort(&jobs);
    println!("{:?}", order);
    println!("Dependencies: {:?}", deps_map);
    // Build dependency map: job_id -> Vec<parent_id>
    let mut dependency_map: HashMap<i32, Vec<i32>> = HashMap::new();
    for dep in &deps {
        dependency_map.entry(dep.cron_job_id).or_default().push(dep.parent_id);
    }
    println!("Dependency map: {:?}", dependency_map);
    // Use a shared epoch state for this group
    let epoch_state = crate::scheduler::s3::EpochState::default();

    // Run all jobs in group using the new function
    let _status = run_group_jobs_with_command(
        order,
        jobs_map,
        dependency_map,
        &group_id.to_string(),
        epoch_state,
        pool.clone()
    ).await;
    println!("Jobs running");

    Ok("Executed Successfully".to_string())
}

/// Handler to get the status and log file of a job
pub async fn get_cron_job_status(
    State(pool): State<PgPool>,
    Path((group_id, job_id)): Path<(Uuid, i32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Query job_status table for this job
    let rec = sqlx::query!(
        "SELECT status, updated_at FROM job_status WHERE cron_job_id = $1 AND group_id = $2",
        job_id,
        group_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(row) = rec {
        Ok(Json(json!({
            "job_id": job_id,
            "group_id": group_id,
            "status": row.status,
            "updated_at": row.updated_at
        })))
    } else {
        Ok(Json(json!({
            "job_id": job_id,
            "group_id": group_id,
            "status": "not found"
        })))
    }
}

// b8f32ccc-1f38-4a7a-baba-852f3dcd562c
// 6debb64a-b7c3-4b60-93a4-e32c9952acab
// https://drive.google.com/file/d/1Zwtl8xVQTp8ktp1dofwszVJsmoEEadJu/view?usp=sharing