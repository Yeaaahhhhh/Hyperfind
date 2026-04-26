// File: src/App.tsx
import React, { useState } from "react";
import SearchPage from "./pages/SearchPage";
import SettingsPage from "./pages/SettingsPage";

type Page = "search" | "settings";

const App: React.FC = () => {
  const [currentPage, setCurrentPage] = useState<Page>("search");

  return (
    <div className="app-container">
      <nav className="app-nav">
        <div className="nav-brand">⚡ HyperFind</div>
        <div className="nav-links">
          <button
            className={`nav-link ${currentPage === "search" ? "active" : ""}`}
            onClick={() => setCurrentPage("search")}
          >
            Search
          </button>
          <button
            className={`nav-link ${currentPage === "settings" ? "active" : ""}`}
            onClick={() => setCurrentPage("settings")}
          >
            Settings
          </button>
        </div>
      </nav>
      <main className="app-main">
        {currentPage === "search" && <SearchPage />}
        {currentPage === "settings" && <SettingsPage />}
      </main>
    </div>
  );
};

export default App;