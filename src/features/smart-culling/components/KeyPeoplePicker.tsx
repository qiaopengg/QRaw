import { ChevronLeft, ChevronRight, ScanFace, UserRoundPlus, X } from 'lucide-react';
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
/** Mirrors MAX_REFERENCES_PER_KEY_PERSON in the Rust coordinator support module. */
const MAX_REFERENCES_PER_PERSON = 5;

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
  /** Identity that the next picked face is added to as another reference photo. */
  const [targetPriority, setTargetPriority] = useState<number | null>(null);
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
  const identities = useMemo(() => {
    const counts = new Map<number, number>();
    keyPeople.forEach((person) => counts.set(person.priority, (counts.get(person.priority) ?? 0) + 1));
    return [...counts.entries()].sort((left, right) => left[0] - right[0]);
  }, [keyPeople]);
  const currentImageIndex = images.findIndex((image) => image.path === samplePath);
  const detect = async () => {
    if (!samplePath) return;
    await runSmartCullingCommand({ action: 'detectPeople', path: samplePath }, true).catch(() => undefined);
  };
  const toggleFace = (bbox: [number, number, number, number]) => {
    const key = faceSelectionKey(samplePath, bbox);
    if (selectedKeys.has(key)) {
      const remaining = keyPeople.filter((person) => faceSelectionKey(person.samplePath, person.bbox) !== key);
      // Identities must stay contiguous from 1 for the backend, so renumber and
      // remap the active target through the same mapping. Without the remap the
      // target would silently point at a different person once an identity is
      // fully removed.
      const order = [...new Set(remaining.map((person) => person.priority))].sort((left, right) => left - right);
      setState({
        keyPeople: remaining.map((person) => ({ ...person, priority: order.indexOf(person.priority) + 1 })),
      });
      setTargetPriority((current) => {
        if (current === null) return null;
        const remapped = order.indexOf(current);
        return remapped >= 0 ? remapped + 1 : null;
      });
      return;
    }
    // Attach to the identity the user is currently building, so one person can
    // gather several reference photos; otherwise start a new identity.
    const nextPriority = targetPriority ?? identities.length + 1;
    const referenceCount = keyPeople.filter((person) => person.priority === nextPriority).length;
    if (referenceCount >= MAX_REFERENCES_PER_PERSON) return;
    setState({ keyPeople: [...keyPeople, { samplePath, bbox, priority: nextPriority }] });
    setTargetPriority(nextPriority);
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
          <div className="sc-identity-targets" role="group" aria-label={tx('referenceTargetLabel')}>
            <span className="sc-identity-targets-label">{tx('addReferenceTo')}</span>
            {identities.map(([priority, count]) => (
              <button
                key={priority}
                className={targetPriority === priority ? 'is-active' : ''}
                aria-pressed={targetPriority === priority}
                disabled={count >= MAX_REFERENCES_PER_PERSON}
                title={
                  count >= MAX_REFERENCES_PER_PERSON
                    ? `${tx('referenceLimitReached')} (${MAX_REFERENCES_PER_PERSON})`
                    : undefined
                }
                onClick={() => setTargetPriority(priority)}
              >
                {tx('person')} {keyPersonIdentityLabel(priority)}
                <em>
                  {count}/{MAX_REFERENCES_PER_PERSON}
                </em>
              </button>
            ))}
            <button
              className={`sc-identity-new ${targetPriority === null ? 'is-active' : ''}`}
              aria-pressed={targetPriority === null}
              onClick={() => setTargetPriority(null)}
            >
              <UserRoundPlus size={13} />
              {tx('newPerson')}
            </button>
          </div>
          <p className="sc-identity-hint">{tx('multiReferenceHint')}</p>
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
