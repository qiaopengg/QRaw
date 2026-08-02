# Smart Culling Review UI — Design QA

- Source visual truth: `/Users/qiaopeng/.codex/generated_images/019f6390-0ad4-7973-bf4a-c5822e85e4b6/call_f0GIwkWDk6JoIUiVpnaL1H98.png`
- Implementation screenshot: `/var/folders/mf/32brj6rn5qdgwddgl88f4brw0000gn/T/com.openai.sky.CUAService/RapidRAW Screenshot 2026-07-29 at 3.10.17 PM.jpeg`
- Full-view comparison: `/tmp/qraw-smart-culling-normalized-comparison.jpg`
- Focused workspace comparison: `/tmp/qraw-smart-culling-normalized-focus-comparison.jpg`
- State: macOS desktop client, dark theme, review stage, one three-photo similar group, A/B fit view, all filmstrip previews loaded
- Implementation viewport: 1404 × 768 pixels, desktop window capture, device density normalized to the captured pixels
- Source pixels: 1487 × 1058
- Implementation pixels: 1404 × 768
- Normalization: the source was resized to 1404 × 999 and top-cropped to 1404 × 768 so both full-view frames use the implementation viewport. The focused comparison uses corresponding review-workspace crops at the same displayed size.

## Full-view comparison evidence

The implementation preserves the selected design's hierarchy: lifecycle header, review queue, explicit A/B evidence panes, per-photo decisions, horizontal group filmstrip, and final apply action. It adapts the mock to QRaw's existing library shell rather than replacing upstream navigation. Portrait evidence is intentionally letterboxed with `contain`; the source mock uses landscape photos that fill its panes.

## Focused comparison evidence

The focused workspace comparison confirms two equal evidence viewports, visible A/B identity, fit/zoom controls, per-photo rating and label actions, and a group filmstrip. The implementation uses QRaw's existing compact typography and color tokens. A/B switching replaces squeezed side-by-side panes below the feature's 899 px container breakpoint.

## Required fidelity surfaces

- Fonts and typography: existing QRaw system font stack, weights, and compact hierarchy are reused. Labels remain legible at the desktop and half-screen breakpoints.
- Spacing and layout rhythm: evidence panes have equal tracks; header, decisions, and filmstrip follow the source order. The QRaw library shell adds an intentional outer navigation region.
- Colors and tokens: QRaw dark surfaces, borders, green selection, yellow stars, and red/yellow/green labels are reused without introducing a second visual system.
- Image quality: all evidence and filmstrip images use current-render previews. Images use `contain`, are never stretched or cropped, and mixed dimensions share normalized zoom/pan coordinates.
- Copy and content: Chinese and English copy distinguishes single-photo review from actual similar-photo comparison. A single photo never displays comparison, similarity, synchronized zoom, or group-count language.

## Comparison history

### Iteration 1

- [P2] A group photo without a library thumbnail displayed a filename-only filmstrip tile.
  - Fix: extracted the existing current-render preview loader into a shared cached hook and added in-flight request deduplication.
  - Post-fix evidence: the final implementation screenshot shows all three filmstrip photos rendered.
- [P2] The previous small-window layout could squeeze two evidence panes.
  - Fix: added an A/B pane selector and changed comparison mode to one active pane below 899 px.
  - Post-fix evidence: live half-screen capture showed the A/B selector with only pane A visible; switching remains available without stacking or squeezing.
- [P1] Synchronized zoom previously risked black previews and used pixel-based pan values across different image proportions.
  - Fix: converted pan to normalized rendered-image coordinates and reset synchronized view state deterministically.
  - Post-fix evidence: live client interaction changed both panes from 20% to 25%; both remained visible and aligned.

### Iteration 2

No actionable P0, P1, or P2 differences remain.

## Findings

- [P3] The implementation is denser than the ideation mock because it retains QRaw's production library shell and existing compact control scale. This is an intentional integration choice; enlarging it independently would break visual consistency with adjacent QRaw screens.
- [P3] Portrait photos create black side areas while the source's landscape photos fill the panes. This is required evidence behavior, not a fidelity defect: images must remain uncropped and unstretched.

## Primary interactions tested

