import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import {
  Send,
  Loader2,
  Bot,
  User,
  RotateCcw,
  ChevronDown,
  RefreshCw,
  ImagePlus,
  Brain,
  ChevronRight,
  SlidersHorizontal,
  Save,
} from 'lucide-react';
import { AppSettings, Invokes } from '../../ui/AppProperties';
import { Adjustments, INITIAL_ADJUSTMENTS } from '../../../utils/adjustments';
import Slider from '../../ui/Slider';
import { usePresets } from '../../../hooks/usePresets';

const DEFAULT_MODEL = 'auto';

const PRESET_MODELS = [
  { label: 'auto（自动路由）⭐', value: 'auto', desc: '自然语言修图→qwen3.5:9b，风格迁移→qwen2.5vl:7b' },
  { label: 'qwen3.5:9b ⭐', value: 'qwen3.5:9b', desc: '推荐 · 最强中文理解 · 需16GB内存' },
  { label: 'qwen2.5vl:7b', value: 'qwen2.5vl:7b', desc: '视觉理解 · 适合风格迁移' },
  { label: 'qwen3.5:4b', value: 'qwen3.5:4b', desc: '轻量 · 8GB内存可用' },
  { label: 'qwen3.5:14b', value: 'qwen3.5:14b', desc: '高性能 · 需24GB内存' },
  { label: 'qwen2.5:7b', value: 'qwen2.5:7b', desc: '上一代 · 稳定可靠' },
  { label: 'deepseek-r1:7b', value: 'deepseek-r1:7b', desc: '推理增强' },
  { label: 'llama3.2:3b', value: 'llama3.2:3b', desc: '英文场景' },
];

const HSL_COLOR_LABELS: Record<string, string> = {
  reds: '红色',
  oranges: '橙色',
  yellows: '黄色',
  greens: '绿色',
  aquas: '青色',
  blues: '蓝色',
  purples: '紫色',
  magentas: '洋红',
};

const DEFAULT_STYLE_TRANSFER_SERVICE_URL = 'http://127.0.0.1:7860';

const STYLE_TRANSFER_PRESET_OPTIONS = [
  { label: 'Realistic', value: 'realistic' },
  { label: 'Artistic', value: 'artistic' },
  { label: 'Creative', value: 'creative' },
] as const;

type StyleTransferModeSetting = 'analysis' | 'generative';
type StyleTransferPreset = (typeof STYLE_TRANSFER_PRESET_OPTIONS)[number]['value'];

interface AdjustmentSuggestion {
  key: string;
  value: any;
  complex_value?: any;
  label: string;
  min: number;
  max: number;
  reason: string;
}

interface ChatAdjustResponse {
  understanding: string;
  adjustments: AdjustmentSuggestion[];
  style_debug?: StyleDebugInfo;
  constraint_debug?: ConstraintDebugInfo;
  executionMeta?: StyleTransferExecutionMeta;
  serviceStatus?: StyleTransferServiceStatus;
  outputImagePath?: string;
  previewImagePath?: string;
}

interface StyleTransferExecutionMeta {
  requestedMode: string;
  resolvedMode: string;
  engine: string;
  preset: string;
  refineEnabled: boolean;
  usedFallback: boolean;
}

interface StyleTransferServiceStatus {
  serviceUrl: string;
  reachable: boolean;
  ready: boolean;
  status: string;
  version?: string | null;
  pipeline?: string | null;
  capabilities?: string[];
  detail?: string | null;
}

interface StyleErrorBreakdown {
  tonal: number;
  color: number;
  skin: number;
  highlight_penalty: number;
  total: number;
}

interface StyleDebugInfo {
  before: StyleErrorBreakdown;
  after: StyleErrorBreakdown;
  proximity_before: StyleProximityScore;
  proximity_after: StyleProximityScore;
  improvement_ratio: number;
  dominant_error: string;
  auto_refine_rounds: number;
  suggested_actions: StyleDebugAction[];
  blocked_reasons: string[];
  blocked_items?: StyleConstraintBlockItem[];
  scene_profile?: StyleSceneProfileDebug;
  constraint_debug?: ConstraintDebugInfo;
}

interface StyleSceneProfileDebug {
  reference_tonal_style: string;
  current_tonal_style: string;
  tonal_gain: number;
  highlight_gain: number;
  shadow_gain: number;
  chroma_limit: number;
  chroma_guard_floor: number;
  color_residual_gain: number;
}

interface StyleProximityScore {
  tonal: number;
  color: number;
  skin: number;
  highlight: number;
  overall: number;
}

interface StyleDebugAction {
  key: string;
  label: string;
  recommended_delta: number;
  priority: number;
  reason: string;
}

interface StyleConstraintAction {
  key: string;
  label: string;
  delta: number;
}

interface StyleConstraintBlockItem {
  category: string;
  label: string;
  reason: string;
  hit_count: number;
  severity: number;
  actions: StyleConstraintAction[];
}

interface ConstraintBand {
  hard_min: number;
  hard_max: number;
  soft_min: number;
  soft_max: number;
}

interface ConstraintWindow {
  source: string;
  highlight_risk: number;
  shadow_risk: number;
  saturation_risk: number;
  bands: Record<string, ConstraintBand>;
}

interface ConstraintClampRecord {
  key: string;
  label: string;
  original: number;
  clamped: number;
  reason: string;
}

interface ConstraintDebugInfo {
  window: ConstraintWindow;
  clamp_count: number;
  clamps: ConstraintClampRecord[];
}

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  thinkingContent?: string;
  adjustments?: AdjustmentSuggestion[];
  appliedValues?: Record<string, any>;
  styleDebug?: StyleDebugInfo;
  constraintDebug?: ConstraintDebugInfo;
  executionMeta?: StyleTransferExecutionMeta;
  serviceStatus?: StyleTransferServiceStatus;
  outputImagePath?: string;
  previewImagePath?: string;
}

interface StreamChunkPayload {
  chunk_type: 'thinking' | 'content' | 'done' | 'error';
  text: string;
  result?: ChatAdjustResponse | null;
}

interface LlmChatMessage {
  role: string;
  content: string;
}

interface ChatPanelProps {
  adjustments: Adjustments;
  setAdjustments(updater: (prev: Adjustments) => Adjustments): void;
  llmEndpoint?: string;
  llmApiKey?: string;
  llmModel?: string;
  styleTransferStrength?: number;
  styleTransferHighlightGuard?: number;
  styleTransferSkinProtect?: number;
  appSettings?: AppSettings | null;
  onSettingsChange?(settings: AppSettings): void;
  currentImagePath?: string | null;
  onOpenImage?(path: string, options?: { preserveAdjustments?: boolean }): void;
}

type OllamaStatus = 'checking' | 'online' | 'offline';

function clampStyleTransferConfig(value: number): number {
  return Math.max(0.5, Math.min(2.0, value));
}

function formatStyleTransferConfig(value: number): string {
  return value.toFixed(2).replace(/\.00$/, '');
}

function normalizeServiceUrl(raw: string): string {
  const trimmed = raw.trim().replace(/\/+$/, '');
  if (!trimmed) {
    return DEFAULT_STYLE_TRANSFER_SERVICE_URL;
  }
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }
  return `http://${trimmed}`;
}

