# Changelog

What changed in each Pro Wave release. The release workflow copies the section of the version
being released into the GitHub Release notes, so keep one `## <version>` heading per release.

## 0.1.7

### All systems (Windows, macOS, Linux)

✅ **Tempo track:** *Track > Show Tempo Track*. Double-click to add a tempo change, drag a point to change or move it, double-click or right-click to delete. Saved with the project and undoable; the control bar shows the tempo at the playhead.
✅ **Marker track:** *Track > Show Marker Track*. Name sections (Intro, Verse, Chorus...): double-click to add, click to jump there, drag to move, double-click to rename, right-click for colour or delete. *Navigate > Create Marker*, *Delete Marker*, *Go to Next / Previous Marker*.
✅ **Audio region edits:** Region Inspector *Gain*, *Fade In*, *Fade Out*, *Curve* (Equal Power, Linear, S-Curve) and *Reverse*; right-click *Normalize Region Gain* and *Reverse Region*. The audio file is never changed. Fades show on the region and the waveform follows gain and reverse. Saved with the project.
✅ **Compact menus like Logic:** each menu is as wide as its longest entry instead of stretching across the window. The File menu follows Logic, with ▸ submenus (Open Recent, Revert to, Import, Export, Bounce, Share...), also in the macOS menu bar. Entries that aren't built yet are greyed out.
✅ **24 tracks without dropouts:** 60 seconds of 24 stereo 48 kHz / 24-bit audio tracks: 0 dropouts, 1.6 % CPU (Windows)
✅ **Lighter audio regions:** the read-ahead buffer of each audio region follows the size of the song: at most 0.68 s per region and 8 MiB for all regions together (up to 128 regions), then 0.17 s each. Before it was a fixed 0.5 MB per region (about 12 MB for 24 tracks).
✅ **More efficient reverse playback:** reuses decoded audio between blocks to reduce repeated disk seeks; automated checks cover resampling and jumping within a reversed region
✅ **Playback during large imports:** the disk worker refills active regions between batches of new-file requests and skips canceled regions before opening their files
✅ **Lighter region playback:** reuses calculated region timing between edits and skips unnecessary fade-curve calculations outside the fade
✅ **Lighter mixer updates:** during playback, volume and pan edits recalculate gain targets for the changed channel while still updating mute and solo across channels
✅ **Steadier playback:** broken values (from a damaged project or a bad setting) can no longer make noise or stop the sound
✅ **No freeze when normalizing:** *Normalize Region Gain* reads the file in the background (a long recording used to freeze the window for seconds); the status line shows "Normalizing…".
✅ **SMPTE timecode fix:** a position exactly on a frame was shown one frame early (up to 8 % of frame-exact positions at 25 fps).
✅ **Lighter transport** when the song has tempo changes; tempo ramps avoid repeated logarithm calculations between tempo boundaries
✅ **17 languages:** *Help > Language*: English (default), 简体中文, 日本語, Français, Deutsch, Español, Português (Portugal), Português (Brasil), Filipino, Bahasa Indonesia, हिन्दी, Українська, Norsk, Svenska, Suomi, Polski, Lietuvių. Each language is listed in its own name; the choice switches the whole interface at once (menus, panels, dialogs, messages, also the macOS menu bar) and is kept for the next start. Chinese, Japanese and Hindi use the system's fonts.

✅ **Still light:** about 360 ms to start and 120 MB of memory at rest (Windows), 13 MB program

### Engine groundwork (not in the interface yet)

✅ Tempo ramps (gradual tempo changes)
✅ SMPTE timecode at 24, 25, 29.97 (drop / non-drop) and 30 fps
✅ Windows ASIO for low latency, as an optional build feature

### Still to do or test

❌ Region gain, fades and reverse heard on real recordings (checked by automated tests only)
❌ Dragging fades directly on a region (Fade tool) and crossfades
❌ Tempo ramps and SMPTE display in the interface
❌ Several time signatures in one song (Signature track)
❌ ASIO in the Windows download, and its latency measured on real hardware
❌ macOS menu bar submenus tried on a real Mac
❌ Translations checked by native speakers (written by the AI, verified by automated tests for completeness, placeholders and fonts)

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
