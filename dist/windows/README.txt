Pro Wave - Windows x64
======================

Pro Wave is a DAW (music production app) in the style of Apple Logic Pro, written in Rust.
This is an early development build: expect missing features and rough edges.

Run it
------

1. Unzip the folder anywhere (it needs no installer and writes nothing outside your user
   folders).
2. Double-click prowave.exe.

Windows may show "Windows protected your PC" (SmartScreen) because the program is not code
signed yet. Choose "More info", then "Run anyway".

From a terminal:
  prowave.exe song.prowave          open a project
  prowave.exe drums.wav bass.flac   start a new project and import audio files
  prowave.exe --play                start playing right away
  prowave.exe --piano               open the Piano Roll on start

Audio files supported: WAV, AIFF, FLAC. You can also drag audio files onto the window.

Requirements
------------

- Windows 10 or 11, 64-bit.
- A graphics driver with DirectX 12 or Vulkan (any recent Intel, AMD or NVIDIA driver).
- Sound goes through WASAPI (shared mode). ASIO is not built in yet.

Where it keeps things
---------------------

- The list of recent projects: %APPDATA%\prowave\recent.txt
- Projects you open are unpacked to a temporary folder (%TEMP%\prowave-media) while they are
  open; it is cleaned up when you close the program.
- If it crashes, it prints the path of a crash report in %TEMP%.

Keyboard shortcuts
------------------

  Ctrl+N new    Ctrl+O open    Ctrl+S save    Ctrl+Shift+S save as
  Space play/stop    Ctrl+R repeat region    Delete delete selection
  Y Library    I Inspector    X Mixer    Q quantize (Piano Roll)

More: https://github.com/fabox-os/Pro-Wave
Licensed under the GNU General Public License v3 or later (see LICENSE).
