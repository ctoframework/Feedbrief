# Optional fonts

The app will run without these — egui has a built-in default font. But for the
real editorial look (Fraunces serif + JetBrains Mono), download these files into
this folder:

1. **Fraunces** (display serif, OFL licensed)
   - Download: https://fonts.google.com/specimen/Fraunces
   - Extract from the .zip and copy these three into this folder:
     - `Fraunces-Regular.ttf`
     - `Fraunces-Bold.ttf`
     - `Fraunces-Italic.ttf`

2. **JetBrains Mono** (monospace, OFL licensed)
   - Download: https://www.jetbrains.com/lp/mono/
   - From the .zip, copy:
     - `JetBrainsMono-Regular.ttf`

Drop them in this `assets/` folder and rebuild. The app reads them at startup
and falls back gracefully to defaults if any are missing.
