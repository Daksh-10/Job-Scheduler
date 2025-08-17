

# DAG Job Scheduler

## Project Goal
This project is a full-stack DAG-based job scheduler. It allows users to create groups and jobs (with dependencies and children), schedule jobs using cron times, and execute jobs in the correct order based on their dependencies. The system supports live job execution, status monitoring, and is designed for extensibility and modern usability.

## DAG Flow of jobs
<p align="center">
	<img src="public/diagram.png" alt="Dashboard Screenshot" width="600" />
</p>

## Architecture
<p align="center">
	<img src="public/architecture.png" alt="Architecture Diagram" width="600" />
</p>

## Features
- Create and manage job groups
- Add jobs with dependencies and children (DAG structure)
- Schedule jobs using cron expressions or ISO timestamps
- Link jobs to Downloadable Dockerfiles (S3 URLs, Drive, etc.)
- Execute jobs and monitor their status live
- Modern, user-friendly dashboard

## Tech Stack
- **Backend:** Rust (axum, sqlx, tokio), PostgreSQL
- **Frontend:** Next.js (React, TypeScript, Tailwind CSS)
- **Other:** Docker, S3 (for Dockerfile storage)

## Folder Structure
- `server/` — Rust backend API and scheduler logic
- `frontend_server/` — Next.js frontend dashboard

## Local Setup(Backend)
1. cd server
2. Set .env file for server  -> DATABASE_URL=postgres://postgres:postgres@localhost:5432/mydb
3. docker compose up
4. docker cp queries.sql server-postgres-1:/queries.sql
5. docker exec -it server-postgres-1 psql -U postgres -d mydb -f /queries.sql
6. cargo run

## Local Setup(Frontend)
1. cd frontend-server
2. Set .env file for frontend -> NEXT_PUBLIC_API_URL=http://localhost:5000
3. npm run dev


### Prerequisites
- Rust (https://rustup.rs/)
- Node.js & npm (https://nodejs.org/)
- Docker & Docker Compose (for Postgres)

### Access the dashboard
Open your browser and go to: [http://localhost:3000](http://localhost:3000)

## Notes
- The backend computes job children dynamically from dependencies.
- The frontend always fetches canonical job data from the backend.
- Jobs can be scheduled with either a cron expression or a specific time (ISO 8601).