function getSimpleAdjustments(adj: Adjustments): Record<string, any> {
  return {
    exposure: adj.exposure,
    brightness: adj.brightness,
    contrast: adj.contrast,
    highlights: adj.highlights,
    shadows: adj.shadows,
    whites: adj.whites,
    blacks: adj.blacks,
    saturation: adj.saturation,
    vibrance: adj.vibrance,
    temperature: adj.temperature,
    tint: adj.tint,
    clarity: adj.clarity,
    dehaze: adj.dehaze,
    structure: adj.structure,
    sharpness: adj.sharpness,
    vignetteAmount: adj.vignetteAmount,
    hsl: adj.hsl,
  };
}

function clampAdjustmentValue(key: string, value: number): number {
  if (key === 'exposure') return Math.max(-2.5, Math.min(2.5, Number(value.toFixed(2))));
  if (key === 'sharpness') return Math.max(0, Math.min(100, Math.round(value)));
  return Math.max(-80, Math.min(80, Math.round(value)));
}

function mergeAdjustments(prev: Adjustments, patch: Partial<Adjustments>): Adjustments {
  const next = { ...prev, ...patch } as Adjustments;
  const patchHsl = (patch as any).hsl;
  if (!patchHsl || typeof patchHsl !== 'object') return next;
  const prevHsl = (prev as any).hsl || {};
  const mergedHsl: any = { ...prevHsl };
  Object.entries(patchHsl).forEach(([color, values]) => {
    if (!values || typeof values !== 'object') return;
    mergedHsl[color] = { ...(prevHsl[color] || {}), ...(values as any) };
  });
  (next as any).hsl = mergedHsl;
  return next;
}

