# Rustris

Tetris in Rust, and (coming next) an AI that learns to play it. The end goal is to play against
the AI in the browser via WebAssembly.

## Play

```sh
cargo run --release
```

| Key | Action |
| --- | --- |
| Left / Right | move (hold to auto-repeat) |
| Down | soft drop |
| Space | hard drop |
| Up / X | rotate clockwise |
| Z | rotate counter-clockwise |
| C / Shift | hold |
| P | pause |
| R | restart |
| V | cycle view: ASCII + effects / plain ASCII / raw scene |
| 1-4 | preview the 1-4 line-clear effects |
| Esc | quit (desktop only) |

On Linux, macroquad needs the X11 and OpenGL dev libraries, for example
`sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev`.

## Play in the browser

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rustris.wasm web/
cd web && python3 -m http.server 8080   # then open http://localhost:8080
```

Any static file server works; opening `index.html` straight from disk does
not, because browsers block loading `.wasm` from `file://`.

## Tests

```sh
cargo test
```

## Layout

| File | What it does |
| --- | --- |
| `src/game.rs` | The rules: board, pieces, rotation with wall kicks, 7-bag, line clears, scoring, hold. No graphics or timing. |
| `src/player.rs` | Real-time play on top of the rules: gravity, 0.5 s lock delay with move resets, DAS/ARR key repeat. |
| `src/rng.rs` | Small seeded random generator, so a seed gives the same pieces on desktop and web. |
| `src/effects.rs` | Line-clear, hard-drop and game-over effects as plain data (no graphics), so they are testable. |
| `src/bigfont.rs` | 3x5 pixel font for banners such as `TETRIS`. |
| `src/ascii.rs` | The ASCII renderer: scene render target, glyph layer and the shader. |
| `src/main.rs` | The macroquad window: layout, input, drawing. |
| `assets/glyphs.png` | Character atlas (printable ASCII, 12 x 20 px each), rendered from DejaVu Sans Mono. |
| `examples/make_atlas.rs` | Regenerates the atlas from any monospace font: `cargo run --example make_atlas -- path/to/font.ttf`. |
| `web/index.html` | Page that loads the WebAssembly build. |

`Game::place(rot, x)` is already there for the AI: it drops the current piece
in a given rotation and column, the same move format Pytris's agent uses.