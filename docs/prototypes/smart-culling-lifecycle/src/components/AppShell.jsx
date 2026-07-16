import {
  Bell,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  Folder,
  Grid2X2,
  Layers3,
  List,
  Search,
  Settings,
  Sparkles,
  X,
} from "lucide-react";
import { folders, lifecyclePages } from "../data.js";

const steps = [
  { id: "setup", label: "配置", index: 1 },
  { id: "analysis", label: "分析", index: 2 },
  { id: "review", label: "复核", index: 3 },
  { id: "confirm", label: "确认", index: 4 },
];

function stageRank(stage) {
  const map = { library: 0, setup: 1, people: 1, unsupported: 1, analysis: 2, ready: 2, review: 3, confirm: 4, write: 4, "library-result": 5 };
  return map[stage] ?? 0;
}

export function TopBar({ stage, children, onNavigate, pending = false }) {
  const rank = stageRank(stage);

  return (
    <header className="topbar">
      <button className="brand" onClick={() => onNavigate("library")} aria-label="返回 Library">
        QRaw
      </button>
      {rank > 0 ? (
        <nav className="stepper" aria-label="智能选图进度">
          {steps.map((step, index) => {
            const current = rank === step.index;
            const complete = rank > step.index;
            return (
              <div className={`step ${current ? "is-current" : ""} ${complete ? "is-complete" : ""}`} key={step.id}>
                <span className="step-index">{complete ? "✓" : step.index}</span>
                <span>{step.label}</span>
                {index < steps.length - 1 ? <span className="step-line" /> : null}
              </div>
            );
          })}
        </nav>
      ) : (
        <div className="library-title">Library</div>
      )}
      <div className="topbar-actions">
        {pending ? <span className="pending-chip"><Bell size={14} /> 结果待复核</span> : null}
        {children}
      </div>
    </header>
  );
}

export function LibrarySidebar({ active = "ceremony", compact = false }) {
  return (
    <aside className={`library-sidebar ${compact ? "is-compact" : ""}`}>
      <div className="sidebar-search"><Search size={15} /><span>搜索文件夹或故事…</span></div>
      <div className="sidebar-label"><Folder size={14} /> 2025婚礼拍摄 <span>12,860</span></div>
      <div className="sidebar-subtitle">李明 & 王悦 · 婚礼纪实</div>
      <div className="folder-list">
        {folders.map((folder) => (
          <button className={folder.id === active ? "is-active" : ""} key={folder.id}>
            <Folder size={15} /><span>{folder.label}</span><em>{folder.count.toLocaleString()}</em>
          </button>
        ))}
      </div>
      {!compact ? (
        <div className="sidebar-footer">
          <button><CircleHelp size={15} /> 智能选图说明</button>
          <button><Settings size={15} /> 显示设置</button>
        </div>
      ) : null}
    </aside>
  );
}

export function ViewControls() {
  return (
    <div className="view-controls" aria-label="视图控制">
      <button aria-label="网格视图" className="is-active"><Grid2X2 size={16} /></button>
      <button aria-label="列表视图"><List size={17} /></button>
      <span />
      <button aria-label="设置"><Settings size={16} /></button>
    </div>
  );
}

export function PrototypeMap({ current, open, onOpen, onClose, onNavigate }) {
  return (
    <>
      <button className="prototype-map-trigger" onClick={onOpen} title="打开完整页面地图">
        <Layers3 size={16} /> 页面地图
      </button>
      {open ? (
        <div className="prototype-map-backdrop" role="dialog" aria-modal="true" aria-label="全生命周期页面地图">
          <section className="prototype-map-panel">
            <header><div><span>Product Design</span><h2>智能选图全生命周期页面</h2></div><button onClick={onClose}><X size={18} /></button></header>
            <div className="prototype-map-list">
              {lifecyclePages.map((page) => (
                <button className={page.id === current ? "is-current" : ""} key={page.id} onClick={() => { onNavigate(page.id); onClose(); }}>
                  <span className="page-group">{page.group}</span>
                  <strong>{page.label}</strong>
                  <small>{page.summary}</small>
                  <ChevronRight size={15} />
                </button>
              ))}
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
}

export function BackButton({ onClick, label = "返回" }) {
  return <button className="text-button" onClick={onClick}><ChevronLeft size={16} />{label}</button>;
}

export function SmartCullingButton({ onClick, pending = false }) {
  return <button className={`smart-culling-button ${pending ? "is-pending" : ""}`} onClick={onClick}><Sparkles size={17} />{pending ? "查看智能选图结果" : "智能选图"}</button>;
}
