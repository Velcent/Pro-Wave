# Changelog

What changed in each Pro Wave release. The release workflow copies the section of the version
being released into the GitHub Release notes, so keep one `## <version>` heading per release.

## 0.1.4

### All systems (Windows, macOS, Linux)

- **Fixed: piano sounds went to `inf`.** The Piano patches (Mellow Grand and the other grands)
  and Quick Sampler's Chopped Vocal made the synth's filter blow up: the channel meter stuck at
  `inf`, clipped red and never reset, and the output could go silent or full-scale. The filter
  now stays within its stable range, a voice that ever goes non-finite is silenced and reset,
  and meters always fall back to a real value. A test plays every Library patch and checks
  every sample.
- **Undo and Redo** (Edit menu, ⌘Z / ⇧⌘Z, Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y on Windows and
  Linux): every edit to tracks, regions, notes, the mixer, tempo and signature can be undone,
  up to 100 steps. Edit > Delete Undo History clears them.
- **Track > New** adds a software instrument track (it did nothing before).
- **Menu entries that are not implemented yet are greyed out** instead of looking enabled and
  doing nothing. Newly working: Edit > Delete and Deselect All, Navigate > Go to End, Left /
  Right Locator, Next / Previous Region, Zoom In / Out / to Fit.
- **Right-click menus** in the Tracks area (on macOS also Control-click):
  - regions: Open in Piano Roll, Mute Region, Repeat Region, Set Locators by Region, Delete
  - track headers: new tracks, Create/Release Track Stack, Mute, Solo, Reset Track Height and
    Delete Track (also in *Track > Delete Track*)
  - empty lanes: new tracks and Zoom to Fit
- **Small screens** (such as a 13-inch MacBook Air): the window opens filling the screen, the
  interface scales down when the window is too narrow for it, and the Tracks toolbar leaves
  out its least-used buttons instead of drawing them on top of each other. *View > Bigger /
  Smaller / Actual Interface Size* (Cmd/Ctrl + = / - / 0) set the size yourself; it is
  remembered. The Mixer starts at about half the window's height, and the window cannot be made
  smaller than 800 x 520. The Mixer toolbar and the control bar (volume, count-in, metronome)
  no longer draw on top of each other in narrow windows.
- **Lighter:** the window uses one graphics API (OpenGL on Windows and Linux when it works,
  Metal on macOS) instead of starting all of them: about 100 MB less memory and a faster start.
  `WGPU_BACKEND=dx12` (or `vulkan`, `gl`) picks one by hand if drawing goes wrong.
- **Benchmarks:** `cargo xtask bench` checks the lightweight targets (start under 400 ms, idle
  memory under 150 MB, binary under 50 MB, 24 tracks under 15 % of a core); CI runs it on macOS
  and Windows. On Windows: 320 ms start, 125 MB idle, 3.5 % of a core for 24 tracks.

### Windows and Linux

- **No title bar.** The menu bar row is the top of the window, with minimize, maximize and
  close buttons at its right end. Drag the empty part of the menu bar to move the window (it
  snaps to screen edges as before), double-click it to maximize, and drag the window's edges to
  resize it. On Linux this also removes the tall header that GNOME and other Wayland desktops
  drew above the window.

### macOS

- The menu bar entries follow the same rules: what is not implemented yet is greyed out, and
  Undo / Redo are enabled only when there is something to undo or redo.

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
