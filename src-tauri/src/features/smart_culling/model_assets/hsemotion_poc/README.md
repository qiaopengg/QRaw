# HSEmotion expression-quality fusion POC

Status: isolated macOS Debug calibration only. The two derived ONNX files live
under the QRaw extension tree, not RapidRAW upstream code or Tauri production
resources. Release and non-macOS builds keep the safe unavailable path.

## Upstream source and license audit

- Official project: https://github.com/av-savchenko/hsemotion-onnx
- Current upstream project: https://github.com/sb-ai-lab/EmotiEffLib
- MTL source model:
  https://raw.githubusercontent.com/sb-ai-lab/EmotiEffLib/main/models/affectnet_emotions/onnx/enet_b0_8_va_mtl.onnx
- VGAF source model:
  https://raw.githubusercontent.com/sb-ai-lab/EmotiEffLib/main/models/affectnet_emotions/onnx/enet_b0_8_best_vgaf.onnx
- Training/model details:
  https://github.com/sb-ai-lab/EmotiEffLib#details

The official repositories carry Apache License 2.0, and the HSEmotionONNX
README states that the library code has no academic/commercial-use limitation.
The model files are distributed from the same upstream repository, but no
separate model card was found that independently restates the weights'
redistribution terms. Release packaging therefore remains blocked on an
explicit weight/training-data license review. This engineering POC is not a
legal clearance. A copy of the upstream repository license is retained as
`LICENSE-APACHE-2.0.txt`; the graph rewrite and renamed files are QRaw
modifications documented below.

## Runtime contract

- Models: `enet_b0_8_va_mtl` (10 raw outputs) and `enet_b0_8_best_vgaf`
  (8 raw outputs).
- Input: one detected face, expanded to a 1.2x square, clipped to the image,
  resized to 224x224 with Triangle filtering.
- Channels: OpenCV-compatible BGR order, divided by 255, then normalized with
  means `[0.485, 0.456, 0.406]` and standard deviations
  `[0.229, 0.224, 0.225]`, matching the official wrapper.
- Output: raw logits/auxiliary values only. Categorical emotion names are not
  used as photo-quality labels.
- Provider: QRaw's existing ONNX Runtime Core ML session with CPU EP fallback
  disabled. No new Rust runtime dependency was added.

The source classifiers end with `GlobalAveragePool -> Flatten -> Gemm`, which
the bundled ONNX Runtime assigns partly to the CPU provider. The derived graphs
replace only `Flatten -> Gemm` with the mathematically equivalent 1x1 `Conv`
after global pooling. `export_and_verify.py` downloads the pinned source files,
checks their hashes, regenerates both graphs, compares deterministic CPU
outputs, and verifies the checked-in derived bytes.

## Frozen hashes and numerical verification

- MTL source SHA-256:
  `c43e056ad388d4a8dc911832b8291435b2af537f967e5870ebd731574ec7e812`
- MTL derived SHA-256:
  `b11cd798683082eee26c1cc0871aeb5ee545bf7d4330db0b5de3091b00d0eed7`
- MTL source/derived maximum absolute CPU difference: `0.00000166893005`
- VGAF source SHA-256:
  `fa07e841fd06c7a67ee651ea4e6e4a3a2bb5695f47b37a7da50492526f59c898`
- VGAF derived SHA-256:
  `52383e3d3757286c0ced73ee0aeb50839111b775c8235cac1e43bb6ff16c773e`
- VGAF source/derived maximum absolute CPU difference: `0.00000250339508`

Both derived models passed the explicit macOS hardware test with Core ML and
`session.disable_cpu_ep_fallback=1`.

## Fusion calibration and limits

`train_fusion.py` consumes paired-replay JSONL reports and uses:

- 18 raw HSE outputs;
- `log1p(100 * value)` for the existing 38 non-eye Blendshape coefficients;
- 105 geometrically reliable expression-labelled observations;
- 28 explicit generated calibration observations and the 10-image `e001`
  incremental batch, weighted 3x;
- 125 reliable old manual 3-5 star observations as one-way positive guards
  only. Old low-star photos are never treated as expression-negative labels.

The frozen constants and input report hashes are recorded in
`FUSION_CALIBRATION.json`. Leave-one-capture-group-out results were:

- original reliable expression set: 85/105 (81.0%), AUC 0.876;
- original 28-image calibration set: 23/28 (82.1%), AUC 0.963;
- newest eight-image subset: 5/8 (62.5%), AUC 0.800;
- `e001` before inclusion: 8/10 (80.0%), AUC 0.880; after explicit
  calibration inclusion: 10/10 (this is in-sample fit, not validation);
- prospective generated `e002` holdout with the unchanged 0.5 head: 8/10
  (80.0%), AUC 1.000, continuous-score MAE 0.239; both false passes were
  routed to final human review rather than persisted automatically;
- prospective generated `e003` holdout with the same unchanged head: 8/10
  (80.0%), AUC 0.960, continuous-score MAE 0.254; its two false passes were
  also routed to final human review;
- combined prospective `e002` + `e003`: 16/20 (80.0%), AUC 0.990,
  continuous-score MAE 0.247;
- `e004` post-reveal deterministic diagnostic: 8/10 (80.0%), AUC 1.000,
  continuous-score MAE 0.181. This batch is excluded from strict prospective
  totals because the old protocol had not frozen component scores before
  labels were revealed and one final runtime/replay review decision differed;
- positive guard: 125/125, minimum score 0.50000025.

Except for the strict prospective `e002` and `e003` rows, these sets have all
participated in model selection or calibration and are not blind tests. In
particular, `e001` exposed two false passes and was then used to select the smallest
calibration-weight change that corrected them; its 10/10 fit result is not a
generalization claim. Runtime confidence remains capped below 0.5. The fusion
never reads filenames, paths, hashes, ratings, or `eye*` Blendshape
coefficients.

`e002` and `e003` were each frozen before their manual expression labels were
revealed and neither was used to train the current 0.5 constants. They provide
prospective component evidence for this version, but each contains only ten
generated images and two threshold false passes. Training on `e002` improved
the then-unseen `e003` result to 9/10 but reduced the original 105-image grouped
holdout from 81.0% to 78.1%; fitting both known batches reduced it to 77.1%.
Neither predefined candidate improved the then-unseen `e004` beyond the
unchanged head's 8/10, so those candidate updates remain rejected. After
reveal all three batches are known data; any future calibration requires a
fresh `e005` or real-camera holdout. The blind tooling now freezes source-based
component predictions and their SHA-256 before manual labels are visible; this
was verified after reveal as an engineering workflow test and cannot
retroactively restore `e004`'s strict status.
This is not a 95% accuracy, full portrait-mode, or Release-readiness claim.

The HSE models do see the full face pixels, so their latent outputs may encode
eye appearance; that limitation must be considered when interpreting the
expression score. The separately frozen eye classifier is evaluated first and
is never changed by the fusion. Across the earlier 200-image regression set,
the 0.4-to-0.5 update kept eye candidates, left/right eye results, EAR values,
and all `eye*` coefficients byte-for-byte identical. The eye contract remains
`qraw-eye-model-contract-1.0` + `qraw-eye-policy-1.1`.

Before any Release enablement, QRaw still requires an independent real-camera
blind task, Windows DirectML parity, performance/thermal measurement,
photographer acceptance, and explicit model-weight redistribution clearance.
