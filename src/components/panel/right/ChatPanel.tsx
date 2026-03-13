import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Send, Loader2, Bot, User, RotateCcw } from 'lucide-react';
import { Invokes } from '../../ui/AppProperties';
import { Adjustments } from '../../../utils/adjustments';
import Slider from '../../ui/Slider';

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
  adjustments?: AdjustmentSuggestion[];
  appliedValues?: Record<string, number>;
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
}

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

export default function ChatPanel({ adjustments, setAdjustments, llmEndpoint, llmApiKey, llmModel }: ChatPanelProps) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const endpoint = llmEndpoint || 'http://localhost:11434';

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSliderChange = useCallback(
    (msgId: string, key: string, value: number) => {
      setMessages((prev) =>
        prev.map((msg) => {
          if (msg.id !== msgId || !msg.adjustments) return msg;
          return {
            ...msg,
            appliedValues: { ...(msg.appliedValues || {}), [key]: value },
          };
        }),
      );
      setAdjustments((prev) => ({ ...prev, [key]: value }));
    },
    [setAdjustments],
  );

  const applyAllSuggestions = useCallback(
    (msg: ChatMessage) => {
      if (!msg.adjustments) return;
      const updates: Partial<Adjustments> = {};
      msg.adjustments.forEach((s) => {
        const val = msg.appliedValues?.[s.key] ?? s.value;
        (updates as Record<string, number>)[s.key] = val;
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

    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: text,
    };
    setMessages((prev) => [...prev, userMsg]);
    setIsLoading(true);

    // 构建历史（只传文本消息）
    const history: LlmChatMessage[] = messages.map((m) => ({
      role: m.role,
      content: m.content,
    }));

    try {
      const simpleAdj = getSimpleAdjustments(adjustments);
      const result = await invoke<ChatAdjustResponse>(Invokes.ChatAdjust, {
        message: text,
        history,
        currentAdjustments: simpleAdj,
        llmEndpoint: endpoint,
        llmApiKey: llmApiKey || null,
        llmModel: llmModel || null,
      });

      // 立即应用建议到调整面板
      const updates: Partial<Adjustments> = {};
      result.adjustments.forEach((s) => {
        (updates as Record<string, number>)[s.key] = s.value;
      });
      if (Object.keys(updates).length > 0) {
        setAdjustments((prev) => ({ ...prev, ...updates }));
      }

      const assistantMsg: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: result.understanding,
        adjustments: result.adjustments,
        appliedValues: Object.fromEntries(result.adjustments.map((s) => [s.key, s.value])),
      };
      setMessages((prev) => [...prev, assistantMsg]);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }, [input, isLoading, messages, adjustments, endpoint, llmApiKey, llmModel, setAdjustments]);

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

  const isConfigured = !!endpoint;

  return (
    <div className="flex flex-col h-full">
      {/* 顶部标题栏 */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-surface">
        <span className="text-xs font-medium text-text-secondary">{t('chat.title')}</span>
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

      {/* 消息列表 */}
      <div className="flex-1 overflow-y-auto px-3 py-2 space-y-3 min-h-0">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center gap-2 py-8">
            <Bot size={28} className="text-text-secondary opacity-50" />
            <p className="text-xs text-text-secondary opacity-70 max-w-[180px]">{t('chat.placeholder')}</p>
          </div>
        )}

        {messages.map((msg) => (
          <div key={msg.id} className={`flex gap-2 ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
            <div
              className={`flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center mt-0.5 ${
                msg.role === 'user' ? 'bg-blue-500/20' : 'bg-surface'
              }`}
            >
              {msg.role === 'user' ? (
                <User size={12} className="text-blue-400" />
              ) : (
                <Bot size={12} className="text-text-secondary" />
              )}
            </div>
            <div className={`flex flex-col gap-1.5 max-w-[85%] ${msg.role === 'user' ? 'items-end' : 'items-start'}`}>
              <div
                className={`px-2.5 py-1.5 rounded-lg text-xs leading-relaxed ${
                  msg.role === 'user' ? 'bg-blue-500/20 text-text-primary' : 'bg-surface text-text-primary'
                }`}
              >
                {msg.content}
              </div>

              {/* 调整建议滑块 */}
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
                      <div className="flex items-center justify-between">
                        <span className="text-[10px] text-text-primary">{s.label}</span>
                        <span className="text-[10px] text-text-secondary tabular-nums">
                          {(msg.appliedValues?.[s.key] ?? s.value).toFixed(s.key === 'exposure' ? 2 : 0)}
                        </span>
                      </div>
                      <Slider
                        min={s.min}
                        max={s.max}
                        step={s.key === 'exposure' ? 0.01 : 1}
                        value={msg.appliedValues?.[s.key] ?? s.value}
                        onChange={(v) => handleSliderChange(msg.id, s.key, v)}
                      />
                      {s.reason && <p className="text-[9px] text-text-secondary opacity-60">{s.reason}</p>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        ))}

        {isLoading && (
          <div className="flex gap-2">
            <div className="flex-shrink-0 w-6 h-6 rounded-full bg-surface flex items-center justify-center">
              <Bot size={12} className="text-text-secondary" />
            </div>
            <div className="px-2.5 py-1.5 rounded-lg bg-surface">
              <Loader2 size={12} className="animate-spin text-text-secondary" />
            </div>
          </div>
        )}

        {error && <div className="text-[10px] text-red-400 bg-red-500/10 rounded px-2 py-1.5">{error}</div>}

        <div ref={messagesEndRef} />
      </div>

      {/* 输入区域 */}
      <div className="px-3 py-2 border-t border-surface">
        {!isConfigured && <p className="text-[10px] text-yellow-400 mb-1.5">{t('chat.notConfigured')}</p>}
        <div className="flex gap-1.5 items-end">
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
