// File: src/components/StatusBar.tsx
import React from "react";
import { useAppStore } from "../stores/appStore";

function formatSize(bytes: number): string {
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`;
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${bytes} B`;
}

const StatusBar: React.FC = () => {
  const { stats, statusMessage, isRebuilding, indexProgress } = useAppStore();

  return (
    <div className="status-bar">
      <div className="status-left">
        {isRebuilding
          ? `🔄 ${indexProgress?.message || "Working..."}`
          : `💬 ${statusMessage}`}
      </div>
      <div className="status-right">
        {stats && (
          <>
            <span>📊 {stats.total_documents.toLocaleString()} items</span>
            <span>|</span>
            <span>📄 {stats.total_files.toLocaleString()} files</span>
            <span>|</span>
            <span>📁 {stats.total_directories.toLocaleString()} dirs</span>
            <span>|</span>
            <span>💾 {formatSize(stats.total_size_bytes)}</span>
            {stats.index_size_bytes > 0 && (
              <>
                <span>|</span>
                <span>📦 Index: {formatSize(stats.index_size_bytes)}</span>
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default StatusBar;