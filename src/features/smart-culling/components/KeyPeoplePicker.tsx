import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Info,
  ScanFace,
  Trash2,
  UserRoundCheck,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { useProcessStore } from '../../../store/useProcessStore';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { fileName } from './LifecycleChrome';
import { faceSelectionKey, PhotoEvidenceViewport } from './PhotoEvidenceViewport';

const PHOTO_PAGE_SIZE = 8;

export function KeyPeoplePicker({
  snapshot,
  images,
  onClose,
}: {
  snapshot: SmartCullingSnapshot;
  images: ImageFile[];
  onClose: () => void;
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
  const pageImages = images.slice(photoPage * PHOTO_PAGE_SIZE, (photoPage + 1) * PHOTO_PAGE_SIZE);
  const selectedKeys = useMemo(
    () => new Set(keyPeople.map((person) => faceSelectionKey(person.samplePath, person.bbox))),
    [keyPeople],
  );
  const currentImageIndex = images.findIndex((image) => image.path === samplePath);
  const detect = async () => {
    if (!samplePath) return;
    await runSmartCullingCommand({ action: 'detectPeople', path: samplePath }, true).catch(() => undefined);
  };
  const selectFace = (bbox: [number, number, number, number]) => {
    const key = faceSelectionKey(samplePath, bbox);
    if (selectedKeys.has(key)) return;
    setState({ keyPeople: [...keyPeople, { samplePath, bbox, priority: keyPeople.length + 1 }] });
  };
  const selectImageAt = (index: number) => {
    if (index < 0 || index >= images.length) return;
    setSamplePath(images[index].path);
    setPhotoPage(Math.floor(index / PHOTO_PAGE_SIZE));
  };
  const move = (index: number, direction: -1 | 1) => {
    const next = [...keyPeople];
    const target = index + direction;
    if (target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target], next[index]];
    setState({ keyPeople: next.map((person, priority) => ({ ...person, priority: priority + 1 })) });
  };
  const remove = (index: number) =>
    setState({
      keyPeople: keyPeople
        .filter((_, current) => current !== index)
        .map((person, priority) => ({ ...person, priority: priority + 1 })),
    });

  return (
    <section className="sc-key-people-picker" aria-labelledby="sc-key-people-title">
      <header>
        <div>
          <span>
            <UserRoundCheck size={15} />
            {tx('optional')}
          </span>
          <h2 id="sc-key-people-title">{tx('selectPeopleTitle')}</h2>
          <p>{tx('selectPeopleDescription')}</p>
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
              onFaceClick={(face) => selectFace(face.bbox)}
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
                    <img src={thumbnails[image.path]} alt={fileName(image.path)} />
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
        <aside className="sc-key-people-priority">
          <div>
            <h3>
              {tx('selectedPeople')} · {keyPeople.length}
            </h3>
            <p>{tx('keyPeopleHint')}</p>
          </div>
          <div className="sc-key-people-list">
            {keyPeople.length ? (
              keyPeople.map((person, index) => (
                <article key={`${person.samplePath}-${person.bbox.join('-')}`}>
                  <b>{index + 1}</b>
                  <div>
                    <strong>{fileName(person.samplePath)}</strong>
                    <small>{index === 0 ? tx('highestPriority') : `${tx('priority')} ${index + 1}`}</small>
                  </div>
                  <span>
                    <button disabled={index === 0} aria-label={tx('moveEarlier')} onClick={() => move(index, -1)}>
                      <ChevronUp size={14} />
                    </button>
                    <button
                      disabled={index === keyPeople.length - 1}
                      aria-label={tx('moveLater')}
                      onClick={() => move(index, 1)}
                    >
                      <ChevronDown size={14} />
                    </button>
                    <button aria-label={tx('removePerson')} onClick={() => remove(index)}>
                      <Trash2 size={14} />
                    </button>
                  </span>
                </article>
              ))
            ) : (
              <p className="sc-key-people-empty">{tx('noPeople')}</p>
            )}
          </div>
          <div className="sc-info">
            <Info size={15} />
            <p>{tx('currentTaskOnly')}</p>
          </div>
        </aside>
      </div>
    </section>
  );
}
