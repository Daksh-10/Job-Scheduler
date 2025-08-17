import { NextResponse } from "next/server";

// In-memory jobs by group
const jobsByGroup: Record<string, any[]> = {};
let jobIdCounter = 1;

export async function GET(
  req: Request,
  { params }: { params: { groupId: string } }
) {
  const { groupId } = params;
  return NextResponse.json(jobsByGroup[groupId] || []);
}

export async function POST(
  req: Request,
  { params }: { params: { groupId: string } }
) {
  const { groupId } = params;
  const data = await req.json();
  const job = {
    job_id: String(jobIdCounter++),
    job_name: data.jobName,
    dockerfile_url: data.dockerfileUrl,
    group_id: groupId,
    children: data.children || [],
    dependencies: data.dependencies || [],
  };
  if (!jobsByGroup[groupId]) jobsByGroup[groupId] = [];
  jobsByGroup[groupId].push(job);
  return NextResponse.json(job);
}
