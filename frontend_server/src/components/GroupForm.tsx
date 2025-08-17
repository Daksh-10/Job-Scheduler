import React, { useState } from "react";

interface GroupFormProps {
  onCreate: (groupName: string) => void;
}

export const GroupForm: React.FC<GroupFormProps> = ({ onCreate }) => {
  const [groupName, setGroupName] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (groupName.trim()) {
      onCreate(groupName.trim());
      setGroupName("");
    }
  };

  return (
    <form onSubmit={handleSubmit} className="group-form flex gap-2 mb-4">
      <input
        type="text"
        value={groupName}
        onChange={(e) => setGroupName(e.target.value)}
        placeholder="New group name"
        className="input input-bordered"
      />
      <button type="submit" className="btn btn-primary">
        Create Group
      </button>
    </form>
  );
};
