import { AlertCircle, Check, FileCheck2, LockKeyhole, ShieldCheck, X } from "lucide-react";

export function ConfirmOverlay({ onNavigate, selectedCount, manualCount }) {
  return (
    <div className="modal-backdrop confirm-apply-backdrop">
      <section className="apply-dialog">
        <header><div><span className="dialog-icon"><FileCheck2 size={22} /></span><div><h2>确认应用智能选图结果</h2><p>系统将在再次检查文件状态后写入对应 .rrdata。</p></div></div><button onClick={() => onNavigate("review")}><X size={18} /></button></header>
        <div className="apply-summary"><article><strong>{selectedCount}</strong><span>将应用结果</span></article><article><strong>{manualCount}</strong><span>人工修改并保护</span></article><article><strong>138</strong><span>已有人工结果已跳过</span></article><article><strong>2</strong><span>分析失败不写入</span></article></div>
        <div className="apply-detail-list"><p><Check size={15} /><span><strong>写入星级、颜色标签和来源</strong><small>未修改结果标记为 AI，用户修改结果标记为人工。</small></span></p><p><ShieldCheck size={15} /><span><strong>确认前再次检查 .rrdata 基线</strong><small>外部修改或并发冲突会跳过，不覆盖新数据。</small></span></p><p><LockKeyhole size={15} /><span><strong>RAW/JPEG 配对只写 RAW sidecar</strong><small>只有非 RAW 文件时写入该文件自己的 .rrdata。</small></span></p></div>
        <div className="apply-warning"><AlertCircle size={15} /><p>写入允许部分成功。少数失败项不会回滚其他照片，完成后可查看并重试。</p></div>
        <footer><button className="secondary-button" onClick={() => onNavigate("review")}>返回复核</button><button className="primary-button" onClick={() => onNavigate("write")}>确认并写入 {selectedCount} 项</button></footer>
      </section>
    </div>
  );
}
