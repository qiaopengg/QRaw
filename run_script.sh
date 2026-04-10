#!/bin/bash
cd /Users/qiaopeng/Desktop/owner/QRaw
git show de446184:src/App.tsx > old_app.tsx
grep -A 20 -B 5 ChatPanel old_app.tsx
