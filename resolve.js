const fs = require('fs');

const files = [
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
];

files.forEach(f => {
    try {
        let content = fs.readFileSync(f, 'utf-8');
        let newContent = content.replace(/<<<<<<< HEAD\n([\s\S]*?)\n=======\n([\s\S]*?)\n>>>>>>> upstream\/main/g, '$2');
        fs.writeFileSync(f, newContent, 'utf-8');
        console.log(`Resolved ${f}`);
    } catch (e) {
        console.error(`Error resolving ${f}: ${e}`);
    }
});
