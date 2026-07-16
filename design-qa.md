# Smart Culling production design QA

Status: **Core ML device path fixed and runtime review passed; full-lifecycle visual comparison remains**.

## Evidence compared

- Approved source: `docs/prototypes/smart-culling-lifecycle/screenshots/10-unsupported.png`
- Production build: `src-tauri/target/debug/bundle/macos/RapidRAW.app`
- Side-by-side comparison: `/Users/qiaopeng/.codex/visualizations/2026/07/15/019f6390-0ad4-7973-bf4a-c5822e85e4b6/smart-culling-unsupported-comparison.png`

## Result

- The QRaw shell, lifecycle header, centered device card, three capability rows, safety notice, candidate-device dialog and Library return action match the approved visual direction.
- Candidate-device guidance opens and closes in the production desktop build.
- The original production preflight rejection was traced to OCEC using the legacy Core ML model format with an unbounded batch dimension.
- The production path now uses Core ML MLProgram with OCEC batch fixed to one, while ONNX Runtime CPU EP fallback remains disabled.
- The rebuilt desktop application entered the real review workbench with 17 analyzed photos on this Apple M4 Max. No mock or hidden CPU fallback was added for QA.

## Remaining release QA

- Render and compare the remaining lifecycle states from the validated Core ML path; the production review state now passes runtime smoke QA.
- Validate the supported-device matrix on additional Apple Silicon models and Windows DirectML hardware.
- Run the 1,000-photo performance corpus and the frozen photography-effect validation corpus before marking the feature release-ready.
