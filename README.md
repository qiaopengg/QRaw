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
      <p>QRaw is based on RapidRAW. For upstream docs and tutorials:</p>
      <strong><a href="https://github.com/CyberTimon/RapidRAW">RapidRAW by CyberTimon →</a></strong>
      <br><br>
    </td>
  </tr>
</table>

---

## About This Fork

QRaw is a personal fork of RapidRAW with the following customizations:

- Chinese (Simplified) localization improvements
- UI/UX tweaks for personal workflow
- Window size and layout optimizations

All core functionality is inherited from the upstream RapidRAW project.

---

## Key Features

Inherited from RapidRAW — see the [upstream README](https://github.com/CyberTimon/RapidRAW#key-features) for the full feature list.

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
# Clone with submodules
git clone https://github.com/qiaopengg/QRaw.git --recurse-submodules
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
