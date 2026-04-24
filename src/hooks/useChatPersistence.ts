import { useCallback, useEffect, useRef } from 'react';
import { ChatMessage } from '../components/panel/right/chat/types';

/**
 * 聊天持久化存储
 * - 按图片路径管理对话历史
 * - 存储在内存中，只在退出系统时清空
 * - 切换图片时自动加载/重置对话
 */

interface ChatHistoryEntry {
  imagePath: string;
  messages: ChatMessage[];
  lastUpdated: number;
}

interface ChatPersistenceConfig {
  llmModel?: string; // 添加模型选择
  styleTransferStrength?: number;
  styleTransferHighlightGuard?: number;
  styleTransferSkinProtect?: number;
  styleTransferPreset?: 'realistic' | 'artistic' | 'creative';
  styleTransferStrategyMode?: 'safe' | 'strong';
  pureStyleTransfer?: boolean;
  enableStyleTransferLut?: boolean;
  enableStyleTransferExpertPreset?: boolean;
  enableStyleTransferFeatureMapping?: boolean;
  enableStyleTransferAutoRefine?: boolean;
  enableStyleTransferVlm?: boolean;
}

// 全局存储（内存中，应用生命周期内持久）
const chatHistoryStore = new Map<string, ChatHistoryEntry>();
const configStore: { current: ChatPersistenceConfig } = { current: {} };

export function useChatPersistence(currentImagePath: string | null | undefined) {
  const previousImagePathRef = useRef<string | null>(null);

  /**
   * 保存当前图片的对话历史
   */
  const saveChatHistory = useCallback((imagePath: string, messages: ChatMessage[]) => {
    if (!imagePath) return;

    chatHistoryStore.set(imagePath, {
      imagePath,
      messages: JSON.parse(JSON.stringify(messages)), // 深拷贝
      lastUpdated: Date.now(),
    });
  }, []);

  /**
   * 加载指定图片的对话历史
   */
  const loadChatHistory = useCallback((imagePath: string): ChatMessage[] => {
    if (!imagePath) return [];

    const entry = chatHistoryStore.get(imagePath);
    if (entry) {
      return JSON.parse(JSON.stringify(entry.messages)); // 深拷贝
    }
    return [];
  }, []);

  /**
   * 清空指定图片的对话历史
   */
  const clearChatHistory = useCallback((imagePath: string) => {
    if (!imagePath) return;
    chatHistoryStore.delete(imagePath);
  }, []);

  /**
   * 清空所有对话历史（退出系统时调用）
   */
  const clearAllChatHistory = useCallback(() => {
    chatHistoryStore.clear();
  }, []);

  /**
   * 获取所有图片的对话历史（用于调试）
   */
  const getAllChatHistory = useCallback((): ChatHistoryEntry[] => {
    return Array.from(chatHistoryStore.values());
  }, []);

  /**
   * 保存配置
   */
  const saveConfig = useCallback((config: ChatPersistenceConfig) => {
    configStore.current = { ...configStore.current, ...config };
  }, []);

  /**
   * 加载配置
   */
  const loadConfig = useCallback((): ChatPersistenceConfig => {
    return { ...configStore.current };
  }, []);

  /**
   * 检测图片切换
   */
  useEffect(() => {
    const previousPath = previousImagePathRef.current;
    const currentPath = currentImagePath || null;

    // 更新引用
    previousImagePathRef.current = currentPath;

    // 如果图片路径发生变化，触发切换事件
    if (previousPath !== currentPath) {
      // 这里可以触发回调，通知外部图片已切换
      // 外部可以根据这个事件来加载新图片的对话历史
    }
  }, [currentImagePath]);

  return {
    saveChatHistory,
    loadChatHistory,
    clearChatHistory,
    clearAllChatHistory,
    getAllChatHistory,
    saveConfig,
    loadConfig,
    currentImagePath: currentImagePath || null,
    hasHistory: currentImagePath ? chatHistoryStore.has(currentImagePath) : false,
  };
}

/**
 * 清空所有持久化数据（退出系统时调用）
 */
export function clearAllPersistenceData() {
  chatHistoryStore.clear();
  configStore.current = {};
}
