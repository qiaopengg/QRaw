import { Check, ChevronRight, Cpu, FolderTree, Info, ShieldCheck, UserRoundCheck, Zap } from "lucide-react";
import { shootModes } from "../data.js";
import { LibrarySidebar, TopBar } from "../components/AppShell.jsx";

export function SetupScreen({ onNavigate, selectedMode, onModeChange }) {
  const peopleAvailable = ["auto", "portrait", "environment", "group", "documentary"].includes(selectedMode);

  return (
    <div className="app-frame">
      <TopBar stage="setup" onNavigate={onNavigate}><button className="text-button" onClick={() => onNavigate("unsupported")}><Cpu size={15} /> 设备检测</button></TopBar>
      <div className="library-layout is-dimmed-context">
        <LibrarySidebar compact />
        <main className="setup-workspace">
          <section className="setup-heading"><div><span className="eyebrow">智能选图</span><h1>设置本次筛选</h1><p>分析当前文件夹及全部子文件夹。确认前不会写入任何星级或标签。</p></div><span className="preflight-status"><ShieldCheck size={16} /> 设备与模型可用</span></section>
          <div className="setup-grid">
            <section className="setup-primary">
              <header><div><span>1</span><div><h2>选择拍摄类型</h2><p>每次任务选择一种，系统据此调整判断重点。</p></div></div></header>
              <div className="mode-grid">
                {shootModes.map((mode) => (
                  <button className={selectedMode === mode.id ? "is-selected" : ""} key={mode.id} onClick={() => onModeChange(mode.id)}>
                    <span className="radio-dot">{selectedMode === mode.id ? <Check size={12} /> : null}</span><strong>{mode.label}</strong><small>{mode.description}</small>
                  </button>
                ))}
              </div>
              <header className="people-step"><div><span>2</span><div><h2>关键人物 <em>可选</em></h2><p>{peopleAvailable ? "从照片中点选人物，顺序决定优先级。数据仅用于本次任务。" : "当前拍摄类型不使用关键人物，系统已自动跳过。"}</p></div></div></header>
              <button className="people-entry" disabled={!peopleAvailable} onClick={() => onNavigate("people")}><UserRoundCheck size={20} /><span><strong>{peopleAvailable ? "选择关键人物" : "当前模式无需选择"}</strong><small>{peopleAvailable ? "尚未选择 · 可直接开始分析" : "切换到人像、群像或纪实模式后可用"}</small></span><ChevronRight size={18} /></button>
            </section>
            <aside className="task-summary">
              <h2>任务摘要</h2>
              <dl><div><dt><FolderTree size={15} />分析范围</dt><dd>2025婚礼拍摄及 7 个子文件夹</dd></div><div><dt><Zap size={15} />预计数量</dt><dd>2,846 个拍摄资产</dd></div><div><dt><ShieldCheck size={15} />人工保护</dt><dd>已识别 138 张，将整张跳过</dd></div></dl>
              <div className="format-note"><Info size={15} /><p>RAW/JPEG 同名文件按一张照片处理；GIF、TIFF/TIF 将正常跳过。</p></div>
              <div className="setup-actions"><button className="secondary-button" onClick={() => onNavigate("library")}>取消</button><button className="primary-button" onClick={() => onNavigate("analysis")}>开始选图</button></div>
              <small className="offline-note">完全离线运行 · 不上传照片或人物数据</small>
            </aside>
          </div>
        </main>
      </div>
    </div>
  );
}
