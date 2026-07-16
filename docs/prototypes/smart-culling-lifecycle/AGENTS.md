# Prototype Instructions

Run the local server yourself and open the preview in the browser available to this environment. Do not give the user server-start instructions when you can run it.

Before making substantial visual changes, use the Product Design plugin's `get-context` skill when the visual source is unclear or no longer matches the current goal. When the user gives durable prototype-specific design feedback, preferences, or decisions, record them in `AGENTS.md`.

When implementing from a selected generated mock, treat that image as the source of truth for layout, component anatomy, density, spacing, color, typography, visible content, and hierarchy.

## Smart Culling Prototype Decisions

- Use the first generated Product Design option selected by the user as the visual source of truth.
- Preserve QRaw's dark desktop style, compact professional density, restrained surfaces, three-column review layout, white primary action, and green/yellow/red culling labels.
- This prototype must cover the complete smart-culling lifecycle, not only the review workbench.
- Include visible, interactive states for Library entry, device/model preflight, configuration, key-person selection, background analysis, cancellation/partial completion, completion notification, pending-review entry, review, manual correction and protection, final confirmation, sidecar write failures/retry, successful Library result display, and unsupported-device handling.
- Keep P0 technical validation and P1-P3 delivery layering visible where it changes user expectations, without exposing internal engineering jargon as primary UI copy.
- This is a self-contained design prototype. Do not modify QRaw production feature code from this folder.
