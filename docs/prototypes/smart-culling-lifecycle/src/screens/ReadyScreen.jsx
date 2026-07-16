import { Bell, CheckCircle2, Clock3, FolderOpen, ShieldCheck } from "lucide-react";
import { photos } from "../data.js";
import { LibrarySidebar, SmartCullingButton, TopBar, ViewControls } from "../components/AppShell.jsx";

export function ReadyScreen({ onNavigate, partial = false }) {
  return (
    <div className="app-frame">
      <TopBar stage="ready" onNavigate={onNavigate} pending><ViewControls /><SmartCullingButton onClick={() => onNavigate("review")} pending /></TopBar>
      <div className="library-layout">
        <LibrarySidebar active="toast" />
        <main className="library-main">
          <header className="library-toolbar"><div><h1>04 敬酒</h1><p>你可以继续只读浏览其他文件夹</p></div><span>786 张</span></header>
          <section className="analysis-photo-grid is-ready">{photos.slice(1, 8).map((photo, index) => <article key={`${photo.id}-${index}`}><img src={photo.src} alt="图库照片" /><span>{photo.id}</span></article>)}</section>
          <section className="completion-toast">
            <span className="completion-icon"><CheckCircle2 size={22} /></span>
            <div><strong>{partial ? "任务已取消，部分结果可复核" : "智能选图分析完成"}</strong><p>{partial ? "已完成 1,124 张，未完成照片保持原样。" : "成功分析 2,708 张，138 张人工结果已保护。"}</p><div><span><ShieldCheck size={13} /> 尚未写入星级或标签</span><span><Clock3 size={13} /> 结果等待你的确认</span></div></div>
            <button className="primary-button" onClick={() => onNavigate("review")}>进入复核</button>
          </section>
          <section className="pending-card" onClick={() => onNavigate("review")}><Bell size={19} /><div><strong>结果待复核</strong><span>{partial ? "1,124 张已完成结果" : "286 张精选 · 312 张待确认 · 1,024 张淘汰建议"}</span></div><em>打开</em></section>
          <footer className="ready-footnote"><FolderOpen size={14} /> 任务根目录：2025婚礼拍摄 · 完全离线分析</footer>
        </main>
      </div>
    </div>
  );
}
