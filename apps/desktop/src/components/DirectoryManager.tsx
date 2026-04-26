// File: src/components/DirectoryManager.tsx
import React, { useState } from "react";
import type { AppConfig } from "../types";
import { addDirectory, removeDirectory } from "../services/tauriApi";
import { useAppStore } from "../stores/appStore";

interface Props {
  config: AppConfig;
  onConfigChanged: () => void;
}

const DirectoryManager: React.FC<Props> = ({ config, onConfigChanged }) => {
  const [newPath, setNewPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const { setStatusMessage } = useAppStore();

  const handleAdd = async () => {
    const trimmed = newPath.trim();
    if (!trimmed) return;

    try {
      setError(null);
      await addDirectory(trimmed);
      setNewPath("");
      setStatusMessage(`Directory added: ${trimmed}`);
      onConfigChanged();
    } catch (err: any) {
      setError(err?.toString() || "Failed to add directory");
    }
  };

  const handleRemove = async (path: string) => {
    try {
      setError(null);
      await removeDirectory(path);
      setStatusMessage(`Directory removed: ${path}`);
      onConfigChanged();
    } catch (err: any) {
      setError(err?.toString() || "Failed to remove directory");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleAdd();
    }
  };

  return (
    <div className="directory-manager">
      <h3>Indexed Directories</h3>

      {error && <div className="error-message">{error}</div>}

      <div className="dir-add-row">
        <input
          type="text"
          className="dir-input"
          placeholder="Enter directory path..."
          value={newPath}
          onChange={(e) => setNewPath(e.target.value)}
          onKeyDown={handleKeyDown}
        />
        <button className="btn btn-primary" onClick={handleAdd}>
          Add
        </button>
      </div>

      <div className="dir-list">
        {config.directories.length === 0 ? (
          <div className="dir-empty">No directories configured.</div>
        ) : (
          config.directories.map((dir) => (
            <div key={dir.path} className="dir-item">
              <div className="dir-path">
                <span className={dir.enabled ? "" : "disabled"}>
                  📁 {dir.path}
                </span>
                {!dir.enabled && <span className="badge">disabled</span>}
              </div>
              <button
                className="btn btn-danger btn-sm"
                onClick={() => handleRemove(dir.path)}
              >
                Remove
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
};

export default DirectoryManager;