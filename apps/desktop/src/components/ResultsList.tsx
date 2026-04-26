// File: src/components/ResultsList.tsx
import React from "react";
import { useAppStore } from "../stores/appStore";
import ResultItem from "./ResultItem";

const ResultsList: React.FC = () => {
  const { results, query, isSearching } = useAppStore();

  if (isSearching) {
    return <div className="results-message">Searching...</div>;
  }

  if (query.length === 0) {
    return (
      <div className="results-message">
        Type a query to search your indexed files.
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className="results-message">No results found for "{query}"</div>
    );
  }

  return (
    <div className="results-list">
      <div className="results-header">
        {results.length} result{results.length !== 1 ? "s" : ""}
      </div>
      {results.map((result, index) => (
        <ResultItem key={result.document.path} result={result} index={index} />
      ))}
    </div>
  );
};

export default ResultsList;