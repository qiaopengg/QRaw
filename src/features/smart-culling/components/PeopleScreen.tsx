import { Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Info, ScanFace, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome, fileName } from './LifecycleChrome';
import { faceSelectionKey, PhotoEvidenceViewport } from './PhotoEvidenceViewport';

const PHOTO_PAGE_SIZE = 12;

export function PeopleScreen({ snapshot, images }: { snapshot: SmartCullingSnapshot; images: ImageFile[] }) {
  const tx = useSmartCullingText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const { keyPeople, mode, busy, setState } = useSmartCullingStore();
  const [samplePath, setSamplePath] = useState(images[0]?.path ?? '');
  const [photoPage, setPhotoPage] = useState(0);
  const faces = snapshot.detectedImagePath === samplePath ? snapshot.detectedFaces : [];
  const pageCount = Math.max(1, Math.ceil(images.length / PHOTO_PAGE_SIZE));
  const pageImages = images.slice(photoPage * PHOTO_PAGE_SIZE, (photoPage + 1) * PHOTO_PAGE_SIZE);
  const sampleUrl = thumbnails[samplePath];
  const selectedKeys = useMemo(
    () => new Set(keyPeople.map((person) => faceSelectionKey(person.samplePath, person.bbox))),
    [keyPeople],
  );
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
  const currentImageIndex = images.findIndex((image) => image.path === samplePath);
  const move = (index: number, direction: -1 | 1) => {
    const next = [...keyPeople];
    const target = index + direction;
    if (target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target], next[index]];
    setState({ keyPeople: next.map((person, priority) => ({ ...person, priority: priority + 1 })) });
  };
  const start = () =>
    snapshot.rootPath &&
    runSmartCullingCommand({ action: 'start', rootPath: snapshot.rootPath, mode, keyPeople }).catch(() => undefined);
  return (
    <div className="sc-page">
      <LifecycleChrome screen="people">
        <button className="sc-text-button" onClick={() => setState({ screen: 'setup' })}>
          {tx('back')}
        </button>
      </LifecycleChrome>
      <main className="sc-people-shell">
        <section className="sc-people-browser">
          <header className="sc-heading">
            <span>{tx('optional')}</span>
            <h1>{tx('selectPeopleTitle')}</h1>
            <p>{tx('selectPeopleDescription')}</p>
            <em>{tx('currentTaskOnly')}</em>
          </header>
          <div className="sc-person-stage">
            <PhotoEvidenceViewport
              path={samplePath}
              fallbackUrl={sampleUrl}
              alt={fileName(samplePath)}
              faces={faces}
              selectedFaceKeys={selectedKeys}
              onFaceClick={(face) => selectFace(face.bbox)}
              onPrevious={() => selectImageAt(currentImageIndex - 1)}
              onNext={() => selectImageAt(currentImageIndex + 1)}
            />
          </div>
          <button className="sc-secondary sc-detect" disabled={!samplePath || busy} onClick={() => void detect()}>
            <ScanFace size={16} />
            {tx('detectPeople')}
          </button>
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
        <aside className="sc-priority">
          <h2>
            {tx('selectedPeople')} · {keyPeople.length}
          </h2>
          <p>{tx('keyPeopleHint')}</p>
          <div>
            {keyPeople.map((person, index) => (
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
                  <button
                    aria-label={tx('removePerson')}
                    onClick={() =>
                      setState({
                        keyPeople: keyPeople
                          .filter((_, current) => current !== index)
                          .map((item, priority) => ({ ...item, priority: priority + 1 })),
                      })
                    }
                  >
                    <Trash2 size={14} />
                  </button>
                </span>
              </article>
            ))}
          </div>
          <section className="sc-info">
            <Info size={15} />
            <p>{tx('offline')}</p>
          </section>
          <footer>
            <button className="sc-secondary" onClick={() => setState({ screen: 'setup', keyPeople: [] })}>
              {tx('skipPeople')}
            </button>
            <button className="sc-primary" disabled={busy} onClick={() => void start()}>
              <Check size={15} />
              {tx('saveAndStart')}
            </button>
          </footer>
        </aside>
      </main>
    </div>
  );
}
