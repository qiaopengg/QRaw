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
  ExternalLink,
  RefreshCw,
  ImagePlus,
  Brain,
  ChevronRight,
} from 'lucide-react';
import { Invokes } from '../../ui/AppProperties';
import { Adjustments } from '../../../utils/adjustments';
import Slider from '../../ui/Slider';

const DEFAULT_MODEL = 'qwen3.5:9b';

const PRESET_MODELS = [
  { label: 'qwen3.5:9b ⭐', value: 'qwen3.5:9b', desc: '推荐 · 最强中文理解 · 需16GB内存' },
  { label: 'qwen3.5:4b', value: 'qwen3.5:4b', desc: '轻量 · 8GB内存可用' },
  { label: 'qwen3.5:14b', value: 'qwen3.5:14b', desc: '高性能 · 需24GB内存' },
  { label: 'qwen2.5:7b', value: 'qwen2.5:7b', desc: '上一代 · 稳定可靠' },
  { label: 'deepseek-r1:7b', value: 'deepseek-r1:7b', desc: '推理增强' },
  { label: 'llama3.2:3b', value: 'llama3.2:3b', desc: '英文场景' },
];

interface AdjustmentSuggestion {
  key: string;
  value: number;
  label: string;
  min: number;
  max: number;
  reason: string;
}

interface ChatAdjustResponse {
  understanding: string;
  adjustments: AdjustmentSuggestion[];
}

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  thinkingContent?: string;
  adjustments?: AdjustmentSuggestion[];
  appliedValues?: Record<string, number>;
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
  currentImagePath?: string | null;
}

type OllamaStatus = 'checking' | 'online' | 'offline';

