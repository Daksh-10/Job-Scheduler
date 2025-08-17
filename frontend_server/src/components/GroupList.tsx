import React from "react";

export interface Group {
  group_id: string;
  group_name: string;
}

interface GroupListProps {
  groups: Group[];
  onSelect: (group: Group) => void;
}

export const GroupList: React.FC<GroupListProps> = ({ groups, onSelect }) => (
  <ul className="group-list">
    {groups.map((g) => (
      <li key={g.group_id} onClick={() => onSelect(g)}>
        {g.group_name}
      </li>
    ))}
  </ul>
);
