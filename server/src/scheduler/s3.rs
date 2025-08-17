use chrono::{DateTime, Utc};
use std::collections::{HashMap};
use std::fs;
use sqlx::PgPool;
use std::process::Command;
use std::sync::{Arc, Mutex};
use crate::scheduler::cron::CronJob;
// Helper function to build dependents map: parent_id -> Vec<child_id>
fn build_dependents_map(dependencies: &HashMap<i32, Vec<i32>>) -> HashMap<i32, Vec<i32>> {
    let mut dependents: HashMap<i32, Vec<i32>> = HashMap::new();
    for (&job_id, parents) in dependencies {
        for &parent in parents {
            dependents.entry(parent).or_default().push(job_id);
        }
    }
    dependents
}

// Helper function to spawn a job and recursively trigger dependents
fn spawn_job_and_dependents(
    job_id: i32,
    jobs: std::sync::Arc<HashMap<i32, CronJob>>,
    dependencies: std::sync::Arc<HashMap<i32, Vec<i32>>>,
    dependents: std::sync::Arc<HashMap<i32, Vec<i32>>>,
    epoch_state: EpochState,
    pool: PgPool,
) {
    let job = jobs.get(&job_id).unwrap();
    let dockerfile_path = format!("/tmp/dockerfile_{}", job_id);
    let image_name = format!("cron_job_image_{}", job_id);
    let container_name = format!("cron_job_container_{}", job_id);
    let s3_link = job.s3_link.clone().unwrap_or_default();
    let pool_clone = pool.clone();
    let epochs_clone = epoch_state.epochs.clone();
    let job_id_clone = job_id;
    let group_id_clone = job.group_id.clone();
    let dockerfile_path_clone = dockerfile_path.clone();
    let image_name_clone = image_name.clone();
    let container_name_clone = container_name.clone();
    let jobs = jobs.clone();
    let dependencies = dependencies.clone();
    let dependents = dependents.clone();
    let epoch_state_clone = epoch_state.clone();
    tokio::spawn(async move {
        // Mark as running in memory and DB
        {
            let mut epochs = epochs_clone.lock().unwrap();
            epochs.insert(job_id_clone, Epoch::Running);
        }
        let _ = sqlx::query!(
            "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,$3,NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status=$3, updated_at=NOW()",
            job_id_clone,
            group_id_clone,
            "running"
        )
        .execute(&pool_clone)
        .await;

        // Download Dockerfile
        let dockerfile_bytes = match reqwest::get(&s3_link).await {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) => bytes,
                Err(_) => {
                    {
                    let mut epochs = epochs_clone.lock().unwrap();
                    epochs.insert(job_id_clone, Epoch::Failed);}
                    let _ = sqlx::query!(
                        "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,'failed',NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status='failed', updated_at=NOW()",
                        job_id_clone,
                        group_id_clone
                    )
                    .execute(&pool_clone)
                    .await;
                    return;
                }
            },
            Err(_) => {
                {let mut epochs = epochs_clone.lock().unwrap();
                epochs.insert(job_id_clone, Epoch::Failed);}
                let _ = sqlx::query!(
                    "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,'failed',NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status='failed', updated_at=NOW()",
                    job_id_clone,
                    group_id_clone
                )
                .execute(&pool_clone)
                .await;
                return;
            }
        };
        if let Err(_) = fs::write(&dockerfile_path_clone, &dockerfile_bytes) {
            {let mut epochs = epochs_clone.lock().unwrap();
            epochs.insert(job_id_clone, Epoch::Failed);}
            let _ = sqlx::query!(
                "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,'failed',NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status='failed', updated_at=NOW()",
                job_id_clone,
                group_id_clone
            )
            .execute(&pool_clone)
            .await;
            return;
        }
        
        // build image
        let build = Command::new("docker")
            .args(&["build", "-f", &dockerfile_path_clone, "-t", &image_name_clone, "/tmp"])
            .status();

        if build.is_err() || !build.as_ref().unwrap().success() {
            {let mut epochs = epochs_clone.lock().unwrap();
            epochs.insert(job_id_clone, Epoch::Failed);}
            let _ = sqlx::query!(
                "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,'failed',NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status='failed', updated_at=NOW()",
                job_id_clone,
                group_id_clone
            )
            .execute(&pool_clone)
            .await;
            return;
        }

        // run container
        let run = Command::new("docker")
            .args(&["run", "--rm", "--name", &container_name_clone, &image_name_clone])
            .status();

        let success = run.map(|s| s.success()).unwrap_or(false);
        {
            let mut epochs = epochs_clone.lock().unwrap();
            epochs.insert(job_id_clone, if success { Epoch::Completed } else { Epoch::Failed });
        }
        let _ = sqlx::query!(
            "INSERT INTO job_status (cron_job_id, group_id, status, updated_at) VALUES ($1,$2,$3,NOW()) ON CONFLICT (cron_job_id, group_id) DO UPDATE SET status=$3, updated_at=NOW()",
            job_id_clone,
            group_id_clone,
            if success { "completed" } else { "failed" }
        )
        .execute(&pool_clone)
        .await;
        let _ = fs::remove_file(&dockerfile_path_clone);

        // After completion, try to spawn dependents if they are now eligible
        if let Some(children) = dependents.get(&job_id_clone) {
            for &child_id in children {
                // Check if all dependencies are completed
                let deps = dependencies.get(&child_id);
                
                let epochs = epoch_state_clone.epochs.lock().unwrap();
                
                let all_parents_done = deps.map(|ps| ps.iter().all(|p| epochs.get(p) == Some(&Epoch::Completed))).unwrap_or(true);
                
                
                if all_parents_done {
                    spawn_job_and_dependents(child_id, jobs.clone(), dependencies.clone(), dependents.clone(), epoch_state_clone.clone(), pool.clone());
                }
            
            }
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Epoch {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Default, Clone)]
pub struct EpochState {
    pub epochs: Arc<Mutex<HashMap<i32, Epoch>>>, // job_id -> epoch
}

pub struct JobSpec {
    pub job_id: i32,
    pub s3_link: String, // Dockerfile link
    pub timings: DateTime<Utc>,
}

pub struct JobStatusReport {
    pub running: Vec<i32>,
    pub completed: Vec<i32>,
    pub pending: Vec<i32>,
}

pub async fn run_group_jobs_with_command(
    order: Vec<i32>,
    jobs: HashMap<i32, CronJob>,
    dependencies: HashMap<i32, Vec<i32>>,
    group_id: &str,
    epoch_state: EpochState,
    pool: PgPool,
) -> Result<JobStatusReport, String> {
    // Set all jobs to Pending in memory
    {
        let mut epochs = epoch_state.epochs.lock().unwrap();
        for job_id in &order {
            epochs.insert(*job_id, Epoch::Pending);
        }
    }

    let now = Utc::now();
    let mut running = Vec::new();
    let mut pending = Vec::new();
    let mut completed = Vec::new();

    // Build dependents map
    let dependents = std::sync::Arc::new(build_dependents_map(&dependencies));
    let jobs_arc = std::sync::Arc::new(jobs);
    let dependencies_arc = std::sync::Arc::new(dependencies);

    for &job_id in &order {
        let job = jobs_arc.get(&job_id).unwrap();
        if now < job.timings {
            pending.push(job_id);
            continue;
        }
        if let Some(parents) = dependencies_arc.get(&job_id) {
            let epochs = epoch_state.epochs.lock().unwrap();
            if !parents.iter().all(|p| epochs.get(p) == Some(&Epoch::Completed)) {
                pending.push(job_id);
                continue;
            }
        }
        spawn_job_and_dependents(job_id, jobs_arc.clone(), dependencies_arc.clone(), dependents.clone(), epoch_state.clone(), pool.clone());
        running.push(job_id);
    }

    // All jobs not started are pending
    for &job_id in &order {
        let epochs = epoch_state.epochs.lock().unwrap();
        if let Some(Epoch::Completed) = epochs.get(&job_id) {
            completed.push(job_id);
        } else if !running.contains(&job_id) && !pending.contains(&job_id) {
            pending.push(job_id);
        }
    }

    Ok(JobStatusReport {
        running,
        completed,
        pending,
    })
}


// 5c5ccd2b-9e9f-4b69-9d71-c3ad2045e17a
// 7c1a091e-6cdf-4b18-b4d9-2133f2c46599
// 17676425-c4ea-4d90-a91f-b49a14fcf7ee