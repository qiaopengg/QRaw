import { isImmichAlbumId } from '../../components/immich/immichApi';
import type { ImageFile } from '../../components/ui/AppProperties';

export function isSmartCullingSource(image: ImageFile, activeAlbumId: string | null) {
  return !isImmichAlbumId(activeAlbumId) && !image.is_virtual_copy && !image.is_cloud_placeholder;
}

export function canInspectSmartCullingFolder(
  currentFolderPath: string | null,
  activeAlbumId: string | null,
): currentFolderPath is string {
  return Boolean(currentFolderPath) && !activeAlbumId;
}
