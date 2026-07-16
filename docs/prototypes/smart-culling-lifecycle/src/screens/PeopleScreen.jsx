import { Check, ChevronDown, ChevronUp, Info, MousePointer2, Trash2, UserRoundCheck } from "lucide-react";
import { photos } from "../data.js";
import { BackButton, TopBar } from "../components/AppShell.jsx";

const people = [
  { id: "bride", name: "关键人物 1", detail: "最高优先级", photo: photos[4].src },
  { id: "groom", name: "关键人物 2", detail: "第二优先级", photo: photos[2].src },
];

export function PeopleScreen({ onNavigate }) {
  return (
    <div className="app-frame">
      <TopBar stage="people" onNavigate={onNavigate}><BackButton onClick={() => onNavigate("setup")} label="返回配置" /></TopBar>
      <main className="people-workspace">
        <section className="people-browser">
          <header><div><span className="eyebrow">步骤 2 · 可选</span><h1>选择关键人物</h1><p>先选照片，再点击照片中具体人物。选择顺序即优先级。</p></div><span className="privacy-chip"><UserRoundCheck size={15} /> 仅本次任务使用</span></header>
          <div className="people-photo-stage">
            <img src={photos[2].src} alt="用于选择关键人物的婚礼照片" />
            <button className="face-target is-bride" aria-label="选择新娘"><span>1</span></button>
            <button className="face-target is-groom" aria-label="选择新郎"><span>2</span></button>
            <div className="click-hint"><MousePointer2 size={16} /> 点击人物完成选择</div>
          </div>
          <div className="people-filmstrip">{photos.slice(0, 6).map((photo, index) => <button className={index === 2 ? "is-active" : ""} key={`${photo.id}-${index}`}><img src={photo.src} alt="人物样本缩略图" /></button>)}</div>
        </section>
        <aside className="people-priority">
          <header><h2>已选择 2 人</h2><p>优先保留包含高优先级人物的有效照片。</p></header>
          <div className="priority-list">
            {people.map((person, index) => (
              <article key={person.id}><span className="priority-number">{index + 1}</span><img src={person.photo} alt="关键人物头像" /><div><strong>{person.name}</strong><small>{person.detail}</small></div><div className="priority-actions"><button aria-label="上移"><ChevronUp size={15} /></button><button aria-label="下移"><ChevronDown size={15} /></button><button aria-label="删除"><Trash2 size={15} /></button></div></article>
            ))}
          </div>
          <div className="privacy-note"><Info size={16} /><p>人物样本和特征会在任务完成、放弃或应用退出时清除，不保存身份信息。</p></div>
          <div className="people-actions"><button className="secondary-button" onClick={() => onNavigate("setup")}>暂不选择</button><button className="primary-button" onClick={() => onNavigate("analysis")}><Check size={16} /> 保存并开始</button></div>
        </aside>
      </main>
    </div>
  );
}
