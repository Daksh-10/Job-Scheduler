import { NextResponse } from "next/server";

let groups: { group_id: string; group_name: string }[] = [];
let groupIdCounter = 1;

export async function GET() {
  return NextResponse.json(groups);
}

export async function POST(req: Request) {
  const { group_name } = await req.json();
  const group = { group_id: String(groupIdCounter++), group_name };
  groups.push(group);
  return NextResponse.json(group);
}
