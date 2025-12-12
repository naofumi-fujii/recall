import { useEffect, useState, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { Monitor, Sun, Moon } from "lucide-react";

interface ClipboardEntry {
  timestamp: string;
  content: string;
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
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [selectedIndex, setSelectedIndex] = useState<number>(0);
  const [version, setVersion] = useState<string>("");
  const [theme, setTheme] = useState<Theme>(() => {
    return (localStorage.getItem("theme") as Theme) || "system";
  });
  const listRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);

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
      const entries = await invoke<ClipboardEntry[]>("get_history");
      setHistory(entries);
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
      if (history.length === 0) return;

      switch (e.key) {
        case "ArrowDown":
        case "j":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.min(prev + 1, history.length - 1);
            scrollToSelected(next);
            return next;
          });
          break;
        case "ArrowUp":
        case "k":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const next = Math.max(prev - 1, 0);
            scrollToSelected(next);
            return next;
          });
          break;
        case "Enter":
          e.preventDefault();
          if (history[selectedIndex]) {
            handleCopy(history[selectedIndex].content, selectedIndex);
          }
          break;
      }
    },
    [history, selectedIndex, scrollToSelected]
  );

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  useEffect(() => {
    loadHistory();
    getVersion().then(setVersion);

    const unlistenChanged = listen<ClipboardEntry>("clipboard-changed", () => {
      loadHistory();
      setSelectedIndex(0);
    });

    return () => {
      unlistenChanged.then((f) => f());
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

  return (
    <div className="app">
      <header className="header">
        <h1>Banzai {version && <span className="version">v{version}</span>}</h1>
        <p className="subtitle">Clipboard History</p>
        <button className="theme-toggle" onClick={cycleTheme} title={theme}>
          <ThemeIcon theme={theme} />
        </button>
      </header>

      <div className="settings-row">
        <span className="history-count">{history.length} 件</span>
      </div>

      <div className="history-list" ref={listRef}>
        {history.length === 0 ? (
          <div className="empty-state">履歴がありません</div>
        ) : (
          history.map((entry, index) => (
            <div
              key={`${entry.timestamp}-${index}`}
              ref={(el) => {
                itemRefs.current[index] = el;
              }}
              className={`history-item ${copiedIndex === index ? "copied" : ""} ${selectedIndex === index ? "selected" : ""}`}
              onClick={() => handleCopy(entry.content, index)}
              onMouseEnter={() => setSelectedIndex(index)}
            >
              <span className="history-content">{entry.content}</span>
              <div className="history-tooltip">{entry.content}</div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default App;
