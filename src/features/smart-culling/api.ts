import { invoke } from '@tauri-apps/api/core';
import { SMART_CULLING_COMMAND } from './constants';
import type { SmartCullingCommandError, SmartCullingRequest, SmartCullingSnapshot } from './types';

export function invokeSmartCulling(request: SmartCullingRequest): Promise<SmartCullingSnapshot> {
  return invoke<SmartCullingSnapshot>(SMART_CULLING_COMMAND, { request });
}

export function normalizeSmartCullingError(error: unknown): SmartCullingCommandError {
  if (error && typeof error === 'object' && 'code' in error && 'detail' in error) {
    const candidate = error as { code: unknown; detail: unknown };
    if (typeof candidate.code === 'string' && typeof candidate.detail === 'string') {
      return { code: candidate.code, detail: candidate.detail };
    }
  }
  return { code: 'unexpected_error', detail: String(error) };
}