function getSimpleAdjustments(adj: Adjustments): Record<string, number> {
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
  };
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

  // 流式结束后自动折叠
  useEffect(() => {
    if (!isStreaming) {
      const timer = setTimeout(() => setExpanded(false), 800);
      return () => clearTimeout(timer);
    }
  }, [isStreaming]);

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
  currentImagePath,
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
  const [customModelInput, setCustomModelInput] = useState('');
  const modelMenuRef = useRef<HTMLDivElement>(null);

  // 检测 Ollama 状态
  const checkStatus = useCallback(async () => {
    setOllamaStatus('checking');
    const online = await checkOllamaStatus(endpoint);
    setOllamaStatus(online ? 'online' : 'offline');
  }, [endpoint]);

  useEffect(() => {
    checkStatus();
  }, [checkStatus]);

  // 点击外部关闭模型菜单
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (modelMenuRef.current && !modelMenuRef.current.contains(e.target as Node)) {
        setModelMenuOpen(false);
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

  const applyAllSuggestions = useCallback(
    (msg: ChatMessage) => {
      if (!msg.adjustments) return;
      const updates: Partial<Adjustments> = {};
      msg.adjustments.forEach((s) => {
        (updates as Record<string, number>)[s.key] = s.value;
      });
      setAdjustments((prev) => ({ ...prev, ...updates }));
    },
    [setAdjustments],
  );

  const sendMessage = useCallback(async () => {
    const text = input.trim();
    if (!text || isLoading) return;

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
        const adjSummary = m.adjustments
          .map((a) => `${a.label}(${a.key}): ${m.appliedValues?.[a.key] ?? a.value}`)
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
        // 流结束，更新最终结果
        const updates: Partial<Adjustments> = {};
        result.adjustments.forEach((s) => {
          (updates as Record<string, number>)[s.key] = s.value;
        });
        if (Object.keys(updates).length > 0) setAdjustments((prev) => ({ ...prev, ...updates }));

        setMessages((prev) =>
          prev.map((msg) =>
            msg.id === streamMsgId
              ? {
                  ...msg,
                  content: result.understanding,
                  adjustments: result.adjustments,
                  appliedValues: Object.fromEntries(result.adjustments.map((s) => [s.key, s.value])),
                }
              : msg,
          ),
        );
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
      });
    } catch (e) {
      setError(String(e));
      // 移除流式占位消息（如果没有内容）
      setMessages((prev) => prev.filter((msg) => msg.id !== streamMsgId || msg.content || msg.adjustments));
    } finally {
      unlisten();
      setIsLoading(false);
    }
  }, [input, isLoading, messages, adjustments, endpoint, llmApiKey, activeModel, setAdjustments]);

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

    try {
      const selected = await openFileDialog({
        multiple: false,
        filters: [{ name: t('chat.imageFiles'), extensions: ['jpg', 'jpeg', 'png', 'tiff', 'tif', 'webp', 'bmp'] }],
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
        content: t('chat.styleTransferRequest'),
      };

      // 创建 AI 流式占位消息（立即显示等待动画）
      const streamMsgId = crypto.randomUUID();
      const streamMsg: ChatMessage = {
        id: streamMsgId,
        role: 'assistant',
        content: '',
        thinkingContent: '',
      };
      setMessages((prev) => [...prev, userMsg, streamMsg]);

      const simpleAdj = getSimpleAdjustments(adjustments);

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
          // 流结束，自动应用结果
          const updates: Partial<Adjustments> = {};
          result.adjustments.forEach((s) => {
            (updates as Record<string, number>)[s.key] = s.value;
          });
          if (Object.keys(updates).length > 0) setAdjustments((prev) => ({ ...prev, ...updates }));

          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === streamMsgId
                ? {
                    ...msg,
                    content: result.understanding,
                    adjustments: result.adjustments,
                    appliedValues: Object.fromEntries(result.adjustments.map((s) => [s.key, s.value])),
                  }
                : msg,
            ),
          );
        }
      });

      try {
        // Ollama 在线时用 LLM 增强版（流式），否则用纯算法版
        if (ollamaStatus === 'online') {
          await invoke<ChatAdjustResponse>(Invokes.AnalyzeStyleTransferWithLlm, {
            referencePath: refPath,
            currentImagePath: currentImagePath,
            currentAdjustments: simpleAdj,
            llmEndpoint: endpoint,
            llmApiKey: llmApiKey || null,
            llmModel: activeModel || null,
          });
        } else {
          const result = await invoke<ChatAdjustResponse>(Invokes.AnalyzeStyleTransfer, {
            referencePath: refPath,
            currentImagePath: currentImagePath,
            currentAdjustments: simpleAdj,
          });

          // 纯算法版没有流式，直接更新
          const updates: Partial<Adjustments> = {};
          result.adjustments.forEach((s) => {
            (updates as Record<string, number>)[s.key] = s.value;
          });
          if (Object.keys(updates).length > 0) setAdjustments((prev) => ({ ...prev, ...updates }));

          setMessages((prev) =>
            prev.map((msg) =>
              msg.id === streamMsgId
                ? {
                    ...msg,
                    content: result.understanding,
                    adjustments: result.adjustments,
                    appliedValues: Object.fromEntries(result.adjustments.map((s) => [s.key, s.value])),
                    thinkingContent: '',
                  }
                : msg,
            ),
          );
        }
      } finally {
        unlisten();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, [isLoading, currentImagePath, adjustments, setAdjustments, t, ollamaStatus, endpoint, llmApiKey, activeModel]);

  // 离线引导页
  if (ollamaStatus === 'offline') {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center justify-between px-3 py-2 border-b border-surface">
          <span className="text-xs font-medium text-text-secondary">{t('chat.title')}</span>
          <button
            onClick={checkStatus}
            className="p-1 rounded hover:bg-surface text-text-secondary hover:text-text-primary transition-colors"
            title={t('chat.retry')}
          >
            <RefreshCw size={13} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
          {/* 状态提示 */}
          <div className="flex items-center gap-2 p-3 bg-yellow-500/10 rounded-lg border border-yellow-500/20">
            <div className="w-2 h-2 rounded-full bg-yellow-400 flex-shrink-0" />
            <span className="text-[11px] text-yellow-400">{t('chat.ollamaOffline')}</span>
          </div>

          {/* 步骤 1：安装 Ollama */}
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-blue-500/20 text-blue-400 text-[10px] flex items-center justify-center font-bold flex-shrink-0">
                1
              </span>
              <span className="text-xs font-medium text-text-primary">{t('chat.step1Title')}</span>
            </div>
            <p className="text-[11px] text-text-secondary ml-7">{t('chat.step1Desc')}</p>
            <a
              href="https://ollama.com/download"
              target="_blank"
              rel="noopener noreferrer"
              className="ml-7 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-500/20 hover:bg-blue-500/30 text-blue-400 text-[11px] transition-colors"
            >
              <ExternalLink size={11} />
              ollama.com/download
            </a>
          </div>

          {/* 步骤 2：拉取模型 */}
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-blue-500/20 text-blue-400 text-[10px] flex items-center justify-center font-bold flex-shrink-0">
                2
              </span>
              <span className="text-xs font-medium text-text-primary">{t('chat.step2Title')}</span>
            </div>
            <p className="text-[11px] text-text-secondary ml-7">{t('chat.step2Desc')}</p>
            <div className="ml-7 bg-bg-primary rounded-lg p-2.5 border border-surface">
              <code className="text-[11px] text-green-400 font-mono">ollama pull {DEFAULT_MODEL}</code>
            </div>
            <p className="text-[10px] text-text-secondary opacity-60 ml-7">{t('chat.step2Note')}</p>
          </div>

          {/* 步骤 3：重试 */}
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <span className="w-5 h-5 rounded-full bg-blue-500/20 text-blue-400 text-[10px] flex items-center justify-center font-bold flex-shrink-0">
                3
              </span>
              <span className="text-xs font-medium text-text-primary">{t('chat.step3Title')}</span>
            </div>
            <button
              onClick={checkStatus}
              className="ml-7 flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface hover:bg-bg-primary text-text-primary text-[11px] transition-colors border border-surface"
            >
              <RefreshCw size={11} />
              {t('chat.checkAgain')}
            </button>
          </div>

          {/* 模型说明 */}
          <div className="p-3 bg-surface/50 rounded-lg space-y-1.5">
            <p className="text-[10px] font-medium text-text-secondary">{t('chat.modelInfo')}</p>
            {PRESET_MODELS.slice(0, 3).map((m) => (
              <div key={m.value} className="flex items-start gap-2">
                <code className="text-[10px] text-blue-400 font-mono flex-shrink-0">{m.value}</code>
                <span className="text-[10px] text-text-secondary opacity-70">{m.desc}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // 检测中
  if (ollamaStatus === 'checking') {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-2">
        <Loader2 size={18} className="animate-spin text-text-secondary" />
        <span className="text-[11px] text-text-secondary">{t('chat.checking')}</span>
      </div>
    );
  }

  // 正常聊天界面
  return (
    <div className="flex flex-col h-full">
      {/* 顶部标题栏 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-surface">
        <div className="flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-green-400 flex-shrink-0" />
          <span className="text-xs font-medium text-text-secondary">{t('chat.title')}</span>
        </div>
        <div className="flex items-center gap-1">
          {/* 模型选择器 */}
          <div className="relative" ref={modelMenuRef}>
            <button
              onClick={() => setModelMenuOpen((v) => !v)}
              className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-text-secondary hover:text-text-primary hover:bg-surface transition-colors"
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

      {/* 消息列表 */}
      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-3 min-h-0">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center gap-3 py-8">
            <Bot size={28} className="text-text-secondary opacity-50" />
            <p className="text-xs text-text-secondary opacity-70 max-w-[180px]">{t('chat.placeholder')}</p>
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
              {msg.adjustments && msg.adjustments.length > 0 && (
                <div className="w-full bg-surface/50 rounded-lg p-2 space-y-2 border border-surface">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-text-secondary">{t('chat.suggestions')}</span>
                    <button
                      onClick={() => applyAllSuggestions(msg)}
                      className="text-[10px] text-blue-400 hover:text-blue-300 transition-colors"
                    >
                      {t('chat.applyAll')}
                    </button>
                  </div>
                  {msg.adjustments.map((s) => (
                    <div key={s.key} className="space-y-0.5">
                      {s.reason && <p className="text-[9px] text-text-secondary opacity-60">{s.reason}</p>}
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
