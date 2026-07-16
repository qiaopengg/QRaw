import { AlertTriangle, Ban, Check, FolderOpen, LoaderCircle, Pause, ShieldCheck, X } from "lucide-react";
import { useEffect, useState } from "react";
import { photos } from "../data.js";
import { LibrarySidebar, TopBar, ViewControls } from "../components/AppShell.jsx";

export function AnalysisScreen({ onNavigate, onPartial }) {
  const [progress, setProgress] = useState(38);
  const [showCancel, setShowCancel] = useState(false);

  useEffect(() => {
    const timer = window.setInterval(() => setProgress((value) => Math.min(value + 2, 92)), 1200);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <div className="app-frame">
      <TopBar stage="analysis" onNavigate={onNavigate}><span className="running-status"><LoaderCircle size={15} /> 正在后台分析</span><ViewControls /></TopBar>
      <div className="library-layout">
        <LibrarySidebar active="portrait" />
        <main className="library-main analysis-library">
          <header className="library-toolbar"><div><h1>03 合影</h1><p>只读浏览 · 智能选图任务范围仍为 2025婚礼拍摄</p></div><span className="readonly-chip"><ShieldCheck size={15} /> 分析期间只读</span></header>
          <section className="analysis-photo-grid">{photos.slice(0, 8).map((photo, index) => <article key={`${photo.id}-${index}`}><img src={photo.src} alt="只读图库照片" /><span>{photo.id}</span></article>)}</section>
          <section className="analysis-dock">
            <div className="analysis-dock-title"><span className="analysis-icon"><LoaderCircle size={19} /></span><div><strong>正在分析当前渲染状态</strong><small>清晰度与曝光 · 人物状态 · 相似分组</small></div></div>
            <div className="analysis-dock-progress"><div><span>已完成 {Math.round((progress / 100) * 2846).toLocaleString()} / 2,846</span><em>预计剩余 03:42</em></div><div className="progress-track"><span style={{ width: `${progress}%` }} /></div></div>
            <button className="secondary-button" onClick={() => setShowCancel(true)}><X size={15} /> 取消任务</button>
          </section>
          <aside className="blocked-actions"><Ban size={15} /><span>编辑、打星、标签、移动和高负载任务暂不可用</span></aside>
        </main>
      </div>
      {showCancel ? (
        <div className="modal-backdrop"><section className="confirm-dialog"><span className="dialog-icon is-warning"><AlertTriangle size={22} /></span><h2>取消智能选图？</h2><p>系统会停止继续分析，但保留已完成的 {Math.round((progress / 100) * 2846).toLocaleString()} 张结果供你复核。未完成照片保持原样。</p><div className="dialog-facts"><span><Check size={14} /> 保留已完成结果</span><span><Pause size={14} /> 停止新增重任务</span><span><FolderOpen size={14} /> 不修改原始照片</span></div><div className="dialog-actions"><button className="secondary-button" onClick={() => setShowCancel(false)}>继续分析</button><button className="danger-button" onClick={() => { onPartial(); onNavigate("ready"); }}>取消并查看已完成结果</button></div></section></div>
      ) : null}
      <button className="prototype-skip" onClick={() => onNavigate("ready")}>演示：完成分析</button>
    </div>
  );
}
