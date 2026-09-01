# Face-motion model POC assets

Status: isolated engineering POC/calibration only. These files are not under
`src-tauri/resources`. macOS Debug calibration builds load the audited
FaceMesh/Blendshape assets for frozen eye evidence and experimental expression
scoring; Release and other platforms keep the safe unavailable path.

The separate HSEmotion expression-quality model, graph rewrite, calibration
manifest, licensing caveat, and verification evidence are documented in
[`hsemotion_poc/README.md`](hsemotion_poc/README.md).

The system-provided Apple Vision aesthetics, face-capture-quality and human
observation audit is documented in
[`vision_quality_poc/README.md`](vision_quality_poc/README.md). It adds no model
asset and remains a macOS Debug calibration path.

## Local calibration loading

The application does not redistribute these calibration weights. Debug builds
resolve an audited model directory in this order:

1. absolute `QRAW_SMART_CULLING_CALIBRATION_MODEL_DIR` override;
2. `resources/smart_culling_calibration_models` when a separately approved
   local build provides it;
3. the application-data `smart_culling_calibration_models` directory;
4. this source directory for development runs.

Every resolved model still has to match the frozen SHA-256 value before a
session is created. A missing directory or mismatched model fails preflight;
there is no network download or silent CPU/model fallback.

## Sources and contracts

- MediaPipe Face Landmarker documentation:
  https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker
- MediaPipe FaceMeshV2 model card:
  https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Face%20Mesh%20V2.pdf
- MediaPipe BlendshapeV2 model card:
  https://storage.googleapis.com/mediapipe-assets/Model%20Card%20Blendshape%20V2.pdf
- MediaPipe blendshape graph and its exact 146-landmark/52-output mapping:
  https://github.com/google-ai-edge/mediapipe/blob/master/mediapipe/tasks/cc/vision/face_landmarker/face_blendshapes_graph.cc
- ONNX conversions audited as inputs to this POC:
  https://github.com/PINTO0309/PINTO_model_zoo/tree/main/410_FaceMeshV2
  and
  https://github.com/PINTO0309/PINTO_model_zoo/tree/main/390_BlendShapeV2

MediaPipe and the model cards identify the source models under Apache License
2.0. PINTO's conversion repository requires retaining the source model's
license and identifies its conversion scripts as MIT-licensed. No model is
downloaded at application runtime.

FaceMeshV2 contract:

- Input: float32 RGB `[1, 3, 256, 256]` after the TFLite metadata transform
  `(pixel - 0) / 255`.
- Outputs: 478 xyz landmarks (`1434` values), one raw face-presence logit, and
  one sigmoid `tongue_out` score.

BlendshapeV2 contract:

- Input: float32 `[1, 146, 2]`, selected from 478 full-image landmark pixel
  coordinates in the exact MediaPipe graph order.
- Output: 52 sigmoid blendshape coefficients in the exact MediaPipe graph
  order. They are raw motion evidence, not eye/expression decisions.

## Source and derived hashes

- Download archive `390_BlendShapeV2/resources.tar.gz`:
  `17970499a4e436b42d96ade1cc8e26c5192ec3bb0d57b4d5ae4e145ace750c33`
- Original static BlendshapeV2 ONNX:
  `82b330e63efe085bc8351db5391a1708df5b2b4aaa0a7c2653795500d1575b75`
- Derived `face_blendshapes_v2_qraw_poc.onnx`:
  `b90ed4146dfdb43745c5988b1d411ed026d4b5e2ba9c1d7c271954fd1f5cb60e`
- Download archive `410_FaceMeshV2/resources.tar.gz`:
  `bcbc0e7fc711b3d5504defcbfc1d47b39ba67591fb238018d65a3c4b1642e79d`
- Original static FaceMeshV2 ONNX:
  `70fe4e14169ca084b03b8103077a4051296e07939a19c1fdfd1f18b3792b4048`
- Derived `face_landmarks_detector_v2_qraw_poc.onnx`:
  `b047d95fab6702c327175e7b77eea71ffd2b2ef0110c7466eee9b6e2ae87b552`

## Audited graph rewrites

The source ONNX files are numerically correct but do not pass QRaw's strict
Core ML gate with CPU fallback disabled. The derived files retain weights and
replace only mathematically equivalent operators:

- FaceMeshV2: per-channel `PRelu(x, a)` becomes
  `Relu(x) - a * Relu(-x)`; channel-only zero `Pad` becomes an axis-1 `Concat`
  with a constant zero tensor.
- BlendshapeV2: `Neg(x)` becomes `0 - x`; the single axis-3 `Concat` is wrapped
  by inverse transposes so Core ML receives an axis-1 channel concat. ONNX
  graph optimization must remain disabled for this model because ORT otherwise
  folds the transposes back into the unsupported axis-3 form.
- Both graphs: Conv and MaxPool nodes that rely on ONNX's omitted zero-padding
  default now declare `pads=[0,0,0,0]` explicitly for the ONNX Runtime version
  bundled by QRaw. This changes no numerical result.

Verification on macOS:

- TFLite vs original ONNX, deterministic official-range inputs: FaceMeshV2
  maximum absolute errors were `0.000305176` (landmarks), `0.000056982`
  (presence logit), and `0.0000000077` (`tongue_out`); BlendshapeV2 maximum
  absolute error was `0.000001431`.
- Original ONNX vs derived ONNX on CPU: maximum absolute error `0` for every
  output across the deterministic audit inputs.
- Both derived models load and infer with strict Core ML execution and
  `session.disable_cpu_ep_fallback=1`, including the ONNX Runtime 1.22 library
  bundled by QRaw.
- The explicit Rust hardware test runs inference in an isolated child process.
  This avoids an unrelated ONNX Runtime 1.21/1.22 macOS logger-mutex destructor
  defect after successful inference:
  https://github.com/microsoft/onnxruntime/issues/24579 and
  https://github.com/microsoft/onnxruntime/issues/25038. Application code does
  not use the child-process exit workaround.

This proves conversion and execution contracts only. It does not prove eye or
expression accuracy. Windows DirectML and real-photo accuracy remain mandatory
gates before any production integration.
