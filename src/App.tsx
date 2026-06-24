import { useEffect, useState, useRef, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Monitor, Sun, Moon, Trash2 } from "lucide-react";

interface ClipboardEntry {
  timestamp: string;
  content: string;
  pinned: boolean;
}

interface HistoryResponse {
  entries: ClipboardEntry[];
  max_entries: number;
}

type Theme = "system" | "light" | "dark";

const ThemeIcon = ({ theme }: { theme: Theme }) => {
  const iconProps = { size: 16, strokeWidth: 2 };
  switch (theme) {
    case "system":
      return <Monitor {...iconProps} />;
    case "light":
      return <Sun {...iconProps} />;
    case "dark":
      return <Moon {...iconProps} />;
  }
};

function App() {
  const [history, setHistory] = useState<ClipboardEntry[]>([]);
  const [maxEntries, setMaxEntries] = useState<number>(100);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [selectedIndex, setSelectedIndex] = useState<number>(0);
  const [query, setQuery] = useState<string>("");
  const [version, setVersion] = useState<string>("");
  const [showClearConfirm, setShowClearConfirm] = useState<boolean>(false);
  const [theme, setTheme] = useState<Theme>(() => {
    return (localStorage.getItem("theme") as Theme) || "system";
  });
  const listRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Filters history by case-insensitive AND match on content (src/App.tsx).
  // The query is split on whitespace and every term must be a substring of the
  // entry content, so space-separated words act as an AND search.
  const filteredHistory = useMemo(() => {
    const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return history;
    return history.filter((entry) => {
      const content = entry.content.toLowerCase();
      return terms.every((term) => content.includes(term));
    });
  }, [history, query]);

  useEffect(() => {
    localStorage.setItem("theme", theme);
    if (theme === "system") {
      document.body.removeAttribute("data-theme");
    } else {
      document.body.setAttribute("data-theme", theme);
    }
  }, [theme]);

  const cycleTheme = () => {
    const themes: Theme[] = ["system", "light", "dark"];
    const currentIndex = themes.indexOf(theme);
    const nextIndex = (currentIndex + 1) % themes.length;
    setTheme(themes[nextIndex]);
  };

  const loadHistory = async () => {
    try {
      const response = await invoke<HistoryResponse>("get_history");
      setHistory(response.entries);
      setMaxEntries(response.max_entries);
    } catch (error) {
      console.error("Failed to load history:", error);
    }
  };

  const scrollToSelected = useCallback((index: number) => {
    const item = itemRefs.current[index];
    if (item) {
      item.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // While the clear-all confirmation dialog is open (src/App.tsx), Enter
      // confirms the deletion and Escape cancels it; all list navigation keys
      // are suppressed so they don't act on the hidden list underneath.
      if (showClearConfirm) {
        if (e.key === "Enter") {
          e.preventDefault();
          confirmClearAll();
        } else if (e.key === "Escape") {
          e.preventDefault();
          cancelClearAll();
        }
        return;
      }

      // Ignore keystrokes emitted while an IME composition is active (src/App.tsx).
      // When converting Japanese with the IME, the Enter that confirms the
      // conversion would otherwise be treated as "copy & close". keyCode 229 is
      // the legacy fallback for browsers that don't set isComposing reliably.
      if (e.isComposing || e.keyCode === 229) return;

      // Whether focus is in the search input (src/App.tsx). When typing in the
      // search box, j/k should be entered as text instead of navigating.
      const inSearch = document.activeElement === searchInputRef.current;

      if (filteredHistory.length === 0) return;

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.min(prev + 1, filteredHistory.length - 1);
            scrollToSelected(next);
            return next;
          });
          break;
        case "j":
          if (inSearch) return;
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.min(prev + 1, filteredHistory.length - 1);
            scrollToSelected(next);
            return next;
          });
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.max(prev - 1, 0);
            scrollToSelected(next);
            return next;
          });
          break;
        case "k":
          if (inSearch) return;
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.max(prev - 1, 0);
            scrollToSelected(next);
            return next;
          });
          break;
        case "Enter":
          e.preventDefault();
          if (filteredHistory[selectedIndex]) {
            handleCopy(filteredHistory[selectedIndex].content, selectedIndex);
          }
          break;
      }
    },
    [filteredHistory, selectedIndex, scrollToSelected, showClearConfirm]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  // Keep selection within bounds when the filtered list shrinks (src/App.tsx)
  useEffect(() => {
    setSelectedIndex((prev) =>
      prev > filteredHistory.length - 1 ? 0 : prev
    );
  }, [filteredHistory.length]);

  useEffect(() => {
    loadHistory();
    getVersion().then(setVersion);

    const unlistenChanged = listen<ClipboardEntry>("clipboard-changed", () => {
      loadHistory();
      setSelectedIndex(0);
    });

    // When the window is shown via the hotkey, reset the search and focus the
    // input so the user can immediately type to filter (src/App.tsx)
    const unlistenShow = listen("show-window-at-mouse", () => {
      setQuery("");
      setSelectedIndex(0);
      requestAnimationFrame(() => searchInputRef.current?.focus());
    });

    return () => {
      unlistenChanged.then((f) => f());
      unlistenShow.then((f) => f());
    };
  }, []);

  const handleCopy = async (content: string, index: number) => {
    try {
      await invoke("copy_to_clipboard", { content });
      setCopiedIndex(index);
      // Close window after copy
      await getCurrentWindow().hide();
      // Restore focus to the previous application
      await invoke("restore_previous_app");
      setTimeout(() => setCopiedIndex(null), 1500);
    } catch (error) {
      console.error("Failed to copy:", error);
    }
  };

  const handleTogglePin = async (
    e: React.MouseEvent,
    timestamp: string,
    currentPinned: boolean
  ) => {
    e.stopPropagation();
    try {
      await invoke("toggle_pin", { timestamp, pinned: !currentPinned });
      loadHistory();
    } catch (error) {
      console.error("Failed to toggle pin:", error);
    }
  };

  // Emacs/readline-style line editing for the search input (src/App.tsx).
  // Ctrl+U clears to line start, Ctrl+W deletes the word before the cursor;
  // Escape clears the whole field. (Cmd is also accepted for U/W.)
  const handleSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    const mod = e.metaKey || e.ctrlKey;
    const el = e.currentTarget;

    // Ctrl+U: delete from cursor back to the line start
    if (mod && e.key === "u") {
      e.preventDefault();
      const cursor = el.selectionStart ?? query.length;
      const after = query.slice(el.selectionEnd ?? cursor);
      setQuery(after);
      requestAnimationFrame(() => el.setSelectionRange(0, 0));
      return;
    }

    // Ctrl+W: delete the word before the cursor (trailing whitespace + word)
    if (mod && e.key === "w") {
      e.preventDefault();
      const start = el.selectionStart ?? query.length;
      const end = el.selectionEnd ?? start;
      const before = query.slice(0, start).replace(/\s+$/, "").replace(/\S+$/, "");
      const after = query.slice(end);
      setQuery(before + after);
      requestAnimationFrame(() =>
        el.setSelectionRange(before.length, before.length)
      );
      return;
    }

    if (e.key === "Escape" && query) {
      e.preventDefault();
      e.stopPropagation();
      setQuery("");
    }
  };

  // Opens the clear-all confirmation dialog in src/App.tsx (settings-row trash
  // button). The actual deletion is deferred to confirmClearAll so an accidental
  // click no longer wipes history immediately.
  const handleClearAll = () => {
    setShowClearConfirm(true);
  };

  // Performs the actual history deletion in src/App.tsx after the user confirms
  // in the dialog opened by handleClearAll. Invokes the clear_all_history
  // command, then reloads the list and closes the dialog.
  const confirmClearAll = async () => {
    try {
      await invoke("clear_all_history");
      loadHistory();
      setSelectedIndex(0);
    } catch (error) {
      console.error("Failed to clear history:", error);
    } finally {
      setShowClearConfirm(false);
    }
  };

  // Cancels the clear-all confirmation dialog in src/App.tsx without deleting.
  const cancelClearAll = () => {
    setShowClearConfirm(false);
  };

  return (
    <div className="app">
      <header className="header">
        <h1>Recall {version && <span className="version">v{version}</span>}</h1>
        <p className="subtitle">Clipboard History</p>
        <button className="theme-toggle" onClick={cycleTheme} title={theme}>
          <ThemeIcon theme={theme} />
        </button>
      </header>

      <div className="search-container">
        <input
          ref={searchInputRef}
          type="text"
          className="search-input"
          placeholder="絞り込み..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleSearchKeyDown}
          autoFocus
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
        />
      </div>

      <div className="settings-row">
        <span className="history-count">
          {query.trim()
            ? `${filteredHistory.length}件 / ${history.length}件`
            : `${history.length}/${maxEntries}件`}
        </span>
        <button
          className="clear-button"
          onClick={handleClearAll}
          disabled={history.length === 0}
          title="全件クリア"
        >
          <Trash2 size={12} />
        </button>
      </div>

      <div className="history-list" ref={listRef}>
        {filteredHistory.length === 0 ? (
          <div className="empty-state">
            {query.trim() ? "一致する履歴がありません" : "履歴がありません"}
          </div>
        ) : (
          filteredHistory.map((entry, index) => (
            <div
              key={`${entry.timestamp}-${index}`}
              ref={(el) => {
                itemRefs.current[index] = el;
              }}
              className={`history-item ${copiedIndex === index ? "copied" : ""} ${selectedIndex === index ? "selected" : ""} ${entry.pinned ? "pinned" : ""}`}
              onClick={() => handleCopy(entry.content, index)}
              onMouseEnter={() => setSelectedIndex(index)}
            >
              <input
                type="checkbox"
                className="pin-checkbox"
                checked={entry.pinned}
                onClick={(e) => handleTogglePin(e, entry.timestamp, entry.pinned)}
                onChange={() => {}}
                title={entry.pinned ? "Unpin" : "Pin"}
              />
              <span className="history-content">{entry.content}</span>
              <div className="history-tooltip">{entry.content}</div>
            </div>
          ))
        )}
      </div>

      {showClearConfirm && (
        <div className="confirm-overlay" onClick={cancelClearAll}>
          <div
            className="confirm-dialog"
            onClick={(e) => e.stopPropagation()}
          >
            <p className="confirm-message">
              ピン留め以外の履歴をすべて削除しますか？
            </p>
            <div className="confirm-actions">
              <button
                className="confirm-cancel"
                onClick={cancelClearAll}
                autoFocus
              >
                キャンセル
              </button>
              <button className="confirm-delete" onClick={confirmClearAll}>
                削除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
