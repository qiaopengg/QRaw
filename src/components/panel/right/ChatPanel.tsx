import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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
} from 'lucide-react';
import { AppSettings, Invokes } from '../../ui/AppProperties';
import { Adjustments } from '../../../utils/adjustments';
import { usePresets } from '../../../hooks/usePresets';
import {
  ChatAdjustResponse,
  ChatMessage,
  ChatOpenImageOptions,
  LlmChatMessage,
  OllamaStatus,
  StreamChunkPayload,
} from './chat/types';
import { getSimpleAdjustments } from './chat/styleTransfer/utils';
import { useStyleTransfer } from './chat/styleTransfer/useStyleTransfer';
import { useStyleTransferMessageActions } from './chat/styleTransfer/useStyleTransferMessageActions';
import { StyleTransferSettings } from './chat/styleTransfer/StyleTransferSettings';
import { StyleTransferReferenceSelectionCard } from './chat/styleTransfer/StyleTransferReferenceSelectionCard';
import { StyleTransferResultCard } from './chat/styleTransfer/StyleTransferResultCard';
import { StyleTransferSuggestionsCard } from './chat/styleTransfer/StyleTransferSuggestionsCard';

const DEFAULT_MODEL = 'auto';

const PRESET_MODELS = [
  { label: 'auto（自动路由）⭐', value: 'auto', desc: '自然语言修图→qwen3.5:9b，风格迁移→qwen3.6:27b' },
  { label: 'qwen3.6:27b ⭐⭐', value: 'qwen3.6:27b', desc: '最新视觉模型 · 强大的图像理解 · 需32GB内存' },
  { label: 'qwen3.5:9b ⭐', value: 'qwen3.5:9b', desc: '推荐 · 最强中文理解 · 需16GB内存' },
  { label: 'qwen2.5vl:7b', value: 'qwen2.5vl:7b', desc: '视觉理解 · 适合风格迁移' },
  { label: 'qwen3.5:4b', value: 'qwen3.5:4b', desc: '轻量 · 8GB内存可用' },
  { label: 'qwen3.5:14b', value: 'qwen3.5:14b', desc: '高性能 · 需24GB内存' },
  { label: 'qwen2.5:7b', value: 'qwen2.5:7b', desc: '上一代 · 稳定可靠' },
  { label: 'deepseek-r1:7b', value: 'deepseek-r1:7b', desc: '推理增强' },
  { label: 'llama3.2:3b', value: 'llama3.2:3b', desc: '英文场景' },
];

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
  onOpenImage?(path: string, options?: ChatOpenImageOptions): void;
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

function StyleTransferProgressBlock({ description, percentage }: { description: string; percentage: number }) {
  const safePercentage = Math.max(0, Math.min(100, percentage));

  return (
    <div className="w-full rounded-lg border border-purple-500/20 bg-purple-500/5 px-2 py-1.5 space-y-1">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[10px] text-purple-200/90 truncate">{description}</span>
        <span className="text-[10px] text-purple-300 tabular-nums">{safePercentage}%</span>
      </div>
      <div className="h-1.5 rounded-full bg-surface overflow-hidden">
        <div
          className="h-full rounded-full bg-gradient-to-r from-purple-500 to-blue-400 transition-[width] duration-300"
          style={{ width: `${safePercentage}%` }}
        />
      </div>
    </div>
  );
}

