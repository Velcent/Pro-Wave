# Changelog

What changed in each Pro Wave release. The release workflow copies the section of the version
being released into the GitHub Release notes, so keep one `## <version>` heading per release.

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
