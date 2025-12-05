import { useEffect, useState } from "react";
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
  const [autoLaunch, setAutoLaunch] = useState(false);
  const [version, setVersion] = useState<string>("");
  const [theme, setTheme] = useState<Theme>(() => {
    return (localStorage.getItem("theme") as Theme) || "system";
  });

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

  useEffect(() => {
    loadHistory();
    loadAutoLaunchStatus();
    getVersion().then(setVersion);

    const unlistenChanged = listen<ClipboardEntry>("clipboard-changed", () => {
      loadHistory();
    });

    return () => {
      unlistenChanged.then((f) => f());
    };
  }, []);

  const loadAutoLaunchStatus = async () => {
    try {
      const status = await invoke<boolean>("get_auto_launch_status");
      setAutoLaunch(status);
    } catch (error) {
      console.error("Failed to get auto launch status:", error);
    }
  };

  const handleAutoLaunchToggle = async () => {
    try {
      const newValue = !autoLaunch;
      await invoke("toggle_auto_launch", { enabled: newValue });
      setAutoLaunch(newValue);
    } catch (error) {
      console.error("Failed to toggle auto launch:", error);
    }
  };

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
        <label className="auto-launch-toggle">
          <input
            type="checkbox"
            checked={autoLaunch}
            onChange={handleAutoLaunchToggle}
          />
          <span>ログイン時に起動</span>
        </label>
        <span className="history-count">{history.length} 件</span>
      </div>

      <div className="history-list">
        {history.length === 0 ? (
          <div className="empty-state">履歴がありません</div>
        ) : (
          history.map((entry, index) => (
            <div
              key={`${entry.timestamp}-${index}`}
              className={`history-item ${copiedIndex === index ? "copied" : ""}`}
              onClick={() => handleCopy(entry.content, index)}
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
