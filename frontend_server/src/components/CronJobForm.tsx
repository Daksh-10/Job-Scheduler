import React, { useState } from "react";

interface CronJobFormProps {
  onCreate: (data: {
    jobName: string;
    dockerfileUrl: string;
    groupId: string;
    children: string[];
    dependencies: string[];
  }) => void;
  groupId: string;
  allJobs: { cron_job_id: string; cron_job_name: string }[];
}

export const CronJobForm: React.FC<CronJobFormProps> = ({ onCreate, groupId, allJobs }) => {
  const [jobName, setJobName] = useState("");
  const [dockerfileUrl, setDockerfileUrl] = useState("");
  const [children, setChildren] = useState<string>("");
  const [dependencies, setDependencies] = useState<string>("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (jobName && dockerfileUrl) {
      const childrenArr = children.split(",").map(s => s.trim()).filter(Boolean);
      const dependenciesArr = dependencies.split(",").map(s => s.trim()).filter(Boolean);
      onCreate({ jobName, dockerfileUrl, groupId, children: childrenArr, dependencies: dependenciesArr });
      setJobName("");
      setDockerfileUrl("");
      setChildren("");
      setDependencies("");
    }
  };

  return (
    <form onSubmit={handleSubmit} className="cronjob-form flex flex-col gap-2 mb-4">
      <input
        type="text"
        value={jobName}
        onChange={(e) => setJobName(e.target.value)}
        placeholder="Job name"
        className="input input-bordered"
      />
      <input
        type="text"
        value={dockerfileUrl}
        onChange={(e) => setDockerfileUrl(e.target.value)}
        placeholder="Dockerfile URL"
        className="input input-bordered"
      />
      <label>Children (comma-separated job names)</label>
      <input
        type="text"
        value={children}
        onChange={e => setChildren(e.target.value)}
        placeholder="e.g. jobB, jobC"
        className="input input-bordered"
      />
      <label>Dependencies (comma-separated job names)</label>
      <input
        type="text"
        value={dependencies}
        onChange={e => setDependencies(e.target.value)}
        placeholder="e.g. jobA, jobD"
        className="input input-bordered"
      />
      <button type="submit" className="btn btn-primary mt-2">Create Cron Job</button>
    </form>
  );
};
