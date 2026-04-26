// File: src/stores/appStore.ts
import { create } from "zustand";
import type { SearchResult, IndexStats, AppConfig } from "../types";

export interface IndexProgress {
  phase: string;
  message: string;
  progress_pct: number | null;
  done: boolean;
  error: string | null;
}

interface AppState {
  query: string;
  results: SearchResult[];
  isSearching: boolean;
  stats: IndexStats | null;
  config: AppConfig | null;
  statusMessage: string;
  isRebuilding: boolean;
  indexProgress: IndexProgress | null;

  setQuery: (query: string) => void;
  setResults: (results: SearchResult[]) => void;
  setIsSearching: (searching: boolean) => void;
  setStats: (stats: IndexStats | null) => void;
  setConfig: (config: AppConfig | null) => void;
  setStatusMessage: (message: string) => void;
  setIsRebuilding: (rebuilding: boolean) => void;
  setIndexProgress: (progress: IndexProgress | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  query: "",
  results: [],
  isSearching: false,
  stats: null,
  config: null,
  statusMessage: "Ready",
  isRebuilding: false,
  indexProgress: null,

  setQuery: (query) => set({ query }),
  setResults: (results) => set({ results }),
  setIsSearching: (isSearching) => set({ isSearching }),
  setStats: (stats) => set({ stats }),
  setConfig: (config) => set({ config }),
  setStatusMessage: (statusMessage) => set({ statusMessage }),
  setIsRebuilding: (isRebuilding) => set({ isRebuilding }),
  setIndexProgress: (indexProgress) => set({ indexProgress }),
}));