import { useEffect } from 'react';
import { toast } from 'react-toastify';
import { useEditorStore } from '../../store/useEditorStore';
import { useUIStore } from '../../store/useUIStore';
import { SMART_CULLING_RUNNING_STATES } from './constants';
import { useSmartCullingText } from './i18n';
import { useSmartCullingStore } from './useSmartCulling';

/** Keeps the GPU task's editor lock inside the independently maintained feature. */
export default function SmartCullingEditorGuard() {
  const tx = useSmartCullingText();
  const state = useSmartCullingStore((store) => store.snapshot?.state);

  useEffect(() => {
    if (!state || !SMART_CULLING_RUNNING_STATES.some((runningState) => runningState === state)) return;
    useEditorStore.getState().setEditor({ selectedImage: null });
    useUIStore.getState().setUI({ activeView: 'library' });
    toast.info(tx('editorBlocked'));
  }, [state, tx]);

  return null;
}
