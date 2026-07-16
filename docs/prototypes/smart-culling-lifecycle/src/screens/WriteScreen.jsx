import { AlertCircle, CheckCircle2, FileWarning, LoaderCircle, RefreshCcw, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { photos } from "../data.js";
import { TopBar } from "../components/AppShell.jsx";

export function WriteScreen({ onNavigate }) {
  const [retried, setRetried] = useState(false);
  const failures = retried ? 1 : 2;
  return (
    <div className="app-frame write-frame">
      <TopBar stage="write" onNavigate={onNavigate}><span className="write-complete"><CheckCircle2 size={15} /> 写入完成</span></TopBar>
      <main className="write-workspace">
        <header><span className="write-hero-icon"><CheckCircle2 size={26} /></span><div><h1>智能选图结果已应用</h1><p>284 项写入成功，{failures} 项失败。原始照片没有被移动、复制或删除。</p></div></header>
        <section className="write-metrics"><article><strong>284</strong><span>写入成功</span></article><article><strong>{failures}</strong><span>写入失败</span></article><article><strong>138</strong><span>人工保护跳过</span></article><article><strong>2</strong><span>分析失败</span></article></section>
        <div className="write-content">
          <section className="write-success-list"><header><h2>最近成功</h2><span>已写入 .rrdata</span></header>{photos.slice(0, 4).map((photo) => <article key={photo.id}><img src={photo.src} alt="成功写入照片" /><div><strong>{photo.id}</strong><span>{photo.rating} 星 · {photo.source === "人工" ? "人工结果" : "AI 结果"}</span></div><CheckCircle2 size={18} /></article>)}</section>
          <section className="write-failures"><header><h2>失败项</h2><span>{failures} 项可重试</span></header><article><FileWarning size={18} /><div><strong>DSC_4281.ARW</strong><span>.rrdata 被外部程序修改，为避免覆盖已跳过</span></div><button onClick={() => setRetried(true)} disabled={retried}><RefreshCcw size={14} />{retried ? "仍有冲突" : "重试"}</button></article>{!retried ? <article><AlertCircle size={18} /><div><strong>DSC_4302.JPG</strong><span>写入时文件暂时被占用</span></div><button onClick={() => setRetried(true)}><RefreshCcw size={14} /> 重试</button></article> : null}<div className="write-protection"><ShieldCheck size={16} /><p>其他成功结果已保留，不会因为单张失败而回滚。</p></div></section>
        </div>
        <footer><button className="secondary-button" onClick={() => onNavigate("review")}>返回查看结果</button><button className="primary-button" onClick={() => onNavigate("library-result")}>回到 Library</button></footer>
      </main>
    </div>
  );
}