export default function ChatPanel({
  adjustments,
  setAdjustments,
  llmEndpoint,
  llmApiKey,
  llmModel,
  styleTransferStrength: _styleTransferStrength,
  styleTransferHighlightGuard: _styleTransferHighlightGuard,
  styleTransferSkinProtect: _styleTransferSkinProtect,
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
  const modelMenuRef = useRef<HTMLDivElement>(null);
  const tuningMenuRef = useRef<HTMLDivElement>(null);

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

  const {
    applyAllSuggestions,
    applyAssistantResult,
    applyConstraintActions,
    applyStyleTransferPreview,
    discardStyleTransferPreview,
    handleHslSliderChange,
    handleSliderChange,
    showStyleTransferPreview,
    showStyleTransferSource,
    toggleStyleTransferCompare,
  } = useStyleTransferMessageActions({
    adjustments,
    onOpenImage,
    setAdjustments,
    setMessages,
  });

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
        const formatApplied = (v: unknown) => (v !== null && typeof v === 'object' ? JSON.stringify(v) : String(v));
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
  const {
    cancelPendingStyleTransferSelection,
    confirmPendingStyleTransferSelection,
    enableStyleTransferAutoRefine,
    enableStyleTransferExpertPreset,
    enableStyleTransferFeatureMapping,
    enableStyleTransferLut,
    enableStyleTransferVlm,
    handleStyleTransfer,
    highlightGuardInput,
    isPreparingStyleTransferModels,
    prepareStyleTransferModels,
    pureStyleTransfer,
    saveStyleTransferConfig,
    setEnableStyleTransferAutoRefine,
    setEnableStyleTransferExpertPreset,
    setEnableStyleTransferFeatureMapping,
    setEnableStyleTransferLut,
    setEnableStyleTransferVlm,
    setHighlightGuardInput,
    setPureStyleTransfer,
    setSkinProtectInput,
    setStyleStrengthInput,
    skinProtectInput,
    styleStrengthInput,
    pendingStyleTransferSelection,
    styleTransferModelStatus,
    styleTransferPreset,
    styleTransferStrategyMode,
    updateStyleTransferStrategyMode,
    updateStyleTransferPreset,
  } = useStyleTransfer({
    activeModel,
    adjustments,
    appSettings,
    applyAssistantResult,
    currentImagePath,
    endpoint,
    isLoading,
    llmApiKey,
    persistAppSettings,
    setError,
    setIsLoading,
    setMessages,
    t,
  });

  // 正常聊天界面
  return (
    <div className="flex flex-col h-full">
      {/* 顶部标题栏 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-surface">
        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
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
                      {activeModel === m.value && <span className="w-1.5 h-1.5 rounded-full bg-blue-400 shrink-0" />}
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
                      className="px-2 py-1 rounded bg-blue-500/20 text-blue-400 text-[10px] hover:bg-blue-500/30 transition-colors shrink-0"
                    >
                      {t('common.add')}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
          <StyleTransferSettings
            enableStyleTransferAutoRefine={enableStyleTransferAutoRefine}
            enableStyleTransferExpertPreset={enableStyleTransferExpertPreset}
            enableStyleTransferFeatureMapping={enableStyleTransferFeatureMapping}
            enableStyleTransferLut={enableStyleTransferLut}
            enableStyleTransferVlm={enableStyleTransferVlm}
            highlightGuardInput={highlightGuardInput}
            menuOpen={tuningMenuOpen}
            menuRef={tuningMenuRef}
            pureStyleTransfer={pureStyleTransfer}
            saveStyleTransferConfig={saveStyleTransferConfig}
            setEnableStyleTransferAutoRefine={setEnableStyleTransferAutoRefine}
            setEnableStyleTransferExpertPreset={setEnableStyleTransferExpertPreset}
            setEnableStyleTransferFeatureMapping={setEnableStyleTransferFeatureMapping}
            setEnableStyleTransferLut={setEnableStyleTransferLut}
            setEnableStyleTransferVlm={setEnableStyleTransferVlm}
            setHighlightGuardInput={setHighlightGuardInput}
            setMenuOpen={setTuningMenuOpen}
            setPureStyleTransfer={setPureStyleTransfer}
            setSkinProtectInput={setSkinProtectInput}
            setStyleStrengthInput={setStyleStrengthInput}
            skinProtectInput={skinProtectInput}
            isPreparingStyleTransferModels={isPreparingStyleTransferModels}
            prepareStyleTransferModels={prepareStyleTransferModels}
            styleStrengthInput={styleStrengthInput}
            styleTransferModelStatus={styleTransferModelStatus}
            styleTransferPreset={styleTransferPreset}
            styleTransferStrategyMode={styleTransferStrategyMode}
            updateStyleTransferStrategyMode={updateStyleTransferStrategyMode}
            updateStyleTransferPreset={updateStyleTransferPreset}
          />
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
            <p className="text-[10px] text-text-secondary/60 max-w-[220px]">{t('chat.analysisModeHint')}</p>
            <button
              onClick={() => handleStyleTransfer()}
              disabled={isLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-500/15 hover:bg-blue-500/25 text-blue-300 text-[11px] transition-colors disabled:opacity-40"
            >
              <ImagePlus size={12} />
              {t('chat.importReference')}
            </button>
          </div>
        )}

        {pendingStyleTransferSelection && (
          <StyleTransferReferenceSelectionCard
            auxReferencePaths={pendingStyleTransferSelection.auxReferencePaths}
            mainReferencePath={pendingStyleTransferSelection.mainReferencePath}
            onCancel={cancelPendingStyleTransferSelection}
            onConfirm={(styleTransferType) => void confirmPendingStyleTransferSelection(styleTransferType)}
          />
        )}

        {messages.map((msg) => (
          <div key={msg.id} className={`flex gap-2 ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
            <div
              className={`shrink-0 w-6 h-6 rounded-full flex items-center justify-center mt-0.5 ${msg.role === 'user' ? 'bg-blue-500/20' : 'bg-surface'}`}
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
              {msg.role === 'assistant' && msg.styleTransferProgress && (
                <StyleTransferProgressBlock
                  description={msg.styleTransferProgress.description}
                  percentage={msg.styleTransferProgress.percentage}
                />
              )}
              <div
                className={`px-2.5 py-1.5 rounded-lg text-xs leading-relaxed ${msg.role === 'user' ? 'bg-blue-500/20 text-text-primary' : 'bg-surface text-text-primary'}`}
              >
                {msg.content ||
                  (isLoading && !msg.adjustments ? (
                    <Loader2 size={12} className="animate-spin text-text-secondary" />
                  ) : null)}
              </div>
              {msg.role === 'assistant' && (
                <StyleTransferResultCard
                  onApplyPreview={applyStyleTransferPreview}
                  onDiscardPreview={discardStyleTransferPreview}
                  isLoading={isLoading}
                  message={msg}
                  onShowPreview={showStyleTransferPreview}
                  onShowSource={showStyleTransferSource}
                  onToggleCompare={toggleStyleTransferCompare}
                />
              )}
              <StyleTransferSuggestionsCard
                addPreset={addPreset}
                adjustments={adjustments}
                applyAllSuggestions={applyAllSuggestions}
                applyConstraintActions={applyConstraintActions}
                handleHslSliderChange={handleHslSliderChange}
                handleSliderChange={handleSliderChange}
                message={msg}
                t={t}
              />
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
            onClick={() => handleStyleTransfer()}
            disabled={isLoading}
            className="shrink-0 w-8 h-8 rounded-lg bg-purple-500/20 hover:bg-purple-500/30 disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center transition-colors"
            title={`${t('chat.importReference')} · ${t('chat.styleTransferAnalysisCost')}`}
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
            className="shrink-0 w-8 h-8 rounded-lg bg-blue-500/20 hover:bg-blue-500/30 disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center transition-colors"
          >
            <Send size={13} className="text-blue-400" />
          </button>
        </div>
        <p className="text-[9px] text-text-secondary opacity-50 mt-1">{t('chat.enterToSend')}</p>
      </div>
    </div>
  );
}
