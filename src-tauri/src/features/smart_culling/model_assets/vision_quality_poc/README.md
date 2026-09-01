# Apple Vision quality observation POC

Status: macOS Debug calibration only. Face-capture quality is admitted only as
a confidence-capped person-clarity soft score. It is not a first-level clarity
gate. Holistic aesthetics remains observation-only and is not an optical,
composition, or final-rating input.

## System sources and runtime contract

This POC uses only public Apple Vision APIs already supplied by macOS:

- [`VNCalculateImageAestheticsScoresRequest`](https://developer.apple.com/documentation/vision/vncalculateimageaestheticsscoresrequest), available on macOS 15 or newer, returns Apple's holistic `overallScore` in `[-1, 1]` and `isUtility` flag;
- [`VNDetectFaceCaptureQualityRequest`](https://developer.apple.com/documentation/vision/vndetectfacecapturequalityrequest), revision 3 on macOS 14 or newer, returns a relative face-capture quality value in `[0, 1]`;
- [`VNDetectHumanRectanglesRequest`](https://developer.apple.com/documentation/vision/vndetecthumanrectanglesrequest) returns human rectangle observations and confidence values;
- [WWDC24 “Discover Swift enhancements in the Vision framework”](https://developer.apple.com/videos/play/wwdc2024/10163/?time=790) describes the aesthetics request as considering blur, exposure, aesthetic quality and memorability.

`vision_quality_poc.rs` and its `macos.rs` bridge resolve all three classes
dynamically, so the macOS 13 deployment target remains loadable. The combined observation is unavailable
when the macOS 15 aesthetics class is absent. The input is resized without
changing aspect ratio to at most 1600 px and PNG-encoded in memory. Existing
YuNet pixel boxes are converted to Vision coordinates and supplied as
`VNFaceObservation` inputs to the face-capture-quality request.

No model weight, runtime download, new crate, or application resource is added.
The adapter reuses the project's existing macOS `objc` dependency and links the
system Vision framework. Release and non-macOS builds do not compile this POC.

## Isolated evidence (2026-08-28)

None of these sets is an independent business blind test.

### Existing 358-image calibration pool

- Apple aesthetics score coverage: `358/358`; Spearman correlation with the
  existing manual final star was `0.243` overall. This is too weak and too
  semantically mixed to enter optical or composition scoring.
- Face-capture quality correlation with the existing manual final star was
  `0.631` overall. The signal is useful for research, but it includes lighting,
  sharpness and framing and therefore cannot be treated as a pure first-level
  person-clarity gate without component labels.
- Human detection returned a result for `349/358`; every positive maximum
  confidence was at least `0.6049519`.

### Controlled quality distortions

Sixteen high-quality anchors produced 112 derived variants. The percentage in
which the distorted image scored below its unchanged anchor was:

| Distortion | Aesthetics | Face capture quality |
| --- | ---: | ---: |
| Mild blur | 100% | 93.75% |
| Severe blur | 100% | 100% |
| 8x downsample/upscale | 93.75% | 100% |
| Low-quality JPEG | 100% | 100% |
| Added noise | 100% | 100% |
| Overexposure | 87.5% | 100% |
| Underexposure | 100% | 100% |

This proves useful distortion sensitivity only. Derived images from known
anchors are not a replacement for independent camera-task validation.

### Human-presence safety guard

- 138 macOS system landscape/abstract wallpaper thumbnails produced `0` human
  detections.
- In the existing 200-image calibration set, the Vision request found three
  genuine people in images where YuNet produced zero faces.
- The Debug-only guard therefore uses only one conservative action: when the
  existing automatic strategy has resolved to the scene path but Vision reports
  a human with confidence `>= 0.50`, the photo is sent to manual review with
  `auto_people_uncertain`. It never routes directly to people scoring and never
  assigns or changes a star.

### Rust integration replay

The complete 200-image paired replay returned:

- aesthetics scores: `200/200`;
- face-capture quality: `202/202` supplied YuNet boxes, no missing value;
- human detections at the review threshold: `193/200`;
- face-count differences from the pre-integration report: `0`;
- left/right eye state and reason differences: `0`;
- portrait-mode rating/review/reason/group differences: `0`.

Repeated whole replays finished in approximately 30.3–31.0 seconds on the
current Mac, but that number includes all existing face, eye and expression inference. It is not a frozen
per-image Vision performance budget.

## Rejected or blocked alternatives in the same audit

- RT-DETRv2 official commit
  `068dfde65f2667ad6555883c69d73de886518cad` is Apache-2.0. The official
  R18 COCO weight was downloaded only to a temporary research directory:
  `81,198,974` bytes, SHA-256
  `2ace52184b620204004509b72752ac7bfe64aadaf7fc1d076b18df8ab5a5c77e`.
  The official PyTorch ONNX exporter crashed three times on this Mac with a
  `libc++` mutex error, including a single-thread retry. No ONNX artifact was
  produced or copied into QRaw, so this candidate failed the current macOS
  deployment gate.
- SAMP-Net official commit
  `b8f80be379d4a4caf9d045db3187cb60bcaca583` has MIT-licensed repository code,
  but its approximately 180 MB checkpoint is hosted only through the README's
  Dropbox/Baidu links. The official Dropbox connection did not complete and no
  stable release asset/hash was available; the data/weight redistribution
  conclusion also remains unresolved. No mirror was substituted.
- IQA-PyTorch official commit
  `18dd7a19694e94aac21019170e3f5e63d6b4e19e` is under the PolyForm
  Noncommercial License plus an additional S-Lab license. It is rejected for a
  commercial product path rather than copied into the project.

## Remaining gates

- independent, never-before-seen camera tasks with component labels for person
  clarity, optical defects, composition and final stars;
- macOS/Windows parity or an explicitly approved platform-specific product
  policy;
- frozen latency, memory and thermal budgets;
- proof that a candidate is component-specific before assigning the requested
  optical/composition weights;
- photographer acceptance.

Until those gates pass, aesthetics remains a raw report field only.
Face-capture quality may affect only the Debug calibration path's weighted
person-clarity evidence, with confidence capped below `0.50` and legacy
sharpness fallback when unavailable. Neither signal may be split into
optical/composition scores, used as a one-star clarity gate, promoted to
Release, or allowed to change the frozen eye contract.
