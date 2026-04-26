// File: src/pages/SearchPage.tsx
import React, { useEffect, useState } from "react";
import SearchBar from "../components/SearchBar";
import ResultsList from "../components/ResultsList";
import StatusBar from "../components/StatusBar";
import { useAppStore } from "../stores/appStore";
import { useDebounce } from "../hooks/useDebounce";
import {
  searchFiles,
  loadIndex,
  indexAllVolumes,
  onIndexProgress,
} from "../services/tauriApi";
import type { IndexProgressEvent } from "../types";

const SearchPage: React.FC = () => {
  const {
    query,
    setResults,
    setIsSearching,
    setStats,
    setStatusMessage,
    setIsRebuilding,
    setIndexProgress,
    isRebuilding,
    indexProgress,
  } = useAppStore();

  const debouncedQuery = useDebounce(query, 200);
  const [showWelcome, setShowWelcome] = useState(false);
  const [indexLoaded, setIndexLoaded] = useState(false);

  // Listen for progress events
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    onIndexProgress((event: IndexProgressEvent) => {
      setIndexProgress({
        phase: event.phase,
        message: event.message,
        progress_pct: event.progress_pct,
        done: event.done,
        error: event.error,
      });
      setStatusMessage(event.message);

      if (event.done && event.stats) {
        setStats(event.stats);
        setIsRebuilding(false);
        setShowWelcome(false);
        setIndexLoaded(true);
        setIndexProgress(null);
      }

      if (event.error) {
        setIsRebuilding(false);
        setIndexProgress(null);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, [setIndexProgress, setStatusMessage, setStats, setIsRebuilding]);

  // Load index on mount — non-blocking
  useEffect(() => {
    const init = async () => {
      try {
        const loadedStats = await loadIndex();
        setStats(loadedStats);
        if (loadedStats.total_documents === 0) {
          setShowWelcome(true);
          setStatusMessage("No index found. Click 'Start Indexing' to begin.");
        } else {
          setStatusMessage(
            `Index loaded: ${loadedStats.total_documents.toLocaleString()} items`
          );
          setIndexLoaded(true);
        }
      } catch {
        setShowWelcome(true);
        setStatusMessage("No index found. Click 'Start Indexing' to begin.");
      }
    };
    init();
  }, [setStats, setStatusMessage]);

  // Search when debounced query changes
  useEffect(() => {
    if (!indexLoaded) return;

    const doSearch = async () => {
      if (debouncedQuery.trim().length === 0) {
        setResults([]);
        return;
      }
      setIsSearching(true);
      try {
        const results = await searchFiles(debouncedQuery);
        setResults(results);
        setStatusMessage(`Found ${results.length} results`);
        setShowWelcome(false);
      } catch (err) {
        console.error("Search error:", err);
        setStatusMessage("Search failed");
        setResults([]);
      } finally {
        setIsSearching(false);
      }
    };
    doSearch();
  }, [debouncedQuery, indexLoaded, setResults, setIsSearching, setStatusMessage]);

  const handleStartIndexing = async () => {
    setIsRebuilding(true);
    setShowWelcome(false);
    setStatusMessage("Starting full system index...");
    try {
      const newStats = await indexAllVolumes();
      setStats(newStats);
      setIndexLoaded(true);
      setStatusMessage(
        `Indexing complete! ${newStats.total_documents.toLocaleString()} items indexed.`
      );
    } catch (err: any) {
      setStatusMessage(`Indexing failed: ${err?.toString()}`);
    } finally {
      setIsRebuilding(false);
    }
  };

  return (
    <div className="search-page">
      <SearchBar />

      {showWelcome && !isRebuilding && (
        <div className="welcome-panel">
          <div className="welcome-icon">⚡</div>
          <h2>Welcome to HyperFind</h2>
          <p>Index all drives on your computer for lightning-fast file search.</p>
          <button className="btn btn-primary btn-lg" onClick={handleStartIndexing}>
            🚀 Start Indexing All Drives
          </button>
        </div>
      )}

      {isRebuilding && (
        <div className="welcome-panel">
          <div className="welcome-icon spinning">⏳</div>
          <h2>Indexing in progress...</h2>
          <p>{indexProgress?.message || "Scanning all drives..."}</p>
          {indexProgress?.progress_pct != null && (
            <div className="progress-bar-container">
              <div
                className="progress-bar-fill"
                style={{ width: `${indexProgress.progress_pct}%` }}
              />
            </div>
          )}
        </div>
      )}

      {!showWelcome && !isRebuilding && <ResultsList />}

      <StatusBar />
    </div>
  );
};

export default SearchPage;