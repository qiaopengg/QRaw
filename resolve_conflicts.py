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

for f in files:
    try:
        with open(f, "r", encoding="utf-8") as file:
            content = file.read()
            
        # The regex to match git conflict markers
        # Group 1 is HEAD (our changes)
        # Group 2 is upstream (their changes)
        # We will replace the whole block with Group 2 (upstream)
        new_content = re.sub(
            r'<<<<<<< HEAD\n(.*?)\n=======\n(.*?)\n>>>>>>> upstream/main',
            r'\2',
            content,
            flags=re.DOTALL
        )
        
        with open(f, "w", encoding="utf-8") as file:
            file.write(new_content)
        print(f"Resolved {f}")
    except Exception as e:
        print(f"Error resolving {f}: {e}")
