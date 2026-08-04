import { ChevronLeft, ChevronRight, ScanFace, X } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { useProcessStore } from '../../../store/useProcessStore';
import { keyPersonIdentityLabel, useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { fileName } from './LifecycleChrome';
import { faceSelectionKey, PhotoEvidenceViewport } from './PhotoEvidenceViewport';
import { SmartCullingImage } from './SmartCullingImage';

const PHOTO_PAGE_SIZE = 8;

export function KeyPeoplePicker({
  snapshot,
  images,
  onClose,
  onRequestThumbnails,
}: {
  snapshot: SmartCullingSnapshot;
  images: ImageFile[];
  onClose: () => void;
  onRequestThumbnails?: (paths: string[]) => void;
}) {
  const tx = useSmartCullingText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const { keyPeople, busy, setState } = useSmartCullingStore();
  const [samplePath, setSamplePath] = useState(images[0]?.path ?? '');
  const [photoPage, setPhotoPage] = useState(0);
  useEffect(() => {
    if (!samplePath && images[0]) {
      setSamplePath(images[0].path);
      setPhotoPage(0);
    }
  }, [images, samplePath]);
  const faces = snapshot.detectedImagePath === samplePath ? snapshot.detectedFaces : [];
  const pageCount = Math.max(1, Math.ceil(images.length / PHOTO_PAGE_SIZE));
  const pageImages = useMemo(
    () => images.slice(photoPage * PHOTO_PAGE_SIZE, (photoPage + 1) * PHOTO_PAGE_SIZE),
    [images, photoPage],
  );
  useEffect(() => {
    const paths = pageImages.map((image) => image.path);
    if (paths.length > 0) onRequestThumbnails?.(paths);
  }, [onRequestThumbnails, pageImages]);
  const selectedKeys = useMemo(
    () => new Set(keyPeople.map((person) => faceSelectionKey(person.samplePath, person.bbox))),
    [keyPeople],
  );
  const selectedLabels = useMemo(
    () =>
      new Map(
        keyPeople.map((person) => [
          faceSelectionKey(person.samplePath, person.bbox),
          keyPersonIdentityLabel(person.priority),
        ]),
      ),
    [keyPeople],
  );
  const currentImageIndex = images.findIndex((image) => image.path === samplePath);
  const detect = async () => {
    if (!samplePath) return;
    await runSmartCullingCommand({ action: 'detectPeople', path: samplePath }, true).catch(() => undefined);
  };
  const toggleFace = (bbox: [number, number, number, number]) => {
    const key = faceSelectionKey(samplePath, bbox);
    if (!selectedKeys.has(key)) {
      setState({ keyPeople: [...keyPeople, { samplePath, bbox, priority: keyPeople.length + 1 }] });
      return;
    }
    setState({
      keyPeople: keyPeople
        .filter((person) => faceSelectionKey(person.samplePath, person.bbox) !== key)
        .map((person, priority) => ({ ...person, priority: priority + 1 })),
    });
  };
  const selectImageAt = (index: number) => {
    if (index < 0 || index >= images.length) return;
    setSamplePath(images[index].path);
    setPhotoPage(Math.floor(index / PHOTO_PAGE_SIZE));
  };
  return (
    <section className="sc-key-people-picker" aria-labelledby="sc-key-people-title">
      <header>
        <div>
          <h2 id="sc-key-people-title">{tx('selectPeopleTitle')}</h2>
        </div>
        <button className="sc-key-people-close" onClick={onClose} aria-label={tx('close')}>
          <X size={16} />
        </button>
      </header>
      <div className="sc-key-people-grid">
        <section className="sc-key-people-browser">
          <div className="sc-key-people-stage">
            <PhotoEvidenceViewport
              path={samplePath}
              fallbackUrl={thumbnails[samplePath]}
              alt={fileName(samplePath)}
              faces={faces}
              selectedFaceKeys={selectedKeys}
              faceLabels={selectedLabels}
              onFaceClick={(face) => toggleFace(face.bbox)}
              onPrevious={() => selectImageAt(currentImageIndex - 1)}
              onNext={() => selectImageAt(currentImageIndex + 1)}
            />
          </div>
          <div className="sc-key-people-controls">
            <button className="sc-secondary" disabled={!samplePath || busy} onClick={() => void detect()}>
              <ScanFace size={15} />
              {tx('detectPeople')}
            </button>
            <p>{tx('keyPeoplePickerHint')}</p>
          </div>
          <div className="sc-filmstrip-row">
            <button
              className="sc-filmstrip-nav"
              disabled={photoPage === 0}
              onClick={() => setPhotoPage((current) => Math.max(0, current - 1))}
              aria-label={tx('previousPhotos')}
            >
              <ChevronLeft size={15} />
            </button>
            <div className="sc-filmstrip">
              {pageImages.map((image) => (
                <button
                  className={samplePath === image.path ? 'is-active' : ''}
                  key={image.path}
                  onClick={() => setSamplePath(image.path)}
                >
                  {thumbnails[image.path] ? (
                    <SmartCullingImage
                      primaryUrl={thumbnails[image.path]}
                      alt={fileName(image.path)}
                      fallback={<span>{fileName(image.path)}</span>}
                    />
                  ) : (
                    <span>{fileName(image.path)}</span>
                  )}
                </button>
              ))}
            </div>
            <button
              className="sc-filmstrip-nav"
              disabled={photoPage >= pageCount - 1}
              onClick={() => setPhotoPage((current) => Math.min(pageCount - 1, current + 1))}
              aria-label={tx('nextPhotos')}
            >
              <ChevronRight size={15} />
            </button>
          </div>
        </section>
      </div>
    </section>
  );
}
