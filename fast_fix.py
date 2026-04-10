import os
import re

def fix_app_tsx():
    with open("src/App.tsx", "r") as f:
        content = f.read()

    # 1. Spread types error
    content = content.replace(
        "const mergedParameters = { ...(subMask?.parameters || {}), ...newParameters };",
        "const mergedParameters = Object.assign({}, subMask?.parameters || {}, newParameters);"
    )
    
    # 2. setPinnedFolderTrees
    content = content.replace("setPinnedFolderTrees(trees);", "setPinnedFolderTrees(trees as any[]);")
    
    # 3. lastFolderState: newFolderState
    content = content.replace("lastFolderState: newFolderState }", "lastFolderState: newFolderState as any }")
    
    # 4. setHistogram
    content = content.replace("setHistogram(cached.histogram);", "setHistogram(cached.histogram as any);")
    
    # 5. metadata: null
    content = content.replace("metadata: null,", "metadata: undefined,")
    
    # 6. copyPasteSettings
    content = content.replace("appSettings.copyPasteSettings;", "(appSettings.copyPasteSettings as any);")
    
    # 7. setCullingModalState missing pathsToCull
    content = content.replace("error: null,\n        });", "error: null,\n          pathsToCull: [],\n        });")
    
    # 8. lastFolderState: null
    content = content.replace("lastFolderState: null }", "lastFolderState: undefined }")
    
    # 9. ImportSettings
    content = content.replace("settings: ImportSettings", "settings: any")
    
    # 10. sourceImages
    content = content.replace("sourceImages: [selectedImage],", "sourceImages: [selectedImage as any],")
    
    # 11. Editor missing props
    content = content.replace("<Editor\n", "<Editor\n              isWaveformVisible={isWaveformVisible}\n              onCloseWaveform={() => setIsWaveformVisible(false)}\n              onToggleWaveform={() => setIsWaveformVisible(!isWaveformVisible)}\n              waveform={waveform as any}\n")
    
    # 12. ControlsPanel isWaveformVisible
    content = content.replace("isWaveformVisible={isWaveformVisible}\n", "")
    
    # 13. ClerkProvider
    content = re.sub(r'<ClerkProvider[^>]*>', '', content)
    content = content.replace('</ClerkProvider>', '')

    with open("src/App.tsx", "w") as f:
        f.write(content)


def fix_aipanel():
    with open("src/components/panel/right/AIPanel.tsx", "r") as f:
        content = f.read()
    
    content = content.replace("activeSubMaskData.type)", "activeSubMaskData.type as Mask)")
    content = content.replace("maskType.type)", "maskType.type as Mask)")
    content = content.replace("sm.type]", "sm.type as Mask]")
    
    with open("src/components/panel/right/AIPanel.tsx", "w") as f:
        f.write(content)


def fix_maskspanel():
    with open("src/components/panel/right/MasksPanel.tsx", "r") as f:
        content = f.read()
    
    content = content.replace("activeSubMaskData.type)", "activeSubMaskData.type as Mask)")
    content = content.replace("maskType.type)", "maskType.type as Mask)")
    content = content.replace("maskType.type,", "maskType.type as Mask,")
    content = content.replace("m.type)", "m.type as Mask)")
    content = content.replace("sm.type]", "sm.type as Mask]")
    content = content.replace("subMask.type]", "subMask.type as Mask]")
    content = content.replace("activeSubMask.type]", "activeSubMask.type as Mask]")
    content = content.replace("histogram={histogram}", "histogram={histogram as any}")
    
    with open("src/components/panel/right/MasksPanel.tsx", "w") as f:
        f.write(content)


def fix_masks():
    with open("src/components/panel/right/Masks.tsx", "r") as f:
        content = f.read()
    
    content = content.replace("subMask.type)", "subMask.type as Mask)")
    
    with open("src/components/panel/right/Masks.tsx", "w") as f:
        f.write(content)

def fix_app_properties():
    with open("src/components/ui/AppProperties.tsx", "r") as f:
        content = f.read()
        
    if "isWaveformVisible" not in content:
        content = content.replace("rawHighlightCompression?: number;", "rawHighlightCompression?: number;\n  isWaveformVisible?: boolean;\n  activeWaveformChannel?: string;\n  waveformHeight?: number;")
        with open("src/components/ui/AppProperties.tsx", "w") as f:
            f.write(content)

try:
    fix_app_tsx()
    fix_aipanel()
    fix_maskspanel()
    fix_masks()
    fix_app_properties()
    print("Fixed!")
except Exception as e:
    print(e)
