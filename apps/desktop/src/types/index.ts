// File: src/types/index.ts

export interface FileDocument {
  id: number;
  name: string;
  name_lower: string;
  path: string;
  parent: string;
  extension: string;
  size: number;
  modified: string;
  is_dir: boolean;
}

export interface SearchResult {
  document: FileDocument;
  score: number;
  snippet: string | null;
}

export interface IndexStats {
  total_documents: number;
  total_files: number;
  total_directories: number;
  total_size_bytes: number;
  indexed_roots: string[];
  last_scan: string | null;
  last_update: string | null;
  trigram_count: number;
  segment_count: number;
  index_size_bytes: number;
}

export interface AppConfig {
  directories: IndexedDirectory[];
  excluded_patterns: string[];
  default_result_limit: number;
  auto_watch: boolean;
  auto_rebuild: boolean;
  index_content: boolean;
  content_max_size: number;
  content_extensions: string[];
}

export interface IndexedDirectory {
  path: string;
  enabled: boolean;
}

export interface IndexProgressEvent {
  phase: string;
  message: string;
  progress_pct: number | null;
  done: boolean;
  error: string | null;
  stats: IndexStats | null;
}