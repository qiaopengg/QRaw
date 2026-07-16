import { Bell, Filter, Image, ShieldCheck, Star } from "lucide-react";
import { photos } from "../data.js";
import { LibrarySidebar, SmartCullingButton, TopBar, ViewControls } from "../components/AppShell.jsx";
import { LabelBadge, Stars } from "../components/PhotoUI.jsx";

export function LibraryScreen({ onNavigate, result = false, pending = false }) {
  return (
    <div className="app-frame">
      <TopBar stage={result ? "library-result" : "library"} onNavigate={onNavigate} pending={pending}>
        <ViewControls />
        <SmartCullingButton onClick={() => onNavigate(pending ? "review" : "setup")} pending={pending} />
      </TopBar>
      <div className="library-layout">
        <LibrarySidebar />
        <main className="library-main">
          <header className="library-toolbar">
            <div><h1>李明 & 王悦 · 婚礼纪实</h1><p>2025婚礼拍摄 / 02 仪式 · 684 张</p></div>
            <div><button><Filter size={15} /> 筛选</button><span>按拍摄时间</span></div>
          </header>
          {pending ? (
            <button className="ready-banner" onClick={() => onNavigate("review")}>
              <span className="ready-banner-icon"><Bell size={20} /></span>
              <span><strong>智能选图已完成，710 张结果待复核</strong><small>分析期间没有修改任何星级或标签 · 点击进入复核</small></span>
              <em>查看结果</em>
            </button>
          ) : null}
          {result ? (
            <section className="result-summary">
              <ShieldCheck size={18} /><div><strong>智能选图结果已应用</strong><span>286 张已写入 .rrdata，2 张写入失败可稍后重试</span></div>
              <button onClick={() => onNavigate("write")}>查看写入详情</button>
            </section>
          ) : null}
          <section className="library-photo-grid">
            {photos.map((photo, index) => (
              <article className="library-photo" key={`${photo.id}-${index}`}>
                <div className="library-photo-media"><img src={photo.src} alt="婚礼图库照片" />{index === 2 ? <span className="active-photo-mark"><Image size={14} /></span> : null}</div>
                <div className="library-photo-info">
                  <div className="library-photo-primary"><span>{photo.id}</span>{result ? <span className={`source-badge ${photo.source === "人工" ? "is-manual" : ""}`}>{photo.source}</span> : null}</div>
                  {result ? (
                    <>
                      <div className="library-result-row"><Stars value={photo.rating} compact /><LabelBadge value={photo.label} /></div>
                      <p>{photo.source === "人工" ? "人工结果已保护" : photo.reason}</p>
                    </>
                  ) : (
                    <div className="library-empty-meta"><Star size={12} /> 尚未评分</div>
                  )}
                </div>
              </article>
            ))}
          </section>
        </main>
      </div>
    </div>
  );
}
