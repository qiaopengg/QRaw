import {
  AlertCircle,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  FileWarning,
  Folder,
  LockKeyhole,
  Search,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import { useMemo, useState } from "react";
import { folders, hdrPhotos, labelMeta } from "../data.js";
import { TopBar, ViewControls } from "../components/AppShell.jsx";
import { LabelControls, PhotoCard, Stars } from "../components/PhotoUI.jsx";

export function ReviewScreen({ onNavigate, items, onToggle, onEdit, stage = "review", onBack }) {
  const [focusedId, setFocusedId] = useState("DSC_4122.ARW");
  const [filter, setFilter] = useState("all");
  const [showFailures, setShowFailures] = useState(true);
  const focused = items.find((photo) => photo.id === focusedId) ?? items[0];
  const selectedCount = items.filter((photo) => photo.selected).length;
  const visibleItems = useMemo(() => {
    if (filter === "selected") return items.filter((photo) => photo.selected);
    if (filter === "review") return items.filter((photo) => photo.label === "review");
    return items;
  }, [filter, items]);

  return (
    <div className="app-frame review-frame">
      <TopBar stage={stage} onNavigate={onNavigate}>
        <div className="review-counts"><span>精选 286</span><span>待确认 312</span><span>淘汰建议 1,024</span><span>失败 2</span></div>
        <ViewControls />
        <button className="primary-button review-apply" onClick={() => onNavigate("confirm")}>确认应用 286 项</button>
      </TopBar>
      <div className="review-layout">
        <aside className="story-sidebar">
          <header><button onClick={onBack}><ChevronLeft size={16} /></button><div><strong>智能选图复核</strong><span>李明 & 王悦 · 婚礼纪实</span></div></header>
          <div className="story-search"><Search size={14} /> 搜索文件夹或故事…</div>
          <div className="story-section-label"><Folder size={14} /> 文件夹</div>
          {folders.map((folder) => <button className={`story-folder ${folder.id === "ceremony" ? "is-active" : ""}`} key={folder.id}><span>{folder.label}</span><em>{folder.count}</em></button>)}
          <div className="story-section-label"><SlidersHorizontal size={14} /> 当前文件夹故事段落</div>
          <button className="story-segment is-active"><span>交换戒指</span><em>32</em></button>
          <button className="story-segment"><span>誓言</span><em>38</em></button>
          <button className="story-segment"><span>亲吻</span><em>24</em></button>
          <footer><span>已选择 {selectedCount} 张</span><button onClick={() => setFilter(filter === "selected" ? "all" : "selected")}>{filter === "selected" ? "显示全部" : "只看已选"}</button></footer>
        </aside>
        <main className="review-canvas">
          <header className="review-section-header"><div><h1>02 仪式</h1><span>交换戒指 · 32 张</span></div><div className="review-filters"><button className={filter === "all" ? "is-active" : ""} onClick={() => setFilter("all")}>全部</button><button className={filter === "review" ? "is-active" : ""} onClick={() => setFilter("review")}>待确认</button><span>按时间排序 <ChevronDown size={13} /></span></div></header>
          <section className="similar-group">
            <header><div><span className="group-dot is-green" /><strong>相似组 28（连拍 5 张）</strong><small>建议保留 3–5 张</small></div><div><span>相似度：高</span><button><ChevronLeft size={15} /></button><button><ChevronRight size={15} /></button></div></header>
            <div className="review-photo-row">
              {visibleItems.slice(0, 5).map((photo) => <PhotoCard key={photo.id} photo={photo} focused={focused.id === photo.id} onFocus={() => setFocusedId(photo.id)} onToggle={() => onToggle(photo.id)} onRating={(rating) => onEdit(photo.id, { rating })} />)}
            </div>
          </section>
          <section className="protected-group">
            <header><div><LockKeyhole size={16} /><strong>受保护组合 · HDR（3 张）</strong><span>整体保留，不因相似度淘汰</span></div><button>查看详情 <ChevronDown size={14} /></button></header>
            <div>{hdrPhotos.map((photo) => <article key={photo.id}><img src={photo.src} alt="受保护 HDR 组合" /><span>{photo.id}</span><small>AI（组合）</small></article>)}</div>
          </section>
          <section className="collapsed-group"><div><span className="group-dot is-yellow" /><strong>相似组 29（连拍 4 张）</strong><small>建议保留 3–4 张</small></div><button>展开组</button></section>
          <section className="failed-group"><button onClick={() => setShowFailures(!showFailures)}><FileWarning size={16} /><strong>失败与跳过 · 140 项</strong><span>138 张人工保护 · 2 张分析失败</span><ChevronDown size={15} /></button>{showFailures ? <div><p><AlertCircle size={14} /><span>DSC_4258.ARW</span><em>解码失败：文件损坏或格式不完整</em></p><p><ShieldCheck size={14} /><span>138 张已有人工结果</span><em>已保护并跳过，不会被修改</em></p></div> : null}</section>
        </main>
        <aside className="review-inspector">
          <header><div><strong>{focused.id}</strong><span>3 / 5</span></div><div><button><ChevronLeft size={15} /></button><button><ChevronRight size={15} /></button></div></header>
          <div className="inspector-photo"><img src={focused.src} alt="当前复核照片" /><span>24mm · 1/200 · f/2.8 · ISO 800</span></div>
          <section><h3>AI 评估理由 <span>AI</span></h3>{focused.source === "人工" ? <p className="manual-reason"><LockKeyhole size={15} /> 已由用户修改，AI 原因已清除。</p> : <p>{focused.reason}</p>}{focused.confidence ? <div className="confidence"><span>置信度</span><div><i style={{ width: `${focused.confidence}%` }} /></div><em>{focused.confidence}%</em></div> : null}</section>
          <section><h3>当前评分与标记</h3><Stars value={focused.rating} onChange={(rating) => onEdit(focused.id, { rating })} /><LabelControls value={focused.label} onChange={(label) => onEdit(focused.id, { label })} /><div className="source-line">来源：<span className={`source-badge ${focused.source === "人工" ? "is-manual" : ""}`}>{focused.source}</span></div></section>
          <section className="protection-note"><ShieldCheck size={18} /><div><strong>{focused.source === "人工" ? "此结果已受保护" : "修改后将转为人工结果"}</strong><p>{focused.source === "人工" ? "后续智能选图会整张跳过，不覆盖本次决定。" : "任何星级或标签修改都会清除 AI 原因，并保护最终决定。"}</p></div></section>
          <footer><span><span className={`color-dot is-${labelMeta[focused.label].color}`} />{labelMeta[focused.label].label}</span><button onClick={() => onToggle(focused.id)}>{focused.selected ? "取消采用" : "采用结果"}</button></footer>
        </aside>
      </div>
    </div>
  );
}
