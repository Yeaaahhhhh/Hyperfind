// File: src/components/SearchBar.tsx
import React from "react";
import { useAppStore } from "../stores/appStore";

const SearchBar: React.FC = () => {
  const { query, setQuery, isSearching } = useAppStore();

  return (
    <div className="search-bar">
      <div className="search-input-wrapper">
        <span className="search-icon">🔍</span>
        <input
          type="text"
          className="search-input"
          placeholder="Search files... (try: ext:rs size:>1024 path:src)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoFocus
        />
        {isSearching && <span className="search-spinner">⏳</span>}
      </div>
    </div>
  );
};

export default SearchBar;