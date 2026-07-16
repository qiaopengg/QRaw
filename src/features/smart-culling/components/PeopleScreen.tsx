import { Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Info, ScanFace, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useProcessStore } from '../../../store/useProcessStore';
import type { ImageFile } from '../../../components/ui/AppProperties';
import { useSmartCullingText } from '../i18n';
import type { SmartCullingSnapshot } from '../types';
import { runSmartCullingCommand, useSmartCullingStore } from '../useSmartCulling';
import { LifecycleChrome, fileName } from './LifecycleChrome';

const PHOTO_PAGE_SIZE = 12;

export function PeopleScreen({ snapshot, images }: { snapshot: SmartCullingSnapshot; images: ImageFile[] }) {
  const tx = useSmartCullingText();
  const thumbnails = useProcessStore((state) => state.thumbnails);
  const { keyPeople, mode, busy, setState } = useSmartCullingStore();
  const [samplePath, setSamplePath] = useState(images[0]?.path ?? '');
  const [detectedSamplePath, setDetectedSamplePath] = useState('');
  const [photoPage, setPhotoPage] = useState(0);
  const faces = detectedSamplePath === samplePath ? snapshot.detectedFaces : [];
  const pageCount = Math.max(1, Math.ceil(images.length / PHOTO_PAGE_SIZE));
  const pageImages = images.slice(photoPage * PHOTO_PAGE_SIZE, (photoPage + 1) * PHOTO_PAGE_SIZE);
  const sampleUrl = thumbnails[samplePath];
  const selectedKeys = useMemo(
    () => new Set(keyPeople.map((person) => `${person.samplePath}:${person.bbox.join(',')}`)),
    [keyPeople],
  );
  const detect = async () => {
    if (!samplePath) return;
    const path = samplePath;
    try {
      await runSmartCullingCommand({ action: 'detectPeople', path }, true);
      setDetectedSamplePath(path);
    } catch {
      setDetectedSamplePath('');
    }
  };
  const selectFace = (bbox: [number, number, number, number]) => {
    const key = `${samplePath}:${bbox.join(',')}`;
    if (selectedKeys.has(key)) return;
    setState({ keyPeople: [...keyPeople, { samplePath, bbox, priority: keyPeople.length + 1 }] });
  };
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
            {sampleUrl ? (
              <img src={sampleUrl} alt={fileName(samplePath)} />
            ) : (
              <div className="sc-empty-photo">{fileName(samplePath)}</div>
            )}
            {faces.map((face, index) => (
              <button
                key={`${face.bbox.join('-')}-${index}`}
                className="sc-face-target"
                style={{
                  left: `${face.bbox[0] * 100}%`,
                  top: `${face.bbox[1] * 100}%`,
                  width: `${face.bbox[2] * 100}%`,
                  height: `${face.bbox[3] * 100}%`,
                }}
                onClick={() => selectFace(face.bbox)}
                aria-label={`${tx('selectPerson')} ${index + 1}`}
              >
                <span>{index + 1}</span>
              </button>
            ))}
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
                  <button aria-label={tx('moveEarlier')} onClick={() => move(index, -1)}>
                    <ChevronUp size={14} />
                  </button>
                  <button aria-label={tx('moveLater')} onClick={() => move(index, 1)}>
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
