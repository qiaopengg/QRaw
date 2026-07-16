import { AlertTriangle, CheckCircle2, Cpu, ExternalLink, MonitorX, ShieldX } from "lucide-react";
import { LibrarySidebar, TopBar } from "../components/AppShell.jsx";

export function UnsupportedScreen({ onNavigate }) {
  return (
    <div className="app-frame">
      <TopBar stage="unsupported" onNavigate={onNavigate} />
      <div className="library-layout is-dimmed-context">
        <LibrarySidebar compact />
        <main className="unsupported-workspace">
          <section className="unsupported-panel">
            <span className="unsupported-icon"><MonitorX size={28} /></span>
            <span className="eyebrow">设备检测</span>
            <h1>此设备暂不支持智能选图</h1>
            <p>当前设备无法使用经过验证的 GPU 推理路径。为保证效果、隐私和结果一致性，智能选图不会以不完整能力启动。</p>
            <div className="device-checks"><article><Cpu size={18} /><div><strong>GPU 推理能力</strong><span>Intel UHD 620 · 不在候选支持范围</span></div><AlertTriangle size={18} /></article><article><CheckCircle2 size={18} /><div><strong>本地模型</strong><span>模型与运行时完整</span></div><CheckCircle2 size={18} /></article><article><ShieldX size={18} /><div><strong>不会自动降级</strong><span>不会上传照片，也不会静默切换到未验证路径</span></div><CheckCircle2 size={18} /></article></div>
            <div className="unsupported-note"><strong>这不会影响 QRaw 的其他功能</strong><span>你仍然可以浏览、编辑、打星、设置标签和导出照片。</span></div>
            <footer><button className="secondary-button" onClick={() => onNavigate("setup")}><ExternalLink size={15} /> 查看候选设备说明</button><button className="primary-button" onClick={() => onNavigate("library")}>返回 Library</button></footer>
          </section>
        </main>
      </div>
    </div>
  );
}
