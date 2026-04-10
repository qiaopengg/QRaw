<p align="center">
  <img src="https://raw.githubusercontent.com/CyberTimon/RapidRAW/assets/.github/assets/editor.png" alt="QRaw Editor">
</p>

<div align="center">

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![wgpu](https://img.shields.io/badge/wgpu-%23282C34.svg?style=for-the-badge&logo=webgpu&logoColor=white)](https://wgpu.rs/)
[![React](https://img.shields.io/badge/react-%2320232a.svg?style=for-the-badge&logo=react&logoColor=%2361DAFB)](https://react.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![AGPL-3.0](https://img.shields.io/badge/License-AGPL_v3-blue.svg?style=for-the-badge)](https://opensource.org/licenses/AGPL-3.0)
[![GitHub stars](https://img.shields.io/github/stars/qiaopengg/QRaw?style=for-the-badge&logo=github&label=Stars)](https://github.com/qiaopengg/QRaw/stargazers)

</div>

# QRaw

> A beautiful, non-destructive, and GPU-accelerated RAW image editor built with performance in mind.

QRaw is a secondary development fork of [RapidRAW](https://github.com/CyberTimon/RapidRAW) by [CyberTimon (Timon Käch)](https://github.com/CyberTimon), licensed under AGPL-3.0. It delivers GPU-accelerated, non-destructive RAW editing in a lightweight package for Windows, macOS, and Linux.

<table width="100%">
  <tr>
    <td width="50%" valign="top" align="center">
      <br>
      <h3>Download QRaw</h3>
      <p>Get the latest release for Windows, macOS, and Linux.</p>
      <strong><a href="https://github.com/qiaopengg/QRaw/releases/latest">Download Latest Version →</a></strong>
      <br><br>
    </td>
    <td width="50%" valign="top" align="center">
      <br>
      <h3>Upstream Project</h3>
      <p>QRaw is based on QRaw. For upstream docs and tutorials:</p>
      <strong><a href="https://github.com/CyberTimon/RapidRAW">QRaw by CyberTimon →</a></strong>
      <br><br>
    </td>
  </tr>
</table>

---

- **2026-03-26:** Performance improvements & new flat list mode for library
- **2026-03-25:** Optimize folder loading & tree fetching
- **2026-03-23:** Generate thumbnails only for visible viewport items
- **2026-03-22:** Dependency migrations and other bug fixes
- **2026-03-21:** Colored sliders for temperature and tint
- **2026-03-18:** Implemented AI NIND denoising
- **2026-03-16:** LRU cache for instant image loading
- **2026-03-15:** Improved high quality subject mask models, various UI improvements and shader improvements
- **2026-03-14:** New image analytics panel which can display vectorscopes, waveforms, parades & histograms
- **2026-03-13:** JPEG XL, WebP, and additional format support, including the ability to export LUTs.

<details>
<summary><strong>Expand further</strong></summary>

- **2026-03-12:** Added parametric color & luminance masks
- **2026-03-10:** Implement region of interest rendering to improve performance when zooming in
- **2026-03-07:** Batch negative conversion & various shader improvements
- **2026-03-06:** Performance optimizations and UI cleanup
- **2026-03-05:** Initial draw support for linear & radial masks
- **2026-03-04:** Real-time mask overlay rendering & pixel perfect zooming
- **2026-03-03:** Instant image rendering & real-time histogram update
- **2026-03-02:** Remember last export settings & lens correction auto cropping
- **2026-03-01:** Optimized pixelated interpolation at maximum zoom level
- **2026-02-27:** Refactored fullscreen handling, smooth and integrated fullscreen viewer
- **2026-02-24:** Improved tonal adjustments using detail masks, remember zoom level & faster fullscreen preview
- **2026-02-23:** Custom AI tag lists, clear button for tag settings & improved window state restoration
- **2026-02-23:** Improved RAW processing, incorrect thumbnail crop scaling & improved mask handles
- **2026-02-21:** XMP metadata read/sync
- **2026-02-20:** Main window size/position persistence, right-click history dropdown & new library organization panel
- **2026-02-19:** Exponential zoom scaling, right-click to delete curve points & selected image count display
- **2026-02-18:** Added a setting for Linear RAW mode for advanced processing & improved right panel switcher
- **2026-02-17:** Display RAW image counts in the folder tree & improved folder reading performance
- **2026-02-16:** New composition guide overlays for cropping
- **2026-02-16:** Added the ability to export masks as separate images
- **2026-02-13:** Optimized live previews, instant metadata loading and new jpeg encoder
- **2026-02-13:** Added ability to merge multiple bracketed images to a HDR
- **2026-02-12:** Straight brush mask lines using shift click and enhanced Lensfun DB parsing
- **2026-02-10:** Improved image loading performance
- **2026-02-06:** Refactored negative conversion logic using characteristic curves.
- **2026-02-04:** Global tooltips & major UI polish
- **2026-02-03:** New creative effects: Glow, Halation & Lens Flares
- **2026-01-31:** Accurate color noise reduction for RAW images & improved image loading
- **2026-01-30:** Enhanced Lensfun DB parsing and improved lens matching logic
- **2026-01-29:** Add cross-channel copy/paste & flat-line clipping logic for curves
- **2026-01-26:** Favorite lens saving, improved rotation controls (finer grid), better local contrast adjustments
- **2026-01-25:** Filmstrip performance boost, improved sorting, lens distortion fixes for AI masks & crop
- **2026-01-24:** Added automatic lens, TCA & vignette correction using lensfun
- **2026-01-22:** Improved and centralized EXIF data handling for greater accuracy and support
- **2026-01-21:** Inpainting now works correctly on images with geometry transformations
- **2026-01-20:** Export preset management for saving export settings
- **2026-01-19:** Preload library for faster startup & automatic geometry transformation helper lines
- **2026-01-18:** Implement image geometry transformation utils
- **2026-01-17:** Refactor AI panel to correctly work with the new masking system
- **2026-01-16:** Major masking system overhaul with drag & drop, per-mask opacity/invert & UI improvements
- **2026-01-13:** New python middleware client for external generative AI integration (ComfyUI)
- **2026-01-12:** Created a QRaw community discord server
- **2026-01-11:** Separate preview worker, optional high-quality live previews & mask/ai patch caching
- **2026-01-10:** Enhanced EXIF UI, optimized color wheels/curves & rawler update
- **2026-01-09:** Live previews for all adjustments & masks with optimized GPU processing
- **2026-01-05:** Collage maker upgrade (drag & drop, zoom, ratio options)
- **2026-01-05:** 'Prefer RAW' filter option added to library
- **2026-01-05:** Support for uppercase file extensions
- **2026-01-05:** Flush thumbnail cache on folder switch
- **2025-12-27:** Fix LUT banding issues with improved sampling
- **2025-12-26:** AI masking stability improvements under load
- **2025-12-23:** Metadata card in toolbar & context menu export
- **2025-12-23:** Monochromatic grain & white balance picker improvements
- **2025-12-22:** BM3D Denoising with comparison slider
- **2025-12-20:** Batch export stability improvements & RAM optimization
- **2025-12-14:** Exposure slider added to masking tools
- **2025-12-14:** Improved delete workflow
- **2025-12-08:** Improved mask eraser tool behavior & ORT v2 migration
- **2025-12-07:** Write EXIF metadata to file
- **2025-12-07:** Color picker for white balance
- **2025-11-30:** HSL luminance artifacts fix
- **2025-11-29:** Improved mask stacking & many bug fixes
- **2025-11-28:** QOI support
- **2025-11-25:** Update rawler
- **2025-11-23:** Recursive library view to display images from all subfolders
- **2025-11-22:** DNG loader improvements
- **2025-11-18:** Improved vibrancy adjustment
- **2025-11-15:** Virtual copies & library improvements
- **2025-11-14:** Open-with-file cross plattform compatibilty & single instance lock
- **2025-11-13:** Rewritten tagging system to support pill-like image tagging
- **2025-11-10:** Improved folder tree with search functionality
- **2025-11-08:** Added EXR file format support
- **2025-11-XX:** Improving AgX
- **2025-11-02:** Optimize image loading & add processing engine settings
- **2025-10-31:** Expose highlights compression point to user & improve keybinds detection
- **2025-10-28:** Copy paste settings & brightness adjustment
- **2025-10-XX:** Working on tonemapping - ongoing...
- **2025-10-24:** Getting AgX right isn't as easy as it seems :=)
- **2025-10-22:** AgX tone mapping
- **2025-10-19:** Whole image mask component & organize mask components better
- **2025-10-19:** You can now apply presets to masks & improved auto adjustments
- **2025-10-17:** New centré adjustment, rawler now as a submodule & improved logger
- **2025-10-15:** Ability to pin folders, improved session handling & smooth library thumbnail updating
- **2025-10-11:** Realistic, complex & non-dulling exposure & highlights slider
- **2025-10-11:** Smooth filmstrip thumbnail updates
- **2025-10-07:** New watermarking support
- **2025-10-06:** Improve crop quality by transforming before scaling
- **2025-10-XX:** Many small improvements - ongoing...
- **2025-09-27:** Sort library by exif metadata & release cleanup / bug fixes
- **2025-09-26:** Collage maker to create unique collages with many different layouts, spacing & border radius
- **2025-09-23:** Color calibration tool to adjust RGB primaries & adjustments visibility settings
- **2025-09-22:** Issue template & CI/CD improvements
- **2025-09-20:** Universal presets importer, prioritize dGPU & improved local contrast tools (sharpness, clarity etc.)
- **2025-09-17:** Automatic image culling (duplicate & blur detection)
- **2025-09-14:** Grid previews in community panel & improved ComfyUi workflow
- **2025-09-12:** New community presets panel to share & showcase presets
- **2025-09-10:** Extended generative AI roadmap & started building QRaw website
- **2025-09-09:** Many shader improvements & bug fixes, invert tint slider
- **2025-09-06:** New update notifier that alerts users when a new version becomes available
- **2025-09-04:** Added toggleable clipping warnings (blue = shadows, red = highlights)
- **2025-09-02:** Transition to Rust 2024 & Cache image on GPU
- **2025-08-31:** Cancel thumbnail generation on folder change & optimized ai patch saving
- **2025-08-30:** Optimize ComfyUI image transfer & speed
- **2025-08-28:** Chromatic aberration correction & Shader improvements
- **2025-08-26:** User customisable ComfyUI workflow selection
- **2025-08-25:** Make LUTs parser more robust (support more advanced formats)
- **2025-08-24:** Improved keyboard shortcuts
- **2025-08-23:** Estimate file size before exporting
- **2025-08-21:** Added LUTs (.cube, .3dl, .png, .jpg, .jpeg, .tiff) support
- **2025-08-16:** Fast AI sky masks
- **2025-08-15:** Show full resolution image when zooming in
- **2025-08-15:** Implement Tauri's IPC as a replacement for the slow Base64 image transfer
- **2025-08-12:** Relative zoom indicator
- **2025-08-11:** TypeScript cleanup & many bug fixes
- **2025-08-09:** Local inpainting without the need for ComfyUI, ability to change thumbnail aspect ratio
- **2025-08-09:** Frontend refactored to TypeScript thanks to @varjolintu
- **2025-08-08:** New onnxruntime download strategy & the base for local inpainting
- **2025-08-05:** Improved HSL cascading, UI & animation improvements, ability to grow & shrink / feather AI masks
- **2025-08-03:** New high performance, seamless image panorama stitcher (without any dependencies on OpenCV)
- **2025-08-02:** Added an image straightening tool and improved crop & rotation functionality (especially on portrait images)
- **2025-08-02:** A new dedicated image importer, ability to rename and batch rename files, improved dark theme, and other fixes
- **2025-07-31:** Ability to tag & filter images by color labels, refactored image right clicking
- **2025-07-31:** Reimplemented the functionality of GPU processing (GPU cropping, etc.) -> No longer dependent on TEXTURE_BINDING_ARRAY
- **2025-07-29:** Refactored generative AI foundation, many small fixes
- **2025-07-27:** Automatic AI image tagging, overall mask transparency setting per mask
- **2025-07-25:** Fuji RAF X-Trans sensor support (new x-trans demosaicing algo)
- **2025-07-24:** Auto crop when cropping an image (to prevent black borders), added drag & drop sort abilty to presets panel
- **2025-07-22:** Significant improvements to the shader: More accurate exposure slider, better tone mapper (simplified ACES)
- **2025-07-21:** Remember scroll position when going into the editing section
- **2025-07-20:** Ability to add presets to folders, export preset folders etc, preset _animations_
- **2025-07-20:** Tutorials on how to use QRaw
- **2025-07-19:** Initial color negative conversion implementation, shader improvements
- **2025-07-19:** New color wheels, persistent collapsed / expanded state for UI elements
- **2025-07-19:** Fixed banding & purple artefacts on RAW images, better color noise reduction, show exposure in stops
- **2025-07-18:** Smooth zoom slider, new adaptive editor theme setting
- **2025-07-18:** New export functionality: Export with metadata, GPS metadata remover, batch export file naming scheme using tags
- **2025-07-18:** Ability to delete the associated RAW/JPEG in right click delete operations
- **2025-07-17:** Small bug fixes
- **2025-07-13:** Native looking titlebar and ability to input precise number into sliders
- **2025-07-13:** Huge update to masks: You can now add multiple masks to a mask containers, subtract / add / combine masks etc.
- **2025-07-12:** Improved curves tool, more shader improvements, improved handling of very large files
- **2025-07-11:** More accurate shader, reorganized main library preferences dropdown, smoother histogram, more realistic film grain
- **2025-07-11:** Added a HUD-like waveform overlay toggle to display specific channel waveforms (w-key)
- **2025-07-10:** Rewritten batch export system and async thumbnail generation (makes the loading of large folders a lot more fluid)
- **2025-07-10:** Window transparency can now be toggled in the settings, thanks to @andrewazores
- **2025-07-08:** Ability to toggle the visibility of individual adjustments sections
- **2025-07-08:** Fixed top-left zoom bug, corrected scale behavior in crop panel, keep default original aspect ratio
- **2025-07-08:** Added image rating filter and redesigned the metadata panel with improved layout, clearer sections, and an embedded GPS map
- **2025-07-07:** Improved generative AI features and updated [AI Roadmap](#ai-roadmap)
- **2025-07-06:** Initial generative AI integration with [ComfyUI](https://github.com/comfyanonymous/ComfyUI) - for more details, checkout the [AI Roadmap](#ai-roadmap)
- **2025-07-05:** Ability to overwrite preset with current settings
- **2025-07-04:** High speed and precise cache to significantly accelerate large image editing
- **2025-07-04:** Greatly improved shader with better dehaze, more accurate curves etc
- **2025-07-04:** Predefined 90° clockwise rotation and ability to flip images
- **2025-07-03:** Switched from [rawloader](https://github.com/pedrocr/rawloader) to [rawler](https://github.com/dnglab/dnglab/tree/main/rawler) to support a wider range of RAW formats
- **2025-07-02:** AI-powered foreground / background masking
- **2025-06-30:** AI-powered subject masking
- **2025-06-30:** Precompiled Linux builds
- **2025-06-29:** New 5:4 aspect ratio, new low contrast grey theme and more cameras support (DJI Mavic lineup)
- **2025-06-28:** Release cleanup, CI/CD improvements and minor fixes
- **2025-06-27:** Initial release. For more information about the earlier progress, look at the [Initial Development Log](#initial-development-log)

All core functionality is inherited from the upstream QRaw project.

---

## Key Features

Inherited from QRaw — see the [upstream README](https://github.com/CyberTimon/RapidRAW#key-features) for the full feature list.

- **GPU-Accelerated:** Full 32-bit image processing pipeline written in WGSL.
- **Masking:** Layer-based masking with AI subject, sky and foreground detection.
- **Full RAW Support:** Wide range of RAW camera formats via rawler.
- **Non-Destructive Workflow:** All edits stored in `.qcr` sidecar files.
- **Lens Correction:** Automatic distortion, TCA, and vignette correction via Lensfun.
- **Professional Adjustments:** Tone mapping (AgX), curves, color grading, HSL, noise reduction, and more.

---

## Getting Started

**Build from Source**

Requires [Rust](https://www.rust-lang.org/tools/install) and [Node.js](https://nodejs.org/).

```bash
# 1. Clone the repository
git clone https://github.com/CyberTimon/RapidRAW.git
cd QRaw

# Install frontend dependencies
npm install

# Build and run
npm start
```

## System Requirements

- **Windows:** Windows 10 or newer
- **macOS:** macOS 13 (Ventura) or newer
- **Linux:** Ubuntu 22.04+ or compatible modern distribution
- **RAM:** 16GB or more recommended
- **GPU:** Dedicated GPU recommended (wgpu-based pipeline)

---

## License & Attribution

This project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**.

QRaw is a modified version of [RapidRAW](https://github.com/CyberTimon/RapidRAW), originally developed by [Timon Käch (CyberTimon)](https://github.com/CyberTimon). The original work is copyright © Timon Käch and contributors. This fork carries prominent notice of modification as required by AGPL-3.0 Section 5.

See the [LICENSE](LICENSE) file for full details.
