import os
import re

files = [
    "src/App.tsx",
    "src/components/adjustments/Color.tsx",
    "src/components/modals/DenoiseModal.tsx",
    "src/components/panel/MainLibrary.tsx",
    "src/components/panel/SettingsPanel.tsx",
    "src/components/panel/editor/ImageCanvas.tsx",
    "src/components/panel/right/AIPanel.tsx",
    "src/components/panel/right/ExportPanel.tsx",
    "src/components/panel/right/LibraryExportPanel.tsx",
    "src/components/panel/right/Masks.tsx",
    "src/components/panel/right/MasksPanel.tsx",
    "src/hooks/useThumbnails.tsx"
]

log_content = ""

for f in files:
    try:
        with open(f, "r", encoding="utf-8") as file:
            content = file.read()
            matches = re.finditer(r'<<<<<<< HEAD\n(.*?)\n=======\n(.*?)\n>>>>>>> upstream/main', content, re.DOTALL)
            for i, match in enumerate(matches):
                log_content += f"\n--- {f} CONFLICT {i+1} ---\n"
                log_content += "[HEAD (Our TS enhancements)]:\n"
                log_content += match.group(1) + "\n"
                log_content += "[UPSTREAM (Their new features)]:\n"
                log_content += match.group(2) + "\n"
    except Exception as e:
        log_content += f"Error reading {f}: {e}\n"

with open("conflicts_log.txt", "w", encoding="utf-8") as out:
    out.write(log_content)