// 检测 Ollama 是否在线
async function checkOllamaStatus(endpoint: string): Promise<boolean> {
  try {
    const res = await fetch(`${endpoint.replace(/\/$/, '')}/api/tags`, {
      signal: AbortSignal.timeout(3000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

// 思考过程折叠块（类似 ChatGPT）
function ThinkingBlock({ content, isStreaming }: { content: string; isStreaming: boolean }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(true);
  const [copied, setCopied] = useState(false);

  // 流式结束后自动折叠
  useEffect(() => {
    if (!isStreaming) {
      const timer = setTimeout(() => setExpanded(false), 800);
      return () => clearTimeout(timer);
    }
  }, [isStreaming]);

  // 复制内容到剪贴板
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  return (
    <div className="w-full rounded-lg border border-surface overflow-hidden">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-1.5 px-2 py-1 text-[10px] text-text-secondary hover:bg-surface/50 transition-colors"
      >
        <Brain size={11} className={isStreaming ? 'animate-pulse text-purple-400' : 'text-text-secondary'} />
        <span>{isStreaming ? t('chat.thinking') : t('chat.thoughtComplete')}</span>
        <ChevronRight
          size={10}
          className={`ml-auto transition-transform duration-200 ${expanded ? 'rotate-90' : ''}`}
        />
        {/* 复制按钮 */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            handleCopy();
          }}
          className="ml-1 p-0.5 rounded hover:bg-surface text-text-secondary hover:text-text-primary transition-colors"
          title={copied ? t('common.copied') : t('common.copy')}
        >
          {copied ? (
            <svg
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="text-green-400"
            >
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          )}
        </button>
      </button>
      {expanded && (
        <div className="px-2 pb-1.5 text-[10px] text-text-secondary/70 leading-relaxed whitespace-pre-wrap max-h-[150px] overflow-y-auto">
          {content}
          {isStreaming && <span className="inline-block w-1 h-3 bg-purple-400/60 animate-pulse ml-0.5 align-middle" />}
        </div>
      )}
    </div>
  );
}

export default function ChatPanel({
  adjustments,
  setAdjustments,
  llmEndpoint,
  llmApiKey,
  llmModel,
  styleTransferStrength,
  styleTransferHighlightGuard,
  styleTransferSkinProtect,
  appSettings,
  onSettingsChange,
  currentImagePath,
  onOpenImage,
}: ChatPanelProps) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus>('checking');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const endpoint = llmEndpoint || 'http://localhost:11434';
  const [activeModel, setActiveModel] = useState(llmModel || DEFAULT_MODEL);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [tuningMenuOpen, setTuningMenuOpen] = useState(false);
  const [customModelInput, setCustomModelInput] = useState('');
  const [styleTransferMode, setStyleTransferMode] = useState<StyleTransferModeSetting>(
    appSettings?.styleTransferMode || 'analysis',
  );
  const [styleTransferPreset, setStyleTransferPreset] = useState<StyleTransferPreset>(
    appSettings?.styleTransferPreset || 'artistic',
  );
  const [styleTransferServiceUrl, setStyleTransferServiceUrl] = useState(
    appSettings?.styleTransferServiceUrl || DEFAULT_STYLE_TRANSFER_SERVICE_URL,
  );
  const [styleTransferEnableRefiner, setStyleTransferEnableRefiner] = useState(
    appSettings?.styleTransferEnableRefiner ?? false,
  );
  const [styleTransferAllowFallback, setStyleTransferAllowFallback] = useState(
    appSettings?.styleTransferAllowFallback ?? true,
  );
  const [styleTransferServiceStatus, setStyleTransferServiceStatus] = useState<StyleTransferServiceStatus | null>(null);
  const [checkingStyleTransferService, setCheckingStyleTransferService] = useState(false);
  const modelMenuRef = useRef<HTMLDivElement>(null);
  const tuningMenuRef = useRef<HTMLDivElement>(null);
  const [styleStrengthInput, setStyleStrengthInput] = useState(formatStyleTransferConfig(styleTransferStrength ?? 1.0));
  const [highlightGuardInput, setHighlightGuardInput] = useState(
    formatStyleTransferConfig(styleTransferHighlightGuard ?? 1.0),
  );
  const [skinProtectInput, setSkinProtectInput] = useState(formatStyleTransferConfig(styleTransferSkinProtect ?? 1.0));
  const [pureStyleTransfer, setPureStyleTransfer] = useState(true);
  const [enableStyleTransferLut, setEnableStyleTransferLut] = useState(true);
  const [enableStyleTransferExpertPreset, setEnableStyleTransferExpertPreset] = useState(true);
  const [enableStyleTransferFeatureMapping, setEnableStyleTransferFeatureMapping] = useState(true);
  const [enableStyleTransferAutoRefine, setEnableStyleTransferAutoRefine] = useState(true);
  const [enableStyleTransferVlm, setEnableStyleTransferVlm] = useState(true);

  const { addPreset } = usePresets(adjustments);

  const persistAppSettings = useCallback(
    (patch: Partial<AppSettings>) => {
      if (onSettingsChange && appSettings) {
        onSettingsChange({
          ...appSettings,
          ...patch,
        });
      }
    },
    [appSettings, onSettingsChange],
  );

  const saveStyleTransferConfig = useCallback(
    (key: 'styleTransferStrength' | 'styleTransferHighlightGuard' | 'styleTransferSkinProtect', raw: string) => {
      const parsed = Number.parseFloat(raw);
      const clamped = Number.isFinite(parsed) ? clampStyleTransferConfig(parsed) : 1.0;
      persistAppSettings({ [key]: clamped } as Partial<AppSettings>);
      return formatStyleTransferConfig(clamped);
    },
    [persistAppSettings],
  );

  const getEffectiveStyleTransferTuning = useCallback(() => {
    const parseOrDefault = (raw: string, fallback: number) => {
      const parsed = Number.parseFloat(raw);
      if (Number.isFinite(parsed)) {
        return clampStyleTransferConfig(parsed);
      }
      return clampStyleTransferConfig(fallback);
    };
    return {
      styleStrength: parseOrDefault(styleStrengthInput, styleTransferStrength ?? 1.0),
      highlightGuardStrength: parseOrDefault(highlightGuardInput, styleTransferHighlightGuard ?? 1.0),
      skinProtectStrength: parseOrDefault(skinProtectInput, styleTransferSkinProtect ?? 1.0),
    };
  }, [
    styleStrengthInput,
    highlightGuardInput,
    skinProtectInput,
    styleTransferStrength,
    styleTransferHighlightGuard,
    styleTransferSkinProtect,
  ]);

  // 检测 Ollama 状态
  const checkStatus = useCallback(async () => {
    setOllamaStatus('checking');
    const online = await checkOllamaStatus(endpoint);
    setOllamaStatus(online ? 'online' : 'offline');
  }, [endpoint]);

  useEffect(() => {
    checkStatus();
  }, [checkStatus]);

  useEffect(() => {
    setStyleStrengthInput(formatStyleTransferConfig(styleTransferStrength ?? 1.0));
  }, [styleTransferStrength]);

  useEffect(() => {
    setHighlightGuardInput(formatStyleTransferConfig(styleTransferHighlightGuard ?? 1.0));
  }, [styleTransferHighlightGuard]);

  useEffect(() => {
    setSkinProtectInput(formatStyleTransferConfig(styleTransferSkinProtect ?? 1.0));
  }, [styleTransferSkinProtect]);

  useEffect(() => {
    setStyleTransferMode(appSettings?.styleTransferMode || 'analysis');
  }, [appSettings?.styleTransferMode]);

  useEffect(() => {
    setStyleTransferPreset(appSettings?.styleTransferPreset || 'artistic');
  }, [appSettings?.styleTransferPreset]);

  useEffect(() => {
    setStyleTransferServiceUrl(appSettings?.styleTransferServiceUrl || DEFAULT_STYLE_TRANSFER_SERVICE_URL);
  }, [appSettings?.styleTransferServiceUrl]);

  useEffect(() => {
    setStyleTransferEnableRefiner(appSettings?.styleTransferEnableRefiner ?? false);
  }, [appSettings?.styleTransferEnableRefiner]);

  useEffect(() => {
    setStyleTransferAllowFallback(appSettings?.styleTransferAllowFallback ?? true);
  }, [appSettings?.styleTransferAllowFallback]);

  const checkStyleTransferService = useCallback(
    async (rawUrl?: string) => {
      const serviceUrl = normalizeServiceUrl(rawUrl ?? styleTransferServiceUrl);
      setCheckingStyleTransferService(true);
      try {
        const status = await invoke<StyleTransferServiceStatus>(Invokes.CheckStyleTransferService, {
          serviceUrl,
        });
        setStyleTransferServiceStatus(status);
        return status;
      } catch (e) {
        const status: StyleTransferServiceStatus = {
          serviceUrl,
          reachable: false,
          ready: false,
          status: 'error',
          capabilities: [],
          detail: String(e),
        };
        setStyleTransferServiceStatus(status);
        return status;
      } finally {
        setCheckingStyleTransferService(false);
      }
    },
    [styleTransferServiceUrl],
  );

  useEffect(() => {
    if (styleTransferMode === 'generative') {
      checkStyleTransferService();
    }
  }, [styleTransferMode, checkStyleTransferService]);

  // 点击外部关闭模型菜单
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (modelMenuRef.current && !modelMenuRef.current.contains(e.target as Node)) {
        setModelMenuOpen(false);
      }
      if (tuningMenuRef.current && !tuningMenuRef.current.contains(e.target as Node)) {
        setTuningMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSliderChange = useCallback(
    (msgId: string, key: string, e: { target: { value: number | string } } | React.ChangeEvent<HTMLInputElement>) => {
      const numericValue = parseFloat(String(e.target.value));
      if (isNaN(numericValue)) return;
      setMessages((prev) =>
        prev.map((msg) => {
          if (msg.id !== msgId || !msg.adjustments) return msg;
          return { ...msg, appliedValues: { ...(msg.appliedValues || {}), [key]: numericValue } };
        }),
      );
      setAdjustments((prev) => ({ ...prev, [key]: numericValue }));
    },
    [setAdjustments],
  );

  const handleHslSliderChange = useCallback(
    (
      msgId: string,
      color: string,
      channel: 'hue' | 'saturation' | 'luminance',
      e: { target: { value: number | string } } | React.ChangeEvent<HTMLInputElement>,
    ) => {
      const numericValue = parseFloat(String(e.target.value));
      if (isNaN(numericValue)) return;
      setMessages((prev) =>
        prev.map((msg) => {
          if (msg.id !== msgId || !msg.adjustments) return msg;
          const keyPath = `hsl.${color}.${channel}`;
          return { ...msg, appliedValues: { ...(msg.appliedValues || {}), [keyPath]: numericValue } };
        }),
      );
      setAdjustments((prev) =>
        mergeAdjustments(prev, {
          hsl: {
            [color]: {
              [channel]: numericValue,
            },
          },
        } as any),
      );
    },
    [setAdjustments],
  );

  const applyAllSuggestions = useCallback(
    (msg: ChatMessage) => {
      if (!msg.adjustments) return;
      const updates: Partial<Adjustments> = {};
      msg.adjustments.forEach((s) => {
        (updates as Record<string, any>)[s.key] = s.complex_value !== undefined ? s.complex_value : s.value;
      });
      setAdjustments((prev) => mergeAdjustments(prev, updates));
    },
    [setAdjustments],
  );

  const applyConstraintActions = useCallback(
    (msgId: string, actions: StyleConstraintAction[]) => {
      if (!actions.length) return;
      const patch: Record<string, number> = {};
      setAdjustments((prev) => {
        const next = { ...prev };
        actions.forEach((action) => {
          const current = Number(next[action.key as keyof Adjustments] ?? 0);
          const updated = clampAdjustmentValue(action.key, current + action.delta);
          (next as Record<string, number>)[action.key] = updated;
          patch[action.key] = updated;
        });
        return next;
      });
      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === msgId ? { ...msg, appliedValues: { ...(msg.appliedValues || {}), ...patch } } : msg,
        ),
      );
    },
    [setAdjustments],
  );

  const applyAssistantResult = useCallback(
    (msgId: string, result: ChatAdjustResponse) => {
      const updates: Partial<Adjustments> = {};
      result.adjustments.forEach((s) => {
        (updates as Record<string, any>)[s.key] = s.complex_value !== undefined ? s.complex_value : s.value;
      });
      if (Object.keys(updates).length > 0) {
        setAdjustments((prev) => mergeAdjustments(prev, updates));
      }

      if (result.outputImagePath && result.executionMeta?.resolvedMode === 'generative') {
        onOpenImage?.(result.outputImagePath, { preserveAdjustments: true });
      }

      setMessages((prev) =>
        prev.map((msg) =>
          msg.id === msgId
            ? {
                ...msg,
                content: result.understanding,
                adjustments: result.adjustments,
                appliedValues: Object.fromEntries(
                  result.adjustments.map((s) => [s.key, s.complex_value !== undefined ? s.complex_value : s.value]),
                ),
                styleDebug: result.style_debug,
                constraintDebug: result.constraint_debug ?? result.style_debug?.constraint_debug,
                executionMeta: result.executionMeta,
                serviceStatus: result.serviceStatus,
                outputImagePath: result.outputImagePath,
                previewImagePath: result.previewImagePath,
              }
            : msg,
        ),
      );
    },
    [onOpenImage, setAdjustments],
  );

  const isChatAvailable = ollamaStatus === 'online';

  const sendMessage = useCallback(async () => {
    const text = input.trim();
    if (!text || isLoading) return;
    if (!isChatAvailable) {
      setError(t('chat.chatUnavailable'));
      return;
    }

    setInput('');
    setError(null);

    const userMsg: ChatMessage = { id: crypto.randomUUID(), role: 'user', content: text };
    const streamMsgId = crypto.randomUUID();
    setMessages((prev) => [...prev, userMsg]);
    setIsLoading(true);

    // 创建一个流式占位消息
    const streamMsg: ChatMessage = {
      id: streamMsgId,
      role: 'assistant',
      content: '',
      thinkingContent: '',
    };
    setMessages((prev) => [...prev, streamMsg]);

    const history: LlmChatMessage[] = messages.map((m) => {
      if (m.role === 'assistant' && m.adjustments && m.adjustments.length > 0) {
        const formatApplied = (v: any) => (v !== null && typeof v === 'object' ? JSON.stringify(v) : String(v));
        const adjSummary = m.adjustments
          .map((a) => `${a.label}(${a.key}): ${formatApplied(m.appliedValues?.[a.key] ?? a.value)}`)
          .join(', ');
        return { role: m.role, content: `${m.content}\n[已应用调整: ${adjSummary}]` };
      }
      return { role: m.role, content: m.content };
    });

    // 监听流式事件
    const unlisten = await listen<StreamChunkPayload>('chat-stream-chunk', (event) => {
      const { chunk_type, text: chunkText, result } = event.payload;

      if (chunk_type === 'thinking') {
        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === streamMsgId ? { ...msg, thinkingContent: (msg.thinkingContent || '') + chunkText } : msg,
          ),
        );
      } else if (chunk_type === 'content') {
        setMessages((prev) =>
          prev.map((msg) => (msg.id === streamMsgId ? { ...msg, content: msg.content + chunkText } : msg)),
        );
      } else if (chunk_type === 'done' && result) {
        applyAssistantResult(streamMsgId, result);
      }
    });

    try {
      const simpleAdj = getSimpleAdjustments(adjustments);
      await invoke<ChatAdjustResponse>(Invokes.ChatAdjust, {
        message: text,
        history,
        currentAdjustments: simpleAdj,
        llmEndpoint: endpoint,
        llmApiKey: llmApiKey || null,
        llmModel: activeModel || null,
        currentImagePath: currentImagePath || null,
      });
    } catch (e) {
      setError(String(e));
      // 移除流式占位消息（如果没有内容）
      setMessages((prev) => prev.filter((msg) => msg.id !== streamMsgId || msg.content || msg.adjustments));
    } finally {
      unlisten();
      setIsLoading(false);
    }
  }, [
    input,
    isLoading,
    isChatAvailable,
    messages,
    adjustments,
    endpoint,
    llmApiKey,
    activeModel,
    currentImagePath,
    setAdjustments,
    t,
    applyAssistantResult,
  ]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const clearHistory = () => {
    setMessages([]);
    setError(null);
  };

  // 风格迁移：导入参考图并分析（流式显示思考过程）
  const handleStyleTransfer = useCallback(async () => {
    if (isLoading) return;
    if (!currentImagePath) {
      setError(t('chat.noImageOpen'));
      return;
    }

    let streamMsgId: string | null = null;

    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [
          {
            name: t('chat.imageFiles'),
            extensions: [
              'jpg',
              'jpeg',
              'png',
              'tiff',
              'tif',
              'webp',
              'bmp',
              'dng',
              'nef',
              'cr2',
              'cr3',
              'arw',
              'raf',
              'orf',
              'rw2',
            ],
          },
        ],
      });
      if (!selected) return;

      const refPath = typeof selected === 'string' ? selected : selected;
      if (!refPath) return;

      setError(null);
      setIsLoading(true);

      // 添加用户消息
      const userMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content:
          styleTransferMode === 'generative'
            ? t('chat.styleTransferRequestGenerative')
            : t('chat.styleTransferRequest'),
      };

      // 创建 AI 流式占位消息（立即显示等待动画）
      streamMsgId = crypto.randomUUID();
      const streamMsg: ChatMessage = {
        id: streamMsgId,
        role: 'assistant',
        content: '',
        thinkingContent: '',
      };
      setMessages((prev) => [...prev, userMsg, streamMsg]);

      const simpleAdj = getSimpleAdjustments(adjustments);
      const tuning = getEffectiveStyleTransferTuning();

      // 监听风格迁移流式事件
      const unlisten = await listen<StreamChunkPayload>('style-transfer-stream', (event) => {
        const { chunk_type, text: chunkText, result } = event.payload;

        if (chunk_type === 'thinking') {
          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === streamMsgId ? { ...msg, thinkingContent: (msg.thinkingContent || '') + chunkText } : msg,
            ),
          );
        } else if (chunk_type === 'content') {
          setMessages((prev) =>
            prev.map((msg) => (msg.id === streamMsgId ? { ...msg, content: msg.content + chunkText } : msg)),
          );
        } else if (chunk_type === 'done' && result) {
          applyAssistantResult(streamMsgId, result);
        }
      });

      try {
        const result = await invoke<ChatAdjustResponse>(Invokes.RunStyleTransfer, {
          request: {
            referencePath: refPath,
            currentImagePath,
            currentAdjustments: simpleAdj,
            mode: styleTransferMode,
            preset: styleTransferPreset,
            serviceUrl: normalizeServiceUrl(styleTransferServiceUrl),
            enableRefiner: styleTransferEnableRefiner,
            allowFallbackToAnalysis: styleTransferAllowFallback,
            styleStrength: tuning.styleStrength,
            highlightGuardStrength: tuning.highlightGuardStrength,
            skinProtectStrength: tuning.skinProtectStrength,
            pureAlgorithm: pureStyleTransfer,
            enableExpertPreset: enableStyleTransferExpertPreset,
            enableFeatureMapping: enableStyleTransferFeatureMapping,
            enableAutoRefine: enableStyleTransferAutoRefine,
            enableLut: enableStyleTransferLut,
            enableVlm: enableStyleTransferVlm,
            llmEndpoint: endpoint,
            llmApiKey: llmApiKey || null,
            llmModel: activeModel || null,
          },
        });

        applyAssistantResult(streamMsgId, result);
      } finally {
        unlisten();
      }
    } catch (e) {
      setError(String(e));
      if (streamMsgId) {
        setMessages((prev) => prev.filter((msg) => msg.id !== streamMsgId || msg.content || msg.adjustments));
      }
    } finally {
      setIsLoading(false);
    }
  }, [
    isLoading,
    currentImagePath,
    adjustments,
    setAdjustments,
    t,
    endpoint,
    llmApiKey,
    activeModel,
    getEffectiveStyleTransferTuning,
    styleTransferMode,
    styleTransferPreset,
    styleTransferServiceUrl,
    styleTransferEnableRefiner,
    styleTransferAllowFallback,
    pureStyleTransfer,
    enableStyleTransferExpertPreset,
    enableStyleTransferFeatureMapping,
    enableStyleTransferAutoRefine,
    enableStyleTransferLut,
    enableStyleTransferVlm,
    applyAssistantResult,
  ]);

  // 正常聊天界面
  return (
    <div className="flex flex-col h-full">
      {/* 顶部标题栏 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-surface">
        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
              ollamaStatus === 'online'
                ? 'bg-green-400'
                : ollamaStatus === 'checking'
                  ? 'bg-yellow-400 animate-pulse'
                  : 'bg-red-400'
            }`}
          />
          <span className="text-xs font-medium text-text-secondary">{t('chat.title')}</span>
        </div>
        <div className="flex items-center gap-1">
          {/* 模型选择器 */}
          <div className="relative" ref={modelMenuRef}>
            <button
              onClick={() => setModelMenuOpen((v) => !v)}
              disabled={!isChatAvailable}
              className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              title={t('chat.switchModel')}
            >
              <span className="max-w-[90px] truncate">{activeModel}</span>
              <ChevronDown size={10} className={`transition-transform ${modelMenuOpen ? 'rotate-180' : ''}`} />
            </button>
            {modelMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-52 bg-surface/95 backdrop-blur-md rounded-lg shadow-xl p-1.5 z-50 border border-surface">
                {PRESET_MODELS.map((m) => (
                  <button
                    key={m.value}
                    onClick={() => {
                      setActiveModel(m.value);
                      setModelMenuOpen(false);
                    }}
                    className={`w-full text-left px-2.5 py-2 rounded transition-colors hover:bg-bg-primary ${activeModel === m.value ? 'text-text-primary' : 'text-text-secondary'}`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-medium">{m.label}</span>
                      {activeModel === m.value && (
                        <span className="w-1.5 h-1.5 rounded-full bg-blue-400 flex-shrink-0" />
                      )}
                    </div>
                    <span className="text-[9px] opacity-60">{m.desc}</span>
                  </button>
                ))}
                <div className="border-t border-surface mt-1 pt-1">
                  <div className="flex gap-1 px-1">
                    <input
                      value={customModelInput}
                      onChange={(e) => setCustomModelInput(e.target.value)}
                      onKeyDown={(e) => {
                        e.stopPropagation();
                        if (e.key === 'Enter' && customModelInput.trim()) {
                          setActiveModel(customModelInput.trim());
                          setCustomModelInput('');
                          setModelMenuOpen(false);
                        }
                      }}
                      placeholder={t('chat.customModel')}
                      className="flex-1 bg-bg-primary rounded px-2 py-1 text-[10px] text-text-primary placeholder:text-text-secondary outline-none min-w-0"
                    />
                    <button
                      onClick={() => {
                        if (customModelInput.trim()) {
                          setActiveModel(customModelInput.trim());
                          setCustomModelInput('');
                          setModelMenuOpen(false);
                        }
                      }}
                      className="px-2 py-1 rounded bg-blue-500/20 text-blue-400 text-[10px] hover:bg-blue-500/30 transition-colors flex-shrink-0"
                    >
                      {t('common.add')}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
          <div className="relative" ref={tuningMenuRef}>
            <button
              onClick={() => setTuningMenuOpen((v) => !v)}
              className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
              title="风格迁移参数"
            >
              <SlidersHorizontal size={10} />
              <span>{t('chat.transferSettings')}</span>
              <ChevronDown size={10} className={`transition-transform ${tuningMenuOpen ? 'rotate-180' : ''}`} />
            </button>
            {tuningMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-72 bg-surface/95 backdrop-blur-md rounded-lg shadow-xl p-2 z-50 border border-surface space-y-2">
                <div className="space-y-1">
                  <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferModeLabel')}</div>
                  <div className="grid grid-cols-2 gap-1">
                    <button
                      onClick={() => {
                        setStyleTransferMode('analysis');
                        persistAppSettings({ styleTransferMode: 'analysis' });
                      }}
                      className={`rounded px-2 py-1.5 text-[10px] transition-colors ${
                        styleTransferMode === 'analysis'
                          ? 'bg-blue-500/20 text-blue-300'
                          : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {t('chat.styleTransferModeAnalysis')}
                    </button>
                    <button
                      onClick={() => {
                        setStyleTransferMode('generative');
                        persistAppSettings({ styleTransferMode: 'generative' });
                        checkStyleTransferService();
                      }}
                      className={`rounded px-2 py-1.5 text-[10px] transition-colors ${
                        styleTransferMode === 'generative'
                          ? 'bg-purple-500/20 text-purple-300'
                          : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {t('chat.styleTransferModeGenerative')}
                    </button>
                  </div>
                </div>

                {styleTransferMode === 'generative' ? (
                  <div className="space-y-2">
                    <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5">
                      <div className="flex items-center justify-between text-[9px] text-text-secondary">
                        <span>{t('chat.styleTransferServiceStatus')}</span>
                        <span
                          className={
                            styleTransferServiceStatus?.reachable && styleTransferServiceStatus.ready
                              ? 'text-green-300'
                              : 'text-amber-300'
                          }
                        >
                          {checkingStyleTransferService
                            ? t('chat.checking')
                            : styleTransferServiceStatus?.reachable && styleTransferServiceStatus.ready
                              ? t('chat.serviceReady')
                              : t('chat.serviceUnavailable')}
                        </span>
                      </div>
                      <div className="mt-1 text-[9px] text-text-secondary/75 break-all">
                        {styleTransferServiceStatus?.serviceUrl || normalizeServiceUrl(styleTransferServiceUrl)}
                      </div>
                      {styleTransferServiceStatus?.detail && (
                        <div className="mt-1 text-[9px] text-amber-300/80">{styleTransferServiceStatus.detail}</div>
                      )}
                    </div>

                    <label className="space-y-1 block">
                      <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferServiceUrl')}</span>
                      <div className="flex gap-1">
                        <input
                          type="text"
                          value={styleTransferServiceUrl}
                          onChange={(e) => setStyleTransferServiceUrl(e.target.value)}
                          onBlur={() => {
                            const normalized = normalizeServiceUrl(styleTransferServiceUrl);
                            setStyleTransferServiceUrl(normalized);
                            persistAppSettings({ styleTransferServiceUrl: normalized });
                            checkStyleTransferService(normalized);
                          }}
                          className="flex-1 bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
                        />
                        <button
                          onClick={() => checkStyleTransferService()}
                          className="px-2 py-1 rounded bg-bg-primary text-[10px] text-text-secondary hover:text-text-primary"
                        >
                          {t('chat.retry')}
                        </button>
                      </div>
                    </label>

                    <div className="space-y-1">
                      <div className="text-[9px] text-text-secondary/85">{t('chat.styleTransferPreset')}</div>
                      <div className="grid grid-cols-3 gap-1">
                        {STYLE_TRANSFER_PRESET_OPTIONS.map((option) => (
                          <button
                            key={option.value}
                            onClick={() => {
                              setStyleTransferPreset(option.value);
                              persistAppSettings({ styleTransferPreset: option.value });
                            }}
                            className={`rounded px-1.5 py-1.5 text-[10px] transition-colors ${
                              styleTransferPreset === option.value
                                ? 'bg-purple-500/20 text-purple-300'
                                : 'bg-bg-primary text-text-secondary hover:text-text-primary'
                            }`}
                          >
                            {option.label}
                          </button>
                        ))}
                      </div>
                    </div>

                    <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                      <span>{t('chat.styleTransferRefiner')}</span>
                      <input
                        type="checkbox"
                        checked={styleTransferEnableRefiner}
                        onChange={(e) => {
                          setStyleTransferEnableRefiner(e.target.checked);
                          persistAppSettings({ styleTransferEnableRefiner: e.target.checked });
                        }}
                        className="accent-purple-500"
                      />
                    </label>
                    <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                      <span>{t('chat.styleTransferFallback')}</span>
                      <input
                        type="checkbox"
                        checked={styleTransferAllowFallback}
                        onChange={(e) => {
                          setStyleTransferAllowFallback(e.target.checked);
                          persistAppSettings({ styleTransferAllowFallback: e.target.checked });
                        }}
                        className="accent-purple-500"
                      />
                    </label>
                    <div className="rounded border border-purple-400/15 bg-purple-500/5 px-2 py-1.5 text-[9px] text-text-secondary/75">
                      {t('chat.styleTransferOutputNote')}
                    </div>
                  </div>
                ) : (
                  <div className="space-y-2">
                    <label className="space-y-1 block">
                      <span className="text-[9px] text-text-secondary/85">{t('chat.styleTransferStrengthLabel')}</span>
                      <input
                        type="number"
                        min={0.5}
                        max={2.0}
                        step={0.05}
                        value={styleStrengthInput}
                        onChange={(e) => setStyleStrengthInput(e.target.value)}
                        onBlur={() =>
                          setStyleStrengthInput(saveStyleTransferConfig('styleTransferStrength', styleStrengthInput))
                        }
                        className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
                      />
                    </label>
                    <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                      <span>{t('chat.styleTransferPureAlgorithm')}</span>
                      <input
                        type="checkbox"
                        checked={pureStyleTransfer}
                        onChange={(e) => setPureStyleTransfer(e.target.checked)}
                        className="accent-blue-500"
                      />
                    </label>
                    {!pureStyleTransfer && (
                      <div className="space-y-1">
                        <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                          <span>{t('chat.styleTransferEnableLut')}</span>
                          <input
                            type="checkbox"
                            checked={enableStyleTransferLut}
                            onChange={(e) => setEnableStyleTransferLut(e.target.checked)}
                            className="accent-blue-500"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                          <span>{t('chat.styleTransferEnableExpertPreset')}</span>
                          <input
                            type="checkbox"
                            checked={enableStyleTransferExpertPreset}
                            onChange={(e) => setEnableStyleTransferExpertPreset(e.target.checked)}
                            className="accent-blue-500"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                          <span>{t('chat.styleTransferEnableFeatureMapping')}</span>
                          <input
                            type="checkbox"
                            checked={enableStyleTransferFeatureMapping}
                            onChange={(e) => setEnableStyleTransferFeatureMapping(e.target.checked)}
                            className="accent-blue-500"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                          <span>{t('chat.styleTransferEnableAutoRefine')}</span>
                          <input
                            type="checkbox"
                            checked={enableStyleTransferAutoRefine}
                            onChange={(e) => setEnableStyleTransferAutoRefine(e.target.checked)}
                            className="accent-blue-500"
                          />
                        </label>
                        <label className="flex items-center justify-between gap-2 text-[10px] text-text-secondary/85">
                          <span>{t('chat.styleTransferEnableVlm')}</span>
                          <input
                            type="checkbox"
                            checked={enableStyleTransferVlm}
                            onChange={(e) => setEnableStyleTransferVlm(e.target.checked)}
                            className="accent-blue-500"
                          />
                        </label>
                      </div>
                    )}
                    <label className="space-y-1 block">
                      <span className="text-[9px] text-text-secondary/85">
                        {t('chat.styleTransferHighlightGuardLabel')}
                      </span>
                      <input
                        type="number"
                        min={0.5}
                        max={2.0}
                        step={0.05}
                        value={highlightGuardInput}
                        onChange={(e) => setHighlightGuardInput(e.target.value)}
                        onBlur={() =>
                          setHighlightGuardInput(
                            saveStyleTransferConfig('styleTransferHighlightGuard', highlightGuardInput),
                          )
                        }
                        className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
                      />
                    </label>
                    <label className="space-y-1 block">
                      <span className="text-[9px] text-text-secondary/85">
                        {t('chat.styleTransferSkinProtectLabel')}
                      </span>
                      <input
                        type="number"
                        min={0.5}
                        max={2.0}
                        step={0.05}
                        value={skinProtectInput}
                        onChange={(e) => setSkinProtectInput(e.target.value)}
                        onBlur={() =>
                          setSkinProtectInput(saveStyleTransferConfig('styleTransferSkinProtect', skinProtectInput))
                        }
                        className="w-full bg-bg-primary rounded px-1.5 py-1 text-[10px] text-text-primary outline-none border border-surface focus:border-blue-500/50"
                      />
                    </label>
                  </div>
                )}
              </div>
            )}
          </div>
          {messages.length > 0 && (
            <button
              onClick={clearHistory}
              className="p-1 rounded hover:bg-surface text-text-secondary hover:text-text-primary transition-colors"
              title={t('chat.clearHistory')}
            >
              <RotateCcw size={13} />
            </button>
          )}
        </div>
      </div>

      {!isChatAvailable && (
        <div className="px-3 py-2 border-b border-surface bg-amber-500/10">
          <div className="flex items-center justify-between gap-3">
            <div className="space-y-0.5">
              <div className="text-[10px] text-amber-300">
                {ollamaStatus === 'checking' ? t('chat.checking') : t('chat.chatOfflineNotice')}
              </div>
              <div className="text-[9px] text-text-secondary/75">{t('chat.styleTransferAvailableWithoutLlm')}</div>
            </div>
            <button
              onClick={checkStatus}
              className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
              title={t('chat.retry')}
            >
              <RefreshCw size={10} />
              {t('chat.retry')}
            </button>
          </div>
        </div>
      )}

      {/* 消息列表 */}
      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-3 min-h-0">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center gap-3 py-8">
            <Bot size={28} className="text-text-secondary opacity-50" />
            <p className="text-xs text-text-secondary opacity-70 max-w-[180px]">{t('chat.placeholder')}</p>
            <p className="text-[10px] text-text-secondary/60 max-w-[220px]">
              {styleTransferMode === 'generative' ? t('chat.generativeModeHint') : t('chat.analysisModeHint')}
            </p>
            <button
              onClick={handleStyleTransfer}
              disabled={isLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-purple-500/15 hover:bg-purple-500/25 text-purple-400 text-[11px] transition-colors disabled:opacity-40"
            >
              <ImagePlus size={12} />
              {t('chat.importReference')}
            </button>
          </div>
        )}

        {messages.map((msg) => (
          <div key={msg.id} className={`flex gap-2 ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
            <div
              className={`flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center mt-0.5 ${msg.role === 'user' ? 'bg-blue-500/20' : 'bg-surface'}`}
            >
              {msg.role === 'user' ? (
                <User size={12} className="text-blue-400" />
              ) : (
                <Bot size={12} className="text-text-secondary" />
              )}
            </div>
            <div className={`flex flex-col gap-1.5 max-w-[85%] ${msg.role === 'user' ? 'items-end' : 'items-start'}`}>
              {/* 思考过程（可折叠） */}
              {msg.role === 'assistant' && msg.thinkingContent && (
                <ThinkingBlock content={msg.thinkingContent} isStreaming={isLoading && !msg.adjustments} />
              )}
              <div
                className={`px-2.5 py-1.5 rounded-lg text-xs leading-relaxed ${msg.role === 'user' ? 'bg-blue-500/20 text-text-primary' : 'bg-surface text-text-primary'}`}
              >
                {msg.content ||
                  (isLoading && !msg.adjustments ? (
                    <Loader2 size={12} className="animate-spin text-text-secondary" />
                  ) : null)}
              </div>
              {msg.role === 'assistant' && msg.outputImagePath && (
                <div className="w-full rounded-lg border border-surface bg-surface/40 px-2 py-1.5 space-y-1">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-text-secondary">{t('chat.styleTransferGenerated')}</span>
                    <button
                      onClick={() => onOpenImage?.(msg.outputImagePath!, { preserveAdjustments: true })}
                      className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
                    >
                      {t('chat.openGenerated')}
                    </button>
                  </div>
                  <div className="text-[9px] text-text-secondary/70">{t('chat.aiDisclaimer')}</div>
                </div>
              )}
              {msg.adjustments && msg.adjustments.length > 0 && (
                <div className="w-full bg-surface/50 rounded-lg p-2 space-y-2 border border-surface">
                  {msg.styleDebug && (
                    <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5 space-y-1">
                      <div className="flex items-center justify-between text-[9px] text-text-secondary">
                        <span>误差分解</span>
                        <span>
                          收敛 {(msg.styleDebug.improvement_ratio * 100).toFixed(1)}% · 当前短板{' '}
                          {msg.styleDebug.dominant_error}
                        </span>
                      </div>
                      <div className="text-[9px] text-text-secondary/70">
                        自动二次微调 {msg.styleDebug.auto_refine_rounds} 轮
                      </div>
                      {msg.styleDebug.scene_profile && (
                        <div className="rounded border border-blue-400/20 px-1.5 py-1 space-y-0.5">
                          <div className="text-[9px] text-blue-300/90">
                            场景判定 {msg.styleDebug.scene_profile.reference_tonal_style} →{' '}
                            {msg.styleDebug.scene_profile.current_tonal_style}
                          </div>
                          <div className="grid grid-cols-3 gap-2 text-[9px] text-text-secondary/80">
                            <span>明暗 {msg.styleDebug.scene_profile.tonal_gain.toFixed(2)}</span>
                            <span>高光 {msg.styleDebug.scene_profile.highlight_gain.toFixed(2)}</span>
                            <span>阴影 {msg.styleDebug.scene_profile.shadow_gain.toFixed(2)}</span>
                            <span>色彩上限 {msg.styleDebug.scene_profile.chroma_limit.toFixed(2)}</span>
                            <span>色彩护栏 {msg.styleDebug.scene_profile.chroma_guard_floor.toFixed(2)}</span>
                            <span>色准回正 {msg.styleDebug.scene_profile.color_residual_gain.toFixed(2)}</span>
                          </div>
                        </div>
                      )}
                      <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[9px] text-text-secondary/80">
                        <span>
                          接近度总分 {msg.styleDebug.proximity_before.overall.toFixed(1)} →{' '}
                          {msg.styleDebug.proximity_after.overall.toFixed(1)}
                        </span>
                        <span>
                          影调 {msg.styleDebug.proximity_before.tonal.toFixed(1)} →{' '}
                          {msg.styleDebug.proximity_after.tonal.toFixed(1)}
                        </span>
                        <span>
                          色彩 {msg.styleDebug.proximity_before.color.toFixed(1)} →{' '}
                          {msg.styleDebug.proximity_after.color.toFixed(1)}
                        </span>
                        <span>
                          肤色 {msg.styleDebug.proximity_before.skin.toFixed(1)} →{' '}
                          {msg.styleDebug.proximity_after.skin.toFixed(1)}
                        </span>
                        <span>
                          高光安全 {msg.styleDebug.proximity_before.highlight.toFixed(1)} →{' '}
                          {msg.styleDebug.proximity_after.highlight.toFixed(1)}
                        </span>
                      </div>
                      <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-[9px] text-text-secondary/80">
                        <span>
                          影调 {msg.styleDebug.before.tonal.toFixed(2)} → {msg.styleDebug.after.tonal.toFixed(2)}
                        </span>
                        <span>
                          色彩 {msg.styleDebug.before.color.toFixed(2)} → {msg.styleDebug.after.color.toFixed(2)}
                        </span>
                        <span>
                          肤色 {msg.styleDebug.before.skin.toFixed(2)} → {msg.styleDebug.after.skin.toFixed(2)}
                        </span>
                        <span>
                          过曝 {msg.styleDebug.before.highlight_penalty.toFixed(2)} →{' '}
                          {msg.styleDebug.after.highlight_penalty.toFixed(2)}
                        </span>
                      </div>
                      {msg.styleDebug.suggested_actions.length > 0 && (
                        <div className="space-y-0.5">
                          {msg.styleDebug.suggested_actions.map((action) => (
                            <div key={action.key} className="text-[9px] text-text-secondary/75">
                              {action.label} {action.recommended_delta >= 0 ? '+' : ''}
                              {action.recommended_delta.toFixed(action.key === 'exposure' ? 2 : 1)} · {action.reason}
                            </div>
                          ))}
                        </div>
                      )}
                      {(msg.styleDebug.blocked_items ?? []).length > 0 && (
                        <div className="space-y-1">
                          {(msg.styleDebug.blocked_items ?? []).map((item) => (
                            <div
                              key={`${item.category}-${item.reason}`}
                              className="rounded border border-amber-400/20 px-1.5 py-1"
                            >
                              <div className="flex items-center justify-between text-[9px]">
                                <span className="text-amber-300/90">
                                  {item.label} · 命中 {item.hit_count} 次 · 强度 {item.severity.toFixed(2)}
                                </span>
                                <button
                                  onClick={() => applyConstraintActions(msg.id, item.actions)}
                                  className="text-blue-300 hover:text-blue-200 transition-colors"
                                >
                                  一键微调
                                </button>
                              </div>
                              <div className="text-[9px] text-amber-300/80">受限原因：{item.reason}</div>
                            </div>
                          ))}
                        </div>
                      )}
                      {(msg.styleDebug.blocked_items ?? []).length === 0 &&
                        msg.styleDebug.blocked_reasons.length > 0 && (
                          <div className="space-y-0.5">
                            {msg.styleDebug.blocked_reasons.map((reason, idx) => (
                              <div key={`${reason}-${idx}`} className="text-[9px] text-amber-300/80">
                                受限原因：{reason}
                              </div>
                            ))}
                          </div>
                        )}
                    </div>
                  )}
                  {msg.constraintDebug && (
                    <div className="rounded border border-surface bg-bg-primary/60 px-2 py-1.5 space-y-1">
                      <div className="flex items-center justify-between text-[9px] text-text-secondary">
                        <span>动态约束</span>
                        <span>
                          来源 {msg.constraintDebug.window.source} · 命中 {msg.constraintDebug.clamp_count} 项
                        </span>
                      </div>
                      <div className="grid grid-cols-3 gap-2 text-[9px] text-text-secondary/80">
                        <span>高光风险 {msg.constraintDebug.window.highlight_risk.toFixed(2)}</span>
                        <span>阴影风险 {msg.constraintDebug.window.shadow_risk.toFixed(2)}</span>
                        <span>饱和风险 {msg.constraintDebug.window.saturation_risk.toFixed(2)}</span>
                      </div>
                      {msg.constraintDebug.clamps.slice(0, 3).map((clamp) => (
                        <div key={`${clamp.key}-${clamp.original}`} className="text-[9px] text-text-secondary/75">
                          {clamp.label} {clamp.original.toFixed(clamp.key === 'exposure' ? 2 : 1)} →{' '}
                          {clamp.clamped.toFixed(clamp.key === 'exposure' ? 2 : 1)} · {clamp.reason}
                        </div>
                      ))}
                    </div>
                  )}
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-text-secondary">{t('chat.suggestions')}</span>
                    <div className="flex items-center gap-3">
                      <button
                        onClick={() => {
                          const updates: Partial<Adjustments> = {};
                          msg.adjustments?.forEach((s) => {
                            (updates as Record<string, any>)[s.key] =
                              s.complex_value !== undefined ? s.complex_value : s.value;
                          });
                          applyAllSuggestions(msg);
                          const name = prompt('请输入预设名称', 'AI 预设');
                          if (name) addPreset(name, null, updates);
                        }}
                        className="flex items-center gap-1 text-[10px] text-purple-400 hover:text-purple-300 transition-colors"
                        title="将当前调整保存为预设"
                      >
                        <Save size={10} />
                        保存为预设
                      </button>
                      <button
                        onClick={() => applyAllSuggestions(msg)}
                        className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
                      >
                        {t('chat.applyAll')}
                      </button>
                    </div>
                  </div>
                  {msg.adjustments.map((s) => (
                    <div key={s.key} className="space-y-0.5">
                      {s.reason && <p className="text-[9px] text-text-secondary opacity-60">{s.reason}</p>}
                      {s.key === 'hsl' && s.complex_value !== undefined && typeof s.complex_value === 'object' ? (
                        <div className="rounded border border-surface bg-surface/30 px-2 py-1.5 space-y-1">
                          <div className="flex items-center justify-between">
                            <span className="text-[10px] text-text-primary">{s.label || '颜色混合器'}</span>
                            <span className="text-[9px] text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded">
                              HSL
                            </span>
                          </div>
                          {Object.entries(s.complex_value as Record<string, any>).map(([color, v]) => (
                            <div key={color} className="space-y-0.5">
                              <div className="text-[9px] text-text-secondary/80">
                                {HSL_COLOR_LABELS[color] || color}
                              </div>
                              <Slider
                                label="色相"
                                min={-100}
                                max={100}
                                step={1}
                                value={(adjustments.hsl as any)?.[color]?.hue ?? 0}
                                onChange={(e) => handleHslSliderChange(msg.id, color, 'hue', e)}
                              />
                              <Slider
                                label="饱和度"
                                min={-100}
                                max={100}
                                step={1}
                                value={(adjustments.hsl as any)?.[color]?.saturation ?? 0}
                                onChange={(e) => handleHslSliderChange(msg.id, color, 'saturation', e)}
                              />
                              <Slider
                                label="明度"
                                min={-100}
                                max={100}
                                step={1}
                                value={(adjustments.hsl as any)?.[color]?.luminance ?? 0}
                                onChange={(e) => handleHslSliderChange(msg.id, color, 'luminance', e)}
                              />
                            </div>
                          ))}
                        </div>
                      ) : s.complex_value !== undefined ? (
                        <div className="flex items-center justify-between bg-surface/50 rounded px-2 py-1.5 border border-surface">
                          <span className="text-[10px] text-text-primary">{s.label}</span>
                          <span className="text-[9px] text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded">
                            已应用高级映射
                          </span>
                        </div>
                      ) : (
                        <Slider
                          label={s.label}
                          min={s.min}
                          max={s.max}
                          step={s.key === 'exposure' ? 0.01 : 1}
                          value={
                            (adjustments[s.key as keyof Adjustments] as number) ?? msg.appliedValues?.[s.key] ?? s.value
                          }
                          onChange={(e) => handleSliderChange(msg.id, s.key, e)}
                        />
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        ))}

        {error && <div className="text-[10px] text-red-400 bg-red-500/10 rounded px-2 py-1.5">{error}</div>}
        <div ref={messagesEndRef} />
      </div>

      {/* 输入区域 */}
      <div className="px-3 py-2 border-t border-surface">
        <div className="flex gap-1.5 items-end">
          <button
            onClick={handleStyleTransfer}
            disabled={isLoading}
            className="flex-shrink-0 w-8 h-8 rounded-lg bg-purple-500/20 hover:bg-purple-500/30 disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center transition-colors"
            title={t('chat.importReference')}
          >
            <ImagePlus size={13} className="text-purple-400" />
          </button>
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('chat.inputPlaceholder')}
            rows={2}
            className="flex-1 resize-none bg-surface rounded-lg px-2.5 py-1.5 text-xs text-text-primary placeholder:text-text-secondary outline-none focus:ring-1 focus:ring-blue-500/50 min-h-[52px] max-h-[120px]"
            disabled={isLoading}
          />
          <button
            onClick={sendMessage}
            disabled={!input.trim() || isLoading}
            className="flex-shrink-0 w-8 h-8 rounded-lg bg-blue-500/20 hover:bg-blue-500/30 disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center transition-colors"
          >
            <Send size={13} className="text-blue-400" />
          </button>
        </div>
        <p className="text-[9px] text-text-secondary opacity-50 mt-1">{t('chat.enterToSend')}</p>
      </div>
    </div>
  );
}
