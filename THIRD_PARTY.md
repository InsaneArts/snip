# Third-party software in Snip

Snip is GPL-3.0-or-later ([LICENSE](LICENSE)). This lists what it's built from, what it links, runs and reads, with their licences (for system packages, as Arch packages them). All of them are compatible with the GPL-3.0.

## Bundled with Snip

| What | Where | Licence |
|---|---|---|
| Nunito (font) | `app/fonts/Nunito.ttf` | SIL Open Font License 1.1 ([app/fonts/OFL-Nunito.txt](app/fonts/OFL-Nunito.txt)) |
| Lucide's icon geometry, redrawn as paths | `app/qml/Icon.qml` | ISC |
| The shutter sound | `app/sounds/shutter.wav` | Snip's own (design language, Q-D5) |

## Rust crates built into Snip

144 crates compiled for Linux, by licence. To list them again: `cargo tree --workspace -e normal,build --target x86_64-unknown-linux-gnu --prefix none`, with each crate's licence from `cargo metadata`.

| Licence | Crates | Which |
|---|---|---|
| MIT OR Apache-2.0 | 94 | ahash 0.8.12, anyhow 1.0.104, autocfg 1.5.1, bitflags 2.13.2, cc 1.6.0, cfg-if 1.0.5, chrono 0.4.45, clang-format 0.3.0, core_detect 1.0.0, crc32fast 1.5.2, cxx 1.0.202, cxx-gen 0.7.202, cxx-qt 0.10.0, cxx-qt-build 0.10.0, cxx-qt-gen 0.10.0, cxx-qt-lib 0.10.0, cxx-qt-macro 0.10.0, cxxbridge-flags 1.0.202, cxxbridge-macro 1.0.202, displaydoc 0.2.7, downcast-rs 1.2.1, equivalent 1.0.2, fallible-iterator 0.3.0, fallible-streaming-iterator 0.1.9, fdeflate 0.3.7, find-msvc-tools 0.1.14, flate2 1.1.10, g2gen 1.2.2, g2p 1.2.2, g2poly 1.2.2, getrandom 0.3.4, hashbrown 0.14.5, hashbrown 0.17.1, hashlink 0.9.1, iana-time-zone 0.1.65, image 0.25.10, image-webp 0.2.4, indexmap 2.14.2, indoc 2.0.7, itoa 1.0.18, jobserver 0.1.35, libc 0.2.190, link-cplusplus 1.0.12, log 0.4.34, multiversion_no_op 1.0.0, num 0.4.3, num-bigint 0.4.8, num-complex 0.4.6, num-integer 0.1.47, num-iter 0.1.46, num-rational 0.4.2, num-traits 0.2.19, once_cell 1.21.4, pin-project-lite 0.2.17, pkg-config 0.3.34, png 0.18.1, polling 3.11.0, ppv-lite86 0.2.21, proc-macro2 1.0.107, qt-build-utils 0.10.0, quick-error 2.0.1, quote 1.0.47, rand 0.9.5, rand_chacha 0.9.0, rand_core 0.9.5, regex 1.13.1, regex-automata 0.4.18, regex-syntax 0.8.11, rustversion 1.0.23, scopeguard 1.2.0, semver 1.0.28, serde 1.0.229, serde_core 1.0.229, serde_derive 1.0.229, serde_json 1.0.151, serde_spanned 0.6.9, shlex 2.0.1, simdutf8 0.1.5, smallvec 1.16.2, static_assertions 1.1.0, syn 2.0.119, syn 3.0.6, thiserror 1.0.69, thiserror 2.0.21, thiserror-impl 1.0.69, thiserror-impl 2.0.21, toml_datetime 0.6.11, toml_edit 0.22.27, toml_write 0.1.2, unicode-segmentation 1.13.3, unicode-width 0.1.14, unicode-width 0.2.2, vcpkg 0.2.15, version_check 0.9.5 |
| MIT | 22 | calloop 0.14.5, calloop-wayland-source 0.4.1, codepage-437 0.1.0, convert_case 0.6.0, libsqlite3-sys 0.30.1, lru 0.18.5, quick-xml 0.41.0, rusqlite 0.32.1, simd-adler32 0.3.10, tracing 0.1.44, tracing-core 0.1.36, ulid 1.2.1, wayland-backend 0.3.17, wayland-client 0.31.15, wayland-protocols 0.32.13, wayland-protocols-wlr 0.3.12, wayland-scanner 0.31.11, wayland-sys 0.31.11, which 8.0.6, winnow 0.7.15, zip 2.4.2, zmij 1.0.23 |
| Unlicense OR MIT | 6 | aho-corasick 1.1.5, byteorder-lite 0.1.0, csv 1.4.0, csv-core 0.1.13, memchr 2.8.3, termcolor 1.4.1 |
| Apache-2.0 | 4 | codespan-reporting 0.11.1, codespan-reporting 0.13.1, rxing 0.9.3, rxing-one-d-proc-derive 0.9.1 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 2 | linux-raw-sys 0.12.1, rustix 1.1.5 |
| BSD-3-Clause OR Apache-2.0 | 2 | moxcms 0.8.1, pxfm 0.1.30 |
| MIT OR Apache-2.0 OR Zlib | 2 | zune-core 0.4.12, zune-jpeg 0.4.21 |
| MIT OR Zlib OR Apache-2.0 | 2 | miniz_oxide 0.8.9, miniz_oxide 0.9.1 |
| (Apache-2.0 OR MIT) AND BSD-3-Clause | 1 | encoding_rs 0.8.42 |
| (MIT OR Apache-2.0) AND IJG | 1 | jpeg-encoder 0.7.1 |
| (MIT OR Apache-2.0) AND ISC | 1 | rqrr 0.11.0 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 | unicode-ident 1.0.26 |
| 0BSD OR MIT OR Apache-2.0 | 1 | adler2 2.0.1 |
| Apache-2.0 OR BSL-1.0 | 1 | ryu 1.0.23 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 1 | zerocopy 0.8.60 |
| LGPL-3.0-only | 1 | wayland-protocols-hyprland 1.2.0 |
| Zlib | 1 | foldhash 0.2.0 |
| Zlib OR Apache-2.0 OR MIT | 1 | bytemuck 1.25.2 |

