// File: src/components/ResultItem.tsx
import React from "react";
import type { SearchResult } from "../types";
import { openFile, openContainingFolder, copyToClipboard } from "../services/tauriApi";

interface Props {
  result: SearchResult;
  index: number;
}

function formatSize(bytes: number): string {
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`;
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${bytes} B`;
}

function formatDate(dateStr: string): string {
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString() + " " + date.toLocaleTimeString();
  } catch {
    return dateStr;
  }
}

const ResultItem: React.FC<Props> = ({ result }) => {
  const doc = result.document;
  const icon = doc.is_dir ? "📁" : "📄";

  const handleDoubleClick = async () => {
    try { await openFile(doc.path); } catch (err) { console.error("Failed to open file:", err); }
  };

  const handleOpenFolder = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try { await openContainingFolder(doc.path); } catch (err) { console.error(err); }
  };

  const handleCopyPath = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try { await copyToClipboard(doc.path); } catch (err) { console.error(err); }
  };

  return (
    <div className="result-item" onDoubleClick={handleDoubleClick}>
      <div className="result-icon">{icon}</div>
      <div className="result-info">
        <div className="result-name">{doc.name}</div>
        <div className="result-path">{doc.path}</div>
        {result.snippet && (
          <div className="result-snippet">
            💬 {result.snippet}
          </div>
        )}
        <div className="result-meta">
          {!doc.is_dir && <span className="result-size">{formatSize(doc.size)}</span>}
          <span className="result-modified">{formatDate(doc.modified)}</span>
          {doc.extension && <span className="result-ext">.{doc.extension}</span>}
        </div>
      </div>
      <div className="result-actions">
        <button className="action-btn" onClick={handleOpenFolder} title="Open containing folder">📂</button>
        <button className="action-btn" onClick={handleCopyPath} title="Copy path">📋</button>
      </div>
    </div>
  );
};

export default ResultItem;