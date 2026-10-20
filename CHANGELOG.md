# Changelog

What changed in each Pro Wave release. The release workflow copies the section of the version
being released into the GitHub Release notes, so keep one `## <version>` heading per release.

## 0.1.4

### All systems (Windows, macOS, Linux)

✅ **Fixed: piano sounds going to `inf`.** Mellow Grand, the other grands and Chopped Vocal made the filter blow up (meter stuck on `inf`, red clipping). The filter stays stable now and meters always reset.
✅ **Undo / Redo:** Edit menu, ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Y on Windows & Linux), up to 100 steps
✅ **Track > New** now adds a track
✅ **Unfinished menu items are greyed out** instead of doing nothing
✅ **Newly working:** Edit > Delete / Deselect All, Go to End, Locators, Next / Previous Region, Zoom In / Out / to Fit
✅ **Right-click menus** on regions, track headers and empty lanes (Control-click on Mac)
✅ **Small screens (13" MacBook Air):** UI scales to fit, toolbars no longer overlap, View > Bigger / Smaller Interface (⌘ = / - / 0)
✅ **Lighter:** ~100 MB less memory, faster start
✅ **Benchmarks:** 320 ms start, 125 MB idle, 3.5 % CPU for 24 tracks (Windows)

### Windows & Linux

✅ **No title bar.** Minimize / maximize / close are in the menu bar. Drag the bar to move, double-click to maximize, drag the edges to resize.
✅ The tall GNOME / Wayland header on Linux is gone
✅ **Smooth maximize and resize:** the interface no longer rescales on every frame while the window changes size, so the animation stays fluid

### macOS

✅ Menu bar items greyed out when not available yet

### Still to test

❌ Piano meter fix on a real Mac
❌ macOS benchmark
❌ Windows maximized with an always-visible taskbar

## 0.1.3

### All systems (Windows, macOS, Linux)

- **Demo song on the first launch only.** The very first time Pro Wave starts it opens the demo
  song; every later start begins with a clean project (one empty instrument track).
- **Project Chooser.** *File > New from Template…* opens a chooser with New Project, your Recent
  projects and Project Templates: Empty Project, Songwriter, Electronic, Podcast and Demo Song.
- **Window title** is now just "Pro Wave", without the project name and "Tracks".
- **Fixed:** the waveforms of audio tracks without a recording (Electric Bass and Lead Vocal in
  the demo) stayed still while the Tracks area scrolled or zoomed. They now move with their
  regions.

### macOS

- **Menus in the macOS menu bar.** The menus now sit in the menu bar at the top of the screen
  instead of inside the window, with a Pro Wave menu (About, Hide, Quit ⌘Q), the ⌘N, ⌘O, ⌘S and
  ⇧⌘S key commands, check marks in the View menu and an Open Recent submenu. Errors and warnings
  show on a thin line at the top of the window.
- **Fixed:** the Dock icon switched from the Pro Wave icon on its rounded tile to the bare logo
  once the app had started. It now keeps the app's icon.

### Windows

- The README and these notes explain what to do when an antivirus (Norton, Avast, AVG, McAfee)
  blocks `prowave.exe` with "Windows cannot access the specified device, path or file".

### Linux

- No Linux-only changes: the .deb, .rpm, AppImage, Flatpak and tarball include all the changes
  listed under *All systems*.

## 0.1.2

- **Aux and Folder tracks.** *Track > New Aux Track*, and *Create Track Stack…* / *Release Track
  Stack* to group tracks in a folder that folds open and closed, with folder Mute and Solo.
  Routing audio through them needs audio engine work that is not done yet.
- **Editable time signature.** Drag the numerator or the denominator in the display up or down.
- **File menu:** Open Recent, Save A Copy As… and Revert to Saved.
- **Windows zip** added to the release downloads, with the Pro Wave icon on `prowave.exe`.