- Single-photo review without comparison language or duplicate cards
- Similar-group A/B selection and swap affordance
- Synchronized zoom with both previews remaining visible
- Horizontal and portrait fit behavior
- Half-screen A/B pane switching
- Review queue navigation
- Full filmstrip preview loading

## Residual test gaps

- A real mixed-orientation group was not produced by the grouping engine during this QA run. The shared viewport was verified in code and separately with portrait and landscape results; mixed-orientation synchronized pan uses normalized image coordinates.

## Final result

final result: passed

---

## Previous device-path QA

The following evidence remains part of the smart-culling release history and is
not superseded by the review UI validation above.

- Approved source: `docs/prototypes/smart-culling-lifecycle/screenshots/10-unsupported.png`
- Production build: `src-tauri/target/debug/bundle/macos/RapidRAW.app`
- Side-by-side comparison: `/Users/qiaopeng/.codex/visualizations/2026/07/15/019f6390-0ad4-7973-bf4a-c5822e85e4b6/smart-culling-unsupported-comparison.png`

### Validated result

- The QRaw shell, lifecycle header, centered device card, three capability rows,
  safety notice, candidate-device dialog, and Library return action match the
  approved visual direction.
- Candidate-device guidance opens and closes in the production desktop build.
- The original production preflight rejection was traced to OCEC using the
  legacy Core ML model format with an unbounded batch dimension.
- The production path now uses Core ML MLProgram with OCEC batch fixed to one,
  while ONNX Runtime CPU EP fallback remains disabled.
- The rebuilt desktop application entered the real review workbench with 17
  analyzed photos on Apple M4 Max. No mock or hidden CPU fallback was added for
  QA.

### Remaining release QA

- Render and compare the remaining lifecycle states from the validated Core ML
  path; the production review state now passes runtime smoke QA.
- Validate the supported-device matrix on additional Apple Silicon models and
  Windows DirectML hardware.
- Run the 1,000-photo performance corpus and the frozen photography-effect
  validation corpus before marking the feature release-ready.

---

# 2026-08-02 安全决策队列 UI 重构 — 当前设计 QA

- Source visual truth: `/Users/qiaopeng/.codex/generated_images/019fbb7e-a87b-74c0-a89a-1453b64915f6/exec-13c8e266-8956-4447-8a49-2bb374a3b7f7.png`
- Source pixels: 1672 × 941
- Intended implementation viewport: 1280 × 720 desktop content area, dark theme, Smart Culling full lifecycle / review queue state
- Implementation screenshot: unavailable
- Implementation pixel dimensions and density normalization: unavailable because a native-window capture could not be obtained; no comparison normalization was performed

**Findings**

- [P1] 本轮无法取得可比较的原生实现截图。
  Location: macOS desktop client / Smart Culling feature view.
  Evidence: `target/debug/RapidRAW` successfully started against the existing Vite server, but the desktop-control runtime could not enumerate or inspect the native process (`RapidRAW` is reported as an invalid app target). A freshly packaged `RapidRAW.app` also did not appear as a capturable application. Therefore there is no screenshot at the required state and viewport.
  Impact: source design and rendered implementation cannot be put into the same comparison input. It would be misleading to claim visual fidelity from code or build output alone.
  Fix: make the native Tauri window available to the desktop-control runtime (macOS Screen Recording and Accessibility permission), open the Smart Culling review state at 1280 × 720, capture it, normalize it against the source visual, and run the comparison loop.

**Open Questions**

- None for product behavior. The remaining blocker is capture access, not an unresolved UI decision.

**Implementation Checklist**

1. Grant the review desktop-control tool macOS Screen Recording and Accessibility access.
2. Relaunch the native client, enter the Smart Culling configuration and review queue states, and capture the same 1280 × 720 state as the selected visual.
3. Compare the source and implementation in one image input; record any P0–P2 findings, apply fixes, then repeat the capture.

**Follow-up Polish**

- After capture access is restored, compare the compact filters, pending-queue header, optional key-person drawer, evidence drawer, and confirmation card at full view and focused scale.

## Comparison history

1. Build verification completed: Vite production build succeeds. This is not visual QA evidence.
2. Native launch verification completed: the Rust/Tauri executable launches, and a debug macOS app bundle builds successfully, but neither exposes a controllable or capturable native window to the QA runtime.

final result: blocked
