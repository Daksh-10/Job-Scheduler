"use client";
import React, { useEffect, useState } from "react";
import { GroupList, Group } from "../components/GroupList";
import { GroupForm } from "../components/GroupForm";
import { CronJobForm } from "../components/CronJobForm";

interface CronJob {
  cron_job_id: string;
  cron_job_name: string;
  timings: string;
  s3_link?: string;
  children: string[];
  dependencies: string[];
}


interface JobStatus {
  job_id: string;
  group_id: string;
  status: string;
  updated_at?: string;
}

export default function HomePage() {
  const [groups, setGroups] = useState<Group[]>([]);
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null);
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [jobStatuses, setJobStatuses] = useState<Record<string, JobStatus>>({});
  const [executing, setExecuting] = useState(false);

  // Fetch groups
  useEffect(() => {
    fetch(`${process.env.NEXT_PUBLIC_API_URL}/groups`)
      .then((r) => r.json())
      .then((data) => {
        setGroups(data.map(([group_id, group_name]: [string, string]) => ({ group_id, group_name })));
      });
  }, []);

  // Fetch jobs for selected group (always get canonical children/dependencies from backend)
  useEffect(() => {
    if (selectedGroup) {
      const fetchJobs = () => {
        fetch(`${process.env.NEXT_PUBLIC_API_URL}/cron_jobs/${selectedGroup.group_id}`)
          .then((r) => r.json())
          .then((data) => setJobs(data));
      };
      fetchJobs();
      // Poll every 2 seconds for live updates
      const interval = setInterval(fetchJobs, 2000);
      return () => clearInterval(interval);
    } else {
      setJobs([]);
    }
  }, [selectedGroup]);

  // Poll job statuses every 2 seconds
  useEffect(() => {
    let interval: NodeJS.Timeout;
    if (selectedGroup && jobs.length > 0) {
      const fetchStatuses = () => {
        jobs.forEach((job) => {
          fetch(`${process.env.NEXT_PUBLIC_API_URL}/cron_job_status/${selectedGroup.group_id}/${job.cron_job_id}`)
            .then((r) => r.json())
            .then((status) => {
              setJobStatuses((prev) => ({ ...prev, [job.cron_job_id]: status }));
            });
        });
      };
      fetchStatuses();
      interval = setInterval(fetchStatuses, 2000);
    } else {
      setJobStatuses({});
    }
    return () => clearInterval(interval);
  }, [selectedGroup, jobs]);

  const handleCreateGroup = async (groupName: string) => {
    const res = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/group`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ group_name: groupName }),
    });
    if (res.ok) {
      fetch(`${process.env.NEXT_PUBLIC_API_URL}/groups`)
        .then((r) => r.json())
        .then((data) => {
          setGroups(data.map(([group_id, group_name]: [string, string]) => ({ group_id, group_name })));
        });
    }
  };

  const handleCreateJob = async (data: any) => {
    const res = await fetch(`${process.env.NEXT_PUBLIC_API_URL}/cron_job/${data.groupId}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        cron_job_name: data.jobName,
        timings: new Date().toISOString(),
        children_names: data.children,
        dependencies_names: data.dependencies.map((dep: string) => [dep, 0]),
        s3_link: data.dockerfileUrl,
      }),
    });
    // Always refresh jobs from backend after creating a job
    fetch(`${process.env.NEXT_PUBLIC_API_URL}/cron_jobs/${data.groupId}`)
      .then((r) => r.json())
      .then((data) => setJobs(data));
  };

  const handleExecuteJobs = async () => {
    if (!selectedGroup) return;
    setExecuting(true);
    await fetch(`${process.env.NEXT_PUBLIC_API_URL}/execute/cron_job/${selectedGroup.group_id}`);
    setExecuting(false);
  };

  return (
    <main className="container mx-auto p-8">
        <h1 className="text-4xl font-extrabold mb-8 text-center text-blue-900 drop-shadow">Job Scheduler Dashboard</h1>
        <div className="flex flex-col md:flex-row gap-8">
          <div className="md:w-1/3 w-full bg-white rounded-xl shadow p-6">
            <h2 className="text-2xl font-semibold mb-4 text-blue-700">Groups</h2>
            <GroupForm onCreate={handleCreateGroup} />
            <div className="mt-4">
              <GroupList groups={groups} onSelect={setSelectedGroup} />
            </div>
          </div>
          <div className="md:w-2/3 w-full">
            {selectedGroup && (
              <div className="bg-white rounded-xl shadow p-6">
                <h2 className="text-2xl font-semibold mb-4 text-blue-700">Jobs in {selectedGroup.group_name}</h2>
                <CronJobForm
                  onCreate={handleCreateJob}
                  groupId={selectedGroup.group_id}
                  allJobs={jobs}
                />
                <button
                  className="bg-green-600 hover:bg-green-700 text-white font-semibold py-2 px-4 rounded shadow mb-6 transition"
                  onClick={handleExecuteJobs}
                  disabled={executing}
                >
                  {executing ? "Executing..." : "Execute All Cron Jobs"}
                </button>
                <ul className="space-y-4">
                  {jobs.map((j) => (
                    <li key={j.cron_job_id} className="bg-gray-50 border border-gray-200 rounded-lg p-4 shadow-sm hover:shadow-md transition">
                      <div className="flex items-center justify-between mb-1">
                        <div className="font-bold text-lg text-blue-900">{j.cron_job_name}</div>
                        <span className={`text-xs px-2 py-1 rounded ${jobStatuses[j.cron_job_id]?.status === 'completed' ? 'bg-green-100 text-green-700' : jobStatuses[j.cron_job_id]?.status === 'running' ? 'bg-yellow-100 text-yellow-700' : jobStatuses[j.cron_job_id]?.status === 'failed' ? 'bg-red-100 text-red-700' : 'bg-gray-100 text-gray-700'}`}>{jobStatuses[j.cron_job_id]?.status || "unknown"}</span>
                      </div>
                      <div className="text-xs text-gray-500 mb-2 truncate">Dockerfile: <a href={j.s3_link} className="underline" target="_blank" rel="noopener noreferrer">{j.s3_link}</a></div>
                      <div className="text-xs mb-1"><span className="font-semibold text-gray-700">Dependencies:</span>{" "}
  {j.dependencies && j.dependencies.length > 0
    ? j.dependencies.join(", ")
    : "None"}</div>
                      <div className="text-xs text-gray-400 mt-1">
                        {jobStatuses[j.cron_job_id]?.updated_at && (
                          <span>Last updated: {new Date(jobStatuses[j.cron_job_id].updated_at!).toLocaleString()}</span>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
    </main>
  );
}
