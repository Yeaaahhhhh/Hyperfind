// File: src/pages/SettingsPage.tsx
import React, { useEffect, useState } from "react";
import DirectoryManager from "../components/DirectoryManager";
import StatusBar from "../components/StatusBar";
import { useAppStore } from "../stores/appStore";
import { getConfig, rebuildIndex, getStats, indexAllVolumes } from "../services/tauriApi";
import type { AppConfig } from "../types";

const SettingsPage: React.FC = () => {
  const { setStats, setStatusMessage, setIsRebuilding, isRebuilding } = useAppStore();
  const [config, setConfig] = useState<AppConfig | null>(null);

  const loadConfig = async () => {
    try {
      const cfg = await getConfig();
      setConfig(cfg);
    } catch (err) {
      console.error("Failed to load config:", err);
    }
  };

  useEffect(() => {
    loadConfig();
  }, []);

  const handleRebuild = async () => {
    setIsRebuilding(true);
    setStatusMessage("Rebuilding index...");
    try {
      const stats = await rebuildIndex();
      setStats(stats);
      setStatusMessage(`Index rebuilt: ${stats.total_documents.toLocaleString()} items`);
    } catch (err: any) {
      setStatusMessage(`Rebuild failed: ${err?.toString()}`);
    } finally {
      setIsRebuilding(false);
    }
  };

  const handleReindexAll = async () => {
    setIsRebuilding(true);
    setStatusMessage("Re-indexing all drives...");
    try {
      const stats = await indexAllVolumes();
      setStats(stats);
      setStatusMessage(`Re-index complete: ${stats.total_documents.toLocaleString()} items`);
    } catch (err: any) {
      setStatusMessage(`Re-index failed: ${err?.toString()}`);
    } finally {
      setIsRebuilding(false);
    }
  };

  const handleRefreshStats = async () => {
    try {
      const stats = await getStats();
      setStats(stats);
    } catch (err) {
      console.error("Failed to refresh stats:", err);
    }
  };

  return (
    <div className="settings-page">
      <h2>Settings</h2>

      {config && <DirectoryManager config={config} onConfigChanged={loadConfig} />}

      <div className="settings-actions">
        <h3>Index Management</h3>
        <div className="action-row">
          <button className="btn btn-primary" onClick={handleRebuild} disabled={isRebuilding}>
            {isRebuilding ? "Working..." : "🔄 Rebuild Index"}
          </button>
          <button className="btn btn-primary" onClick={handleReindexAll} disabled={isRebuilding}>
            {isRebuilding ? "Working..." : "🚀 Re-index All Drives"}
          </button>
          <button className="btn btn-secondary" onClick={handleRefreshStats}>
            📊 Refresh Stats
          </button>
        </div>
      </div>

      <div className="settings-section">
        <h3>Excluded Patterns</h3>
        {config && (
          <div className="excluded-list">
            {config.excluded_patterns.map((pattern) => (
              <span key={pattern} className="excluded-tag">{pattern}</span>
            ))}
          </div>
        )}
      </div>

      <StatusBar />
    </div>
  );
};

export default SettingsPage;