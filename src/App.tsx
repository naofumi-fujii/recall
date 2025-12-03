import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface ClipboardEntry {
  timestamp: string;
  content: string;
}

function App() {
  const [history, setHistory] = useState<ClipboardEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);

  const loadHistory = async () => {
    try {
      const entries = await invoke<ClipboardEntry[]>("get_history");
      setHistory(entries);
    } catch (error) {
      console.error("Failed to load history:", error);
    }
  };

  useEffect(() => {
    loadHistory();

    const unlistenChanged = listen<ClipboardEntry>("clipboard-changed", () => {
      loadHistory();
    });

    const unlistenCleared = listen("history-cleared", () => {
      setHistory([]);
    });

    return () => {
      unlistenChanged.then((f) => f());
      unlistenCleared.then((f) => f());
    };
  }, []);

  const handleCopy = async (content: string, index: number) => {
    try {
      await invoke("copy_to_clipboard", { content });
      setCopiedIndex(index);
      // Close window after copy
      await getCurrentWindow().hide();
      setTimeout(() => setCopiedIndex(null), 1500);
    } catch (error) {
      console.error("Failed to copy:", error);
    }
  };

  const handleClear = async () => {
    if (window.confirm("履歴をすべてクリアしますか？")) {
      try {
        await invoke("clear_all_history");
        setHistory([]);
      } catch (error) {
        console.error("Failed to clear history:", error);
      }
    }
  };

  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    return date.toLocaleString("ja-JP", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  const filteredHistory = history.filter((entry) =>
    entry.content.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="app">
      <header className="header">
        <h1>Banzai</h1>
        <p className="subtitle">Clipboard History</p>
      </header>

      <div className="search-container">
        <input
          type="text"
          className="search-input"
          placeholder="検索..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
        <button
          className="clear-button"
          onClick={handleClear}
          disabled={history.length === 0}
        >
          クリア
        </button>
      </div>

      <div className="history-count">
        {filteredHistory.length} / {history.length} 件
      </div>

      <div className="history-list">
        {filteredHistory.length === 0 ? (
          <div className="empty-state">
            {history.length === 0
              ? "履歴がありません"
              : "検索結果がありません"}
          </div>
        ) : (
          filteredHistory.map((entry, index) => (
            <div
              key={`${entry.timestamp}-${index}`}
              className={`history-item ${copiedIndex === index ? "copied" : ""}`}
              onClick={() => handleCopy(entry.content, index)}
            >
              <div className="history-timestamp">
                {formatTimestamp(entry.timestamp)}
              </div>
              <div className="history-content">{entry.content}</div>
              {copiedIndex === index && (
                <div className="copied-badge">コピーしました</div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default App;
