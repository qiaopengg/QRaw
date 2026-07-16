import { AlertTriangle, ArchiveX, CheckCircle2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { photos } from "./data.js";
import { PrototypeMap } from "./components/AppShell.jsx";
import { AnalysisScreen } from "./screens/AnalysisScreen.jsx";
import { ConfirmOverlay } from "./screens/ConfirmOverlay.jsx";
import { LibraryScreen } from "./screens/LibraryScreen.jsx";
import { PeopleScreen } from "./screens/PeopleScreen.jsx";
import { ReadyScreen } from "./screens/ReadyScreen.jsx";
import { ReviewScreen } from "./screens/ReviewScreen.jsx";
import { SetupScreen } from "./screens/SetupScreen.jsx";
import { UnsupportedScreen } from "./screens/UnsupportedScreen.jsx";
import { WriteScreen } from "./screens/WriteScreen.jsx";

const validScreens = new Set([
  "library",
  "setup",
  "people",
  "analysis",
  "ready",
  "review",
  "confirm",
  "write",
  "library-result",
  "unsupported",
]);

function readScreen() {
  const requested = new URLSearchParams(window.location.search).get("screen");
  return validScreens.has(requested) ? requested : "library";
}

export function App() {
  const [screen, setScreen] = useState(readScreen);
  const [mapOpen, setMapOpen] = useState(false);
  const [selectedMode, setSelectedMode] = useState("auto");
  const [partial, setPartial] = useState(false);
  const [showAbandon, setShowAbandon] = useState(false);
  const [items, setItems] = useState(() => photos.slice(0, 5).map((photo) => ({ ...photo })));
  const qaMode = new URLSearchParams(window.location.search).get("qa") === "1";

  const navigate = useCallback((nextScreen) => {
    const nextUrl = new URL(window.location.href);
    nextUrl.searchParams.set("screen", nextScreen);
    window.history.pushState({}, "", nextUrl);
    setScreen(nextScreen);
  }, []);

  useEffect(() => {
    const handlePopState = () => setScreen(readScreen());
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  const toggleItem = useCallback((id) => {
    setItems((current) => current.map((photo) => photo.id === id ? { ...photo, selected: !photo.selected } : photo));
  }, []);

  const editItem = useCallback((id, update) => {
    setItems((current) => current.map((photo) => photo.id === id ? {
      ...photo,
      ...update,
      source: "人工",
      reason: "",
      confidence: null,
    } : photo));
  }, []);

  const selectedCount = items.filter((photo) => photo.selected).length + 283;
  const manualCount = items.filter((photo) => photo.source === "人工").length;

  let content;
  switch (screen) {
    case "setup":
      content = <SetupScreen onNavigate={navigate} selectedMode={selectedMode} onModeChange={setSelectedMode} />;
      break;
    case "people":
      content = <PeopleScreen onNavigate={navigate} />;
      break;
    case "analysis":
      content = <AnalysisScreen onNavigate={navigate} onPartial={() => setPartial(true)} />;
      break;
    case "ready":
      content = <ReadyScreen onNavigate={navigate} partial={partial} />;
      break;
    case "review":
      content = <ReviewScreen onNavigate={navigate} items={items} onToggle={toggleItem} onEdit={editItem} onBack={() => setShowAbandon(true)} />;
      break;
    case "confirm":
      content = (
        <>
          <ReviewScreen onNavigate={navigate} items={items} onToggle={toggleItem} onEdit={editItem} stage="confirm" onBack={() => setShowAbandon(true)} />
          <ConfirmOverlay onNavigate={navigate} selectedCount={selectedCount} manualCount={manualCount} />
        </>
      );
      break;
    case "write":
      content = <WriteScreen onNavigate={navigate} />;
      break;
    case "library-result":
      content = <LibraryScreen onNavigate={navigate} result />;
      break;
    case "unsupported":
      content = <UnsupportedScreen onNavigate={navigate} />;
      break;
    default:
      content = <LibraryScreen onNavigate={navigate} />;
  }

  return (
    <div className="prototype-root">
      {content}
      {showAbandon ? (
        <div className="modal-backdrop">
          <section className="confirm-dialog">
            <span className="dialog-icon is-warning"><AlertTriangle size={22} /></span>
            <h2>离开复核并放弃结果？</h2>
            <p>尚未确认的筛选结果不会写入 .rrdata。离开后本次任务会被清除，需要重新开始。</p>
            <div className="dialog-facts"><span><CheckCircle2 size={14} /> 原图保持不变</span><span><ArchiveX size={14} /> 临时结果和人物特征将清除</span></div>
            <div className="dialog-actions"><button className="secondary-button" onClick={() => setShowAbandon(false)}>继续复核</button><button className="danger-button" onClick={() => { setShowAbandon(false); navigate("library"); }}>完全放弃</button></div>
          </section>
        </div>
      ) : null}
      {qaMode ? null : <PrototypeMap current={screen} open={mapOpen} onOpen={() => setMapOpen(true)} onClose={() => setMapOpen(false)} onNavigate={navigate} />}
    </div>
  );
}
