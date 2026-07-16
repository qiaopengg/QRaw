import { Check, LockKeyhole, Star } from "lucide-react";
import { labelMeta } from "../data.js";

export function Stars({ value, onChange, compact = false }) {
  return (
    <div className={`stars ${compact ? "is-compact" : ""}`} aria-label={`${value} 星`}>
      {[1, 2, 3, 4, 5].map((star) => (
        <button key={star} onClick={() => onChange?.(star)} aria-label={`设置 ${star} 星`}>
          <Star size={compact ? 12 : 18} fill={star <= value ? "currentColor" : "none"} />
        </button>
      ))}
    </div>
  );
}
export function LabelBadge({ value }) {
  const meta = labelMeta[value];
  return <span className={`label-badge is-${meta.color}`}>{meta.label}</span>;
}

export function PhotoCard({ photo, focused, onFocus, onToggle, onRating, onLabel, showReason = false }) {
  return (
    <article className={`photo-card ${focused ? "is-focused" : ""} ${photo.source === "人工" ? "is-manual" : ""}`} onClick={onFocus}>
      <div className="photo-media">
        <img src={photo.src} alt="婚礼照片缩略图" />
        <button className={`keep-check ${photo.selected ? "is-checked" : ""}`} onClick={(event) => { event.stopPropagation(); onToggle?.(); }} aria-label={photo.selected ? "取消采用" : "采用结果"}>
          {photo.selected ? <Check size={14} /> : null}
        </button>
        {photo.source === "人工" ? <span className="manual-lock"><LockKeyhole size={12} /> 已保护</span> : null}
      </div>
      <div className="photo-meta">
        <div><Stars value={photo.rating} onChange={onRating} compact /><LabelBadge value={photo.label} /></div>
        <div className="filename-row"><span>{photo.id}</span><span className={`source-badge ${photo.source === "人工" ? "is-manual" : ""}`}>{photo.source}</span></div>
        {showReason ? <p>{photo.source === "人工" ? "人工保留" : photo.reason}</p> : <small>24mm · 1/200 · f/2.8 · ISO 800</small>}
      </div>
    </article>
  );
}

export function LabelControls({ value, onChange }) {
  return (
    <div className="label-controls" aria-label="颜色标签">
      {Object.entries(labelMeta).map(([id, meta]) => (
        <button className={`${value === id ? "is-active" : ""} is-${meta.color}`} key={id} onClick={() => onChange(id)}>{meta.label}</button>
      ))}
    </div>
  );
}
