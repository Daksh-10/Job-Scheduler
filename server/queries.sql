-- Table to track job execution status
DROP Table IF exists job_status CASCADE;
-- job_status table to track the status of cron jobs within a group
CREATE TABLE IF NOT EXISTS job_status (
    cron_job_id INT NOT NULL REFERENCES cron_jobs(cron_job_id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES job_groups(group_id) ON DELETE CASCADE,
    status TEXT NOT NULL, -- e.g. 'pending', 'running', 'completed', 'failed'
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
ALTER TABLE job_status
ADD CONSTRAINT job_status_cron_group_unique UNIQUE (cron_job_id, group_id);
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

Drop table if exists job_groups CASCADE;
-- job_groups table (already exists, using UUID for group_id)
CREATE TABLE IF NOT EXISTS job_groups (
    group_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_name TEXT NOT NULL
);

Drop table if exists cron_jobs CASCADE;

CREATE TABLE IF NOT EXISTS cron_jobs (
    cron_job_id SERIAL PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES job_groups(group_id) ON DELETE CASCADE,
    cron_job_name TEXT NOT NULL,
    timings TIMESTAMPTZ NOT NULL, -- timestamp for scheduled execution
    children INT[] DEFAULT '{}', -- array of cron_job_ids
    s3_link TEXT -- S3 file link for the node
);
ALTER TABLE cron_jobs ADD CONSTRAINT unique_group_job_name UNIQUE (group_id, cron_job_name);

Drop table if exists cron_job_dependencies CASCADE;
-- Table for dependencies (DAG edges) with epoch tracking
CREATE TABLE IF NOT EXISTS cron_job_dependencies (
    cron_job_id INT NOT NULL REFERENCES cron_jobs(cron_job_id) ON DELETE CASCADE,
    parent_id INT NOT NULL REFERENCES cron_jobs(cron_job_id) ON DELETE CASCADE,
    epoch INT NOT NULL DEFAULT 0,
    PRIMARY KEY (cron_job_id, parent_id)
);

-- Helpful indexes
CREATE INDEX IF NOT EXISTS idx_cron_jobs_group_id ON cron_jobs(group_id);
CREATE INDEX IF NOT EXISTS idx_cron_job_dependencies_job_id ON cron_job_dependencies(cron_job_id);
CREATE INDEX IF NOT EXISTS idx_cron_job_dependencies_parent_id ON cron_job_dependencies(parent_id);

-- cdb739d4-fb64-4a1e-b593-66be02f4db99
-- d1442ace-7c37-461f-895e-6b48a0c3d4b4