## Built with, not shipped

| What | Licence |
|---|---|
| cxx-qt-cmake (CMake for the Rust–Qt bridge) | MIT OR Apache-2.0 |
| Corrosion (CMake for Cargo) | MIT |
| Qt Shader Tools | GPL-3.0-only OR LGPL-3.0-only |

## Libraries Snip links (the system's)

| Library | Licence |
|---|---|
| Qt 6 (Core, Gui, Qml, Quick, DBus, Multimedia) | LGPL-3.0-only OR GPL-3.0-only |
| KDE Frameworks 6 CoreAddons | LGPL-2.0-only, LGPL-3.0-only |
| LayerShellQt | LGPL-2.0-or-later, GPL-2.0-or-later |
| libwayland-client | MIT |
| SQLite | Public domain |

## Programs Snip runs (the system's)

| Program | For | Licence |
|---|---|---|
| gpu-screen-recorder | recording | GPL-3.0-only |
| FFmpeg (`ffmpeg`, `ffprobe`) | exports, GIFs, thumbnails, the camera | GPL-3.0-only (Arch's build) |
| Tesseract | Capture Text (OCR) | Apache-2.0 |
| PipeWire (`pw-record`, `pw-play`) | the microphone's level, sounds | MIT, LGPL-2.1-or-later |
| `pactl` (libpulse) | sound devices | LGPL-2.1-or-later |
| Hyprland (`hyprctl`, its socket) | windows, binds, the pointer | BSD-3-Clause |
| Omarchy's tools (`omarchy-theme-color`, `omarchy-shell`) | the theme, the shell | MIT |

## Read, not shipped

- **Cursor themes** (the redrawn pointer, D59): the user's own. Adwaita's cursors are CC-BY-SA-3.0 OR LGPL-3.0-only.
- **Wallpapers** in the Background tool: the user's own images.
