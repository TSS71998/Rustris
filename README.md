# Rustris

Rustris is Tetris written in Rust, drawn entirely in ASCII, with an AI that
learned to play it. You can play solo, watch the AI play, or fight it in a
versus mode where clearing lines sends garbage to the other board, like
Tetris 99. It runs as a desktop app and in the browser through WebAssembly.

- **The game:** standard modern Tetris rules: 7-bag randomizer, hold, ghost
  piece, wall kicks, lock delay, levels that speed up gravity.
- **The look:** every frame is turned into characters by a shader. Bright
  areas become dense characters like `@` and `M`, dark areas sparse ones
  like `.` and `:`. Line clears get character-particle effects: shatter,
  dissolve, sweeps, digital rain, fireworks and shockwaves.
- **The AI:** a small, fast evaluator whose weights were learned with the
  cross-entropy method. It plans one piece ahead, uses hold, and goes for
  Tetrises. Details are in [How the AI works](#how-the-ai-works).
- **Versus:** you against the AI at four difficulty levels, with attacks,
  garbage, combos and back-to-back bonuses.

The project is a Rust rewrite of [Pytris](https://github.com/TSS71998/Pytris).
Everything, including the AI and its training, is plain Rust: the only
dependency is [macroquad](https://macroquad.rs) for the window and graphics.

## Quick start

```sh
cargo run --release
```

The menu lets you pick **Solo** or **VS AI**. Left/Right picks the AI's
difficulty and Enter starts.

On Linux, macroquad needs the X11 and OpenGL dev libraries, for example
`sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev`.

### In the browser

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/rustris.wasm web/
cd web && python3 -m http.server 8080   # then open http://localhost:8080
```

Any static file server works. Opening `index.html` straight from disk does
not, because browsers block loading `.wasm` from `file://`. The GitHub
Actions workflow in `.github/workflows/pages.yml` builds and publishes this to
GitHub Pages on every push to `main`.

### Tests

```sh
cargo test
```

## Controls

| Key | Action |
| --- | --- |
| Left / Right | move (hold to auto-repeat) |
| Down | soft drop |
| Space | hard drop |
| Up / X | rotate clockwise |
| Z | rotate counter-clockwise |
| C / Shift | hold |
| P | pause |
| R | restart (rematch in versus) |
| V | cycle view: ASCII + effects / plain ASCII / raw scene |
| 1-4 | preview the 1-4 line-clear effects (solo) |
| A | let the AI play (solo) |
| S | AI speed: normal / fast / max (solo) |
| Esc | back to the menu; quits from the menu (desktop) |

## Versus mode

Both players get the same pieces in the same order. Clearing lines attacks:

| Clear | Lines sent |
| --- | --- |
| Single | 0 |
| Double | 1 |
| Triple | 2 |
| Tetris | 4 (5 if the previous clear was also a Tetris) |
| Combo | +1 to +5 for clearing on consecutive pieces |
| Perfect clear | 10 |

An attack first cancels garbage that is on its way to you; the rest flies to
the opponent. Incoming garbage shows in the meter beside each board (orange
while it travels, red once it is ready). After 0.6 seconds it rises from the
bottom, with one gap per batch, the next time that player places a piece
without clearing a line, at most 8 rows per piece. Top out and you lose.

## How the project fits together

The code is split into a **library** (`src/lib.rs` and the modules it lists)
that holds the rules, the AI and the effects as plain data, and a **front-end**
(`src/main.rs`, `draw.rs`, `ascii.rs`) that handles the window, input and
drawing. The library has no graphics or timing of its own, so it is fully
testable, and the AI can train headless at thousands of games per minute.

| File | What it does |
| --- | --- |
| `src/game.rs` | The rules: board, pieces, rotation with wall kicks, 7-bag, line clears, scoring, hold, garbage rows. No graphics or timing. |
| `src/player.rs` | Real-time play on top of the rules: gravity, 0.5 s lock delay with move resets, DAS/ARR key repeat. |
| `src/rng.rs` | Small seeded random generator, so a seed gives the same pieces on desktop and web. |
| `src/ai.rs` | The AI: move search, board features, weights, difficulty levels, and the pilot that plays in real time. |
| `src/versus.rs` | Versus rules: attacks, cancelling, garbage delay and cap, knockouts; plus an AI-vs-AI simulator. |
| `src/effects.rs` | Line-clear, hard-drop, garbage and game-over effects as plain data. |
| `src/bigfont.rs` | 3x5 pixel font for banners such as `TETRIS`. |
| `src/ascii.rs` | The ASCII renderer: scene render target, glyph layer and the shader setup. |
| `src/shaders/` | The GLSL vertex and fragment shaders, included at compile time. |
| `src/draw.rs` | Drawing shared by the screens: boards, previews, effects, banners, frames. |
| `src/main.rs` | The window: menu, solo and versus screens, input. |
| `assets/ai_weights.txt` | The trained AI weights, built into the game. |
| `assets/glyphs.png` | Character atlas (printable ASCII, 12 x 20 px each), rendered from DejaVu Sans Mono. |
| `examples/train.rs` | Trains the AI's weights. |
| `examples/bench.rs` | Measures the AI: score, lines, Tetrises, survival, speed. |
| `examples/dual.rs` | Plays every difficulty level against every other. |
| `examples/make_atlas.rs` | Regenerates the atlas from any monospace font. |
| `web/index.html` | Page that loads the WebAssembly build. |

## How the AI works

The AI does not see pixels or learn a neural network. It is a **one-move
evaluator**: for the piece in play it tries every place the piece could
land, scores each resulting board with a weighted sum of 11 measurements,
and picks the best. All of its "knowledge" is the 11 weights, which were
learned by having thousands of candidate AIs play and keeping the best. This
is the approach that works best for Tetris in practice: it is tiny, fast,
and very strong.

The code is in `src/ai.rs`. The sections below follow the order in which a
move is made.

### 1. Board representation

The engine stores the board as a 10 x 20 grid of piece ids, which is easy to
draw but slow to search. Each time the AI moves, it converts the board into
**20 bitmasks**, one `u16` per row, with bit `c` set if column `c` is
filled. A full row is `0b11_1111_1111`, clearing it is dropping one array
entry, and counting or comparing cells becomes a handful of bit operations.

### 2. Generating moves

A move is a rotation plus a column. For each piece the AI precomputes its
**distinct rotations** once (an O has 1, I/S/Z have 2, the rest 4) and, for
each, the lowest occupied cell in every column it covers.

To place a piece, it finds the highest filled cell in each column of the
board and drops the piece straight down until one of its columns would hit
the stack. That gives every hard-drop landing spot directly, without
simulating the fall. Placements that would stick out of the top are skipped,
since they end the game. A typical piece has 9 to 34 placements.

Moves are straight drops, the same as `Game::place(rot, x)` in the engine,
and a test checks that every simulated placement matches what the engine
does. The AI therefore never slides pieces under overhangs or performs
T-spins.

### 3. Scoring a board

After simulating a placement (including any line clears), the AI measures
the result. Eight measurements come from Pytris's agent, which in turn
follows classic Tetris AI research (Dellacherie, El-Tetris); three were added
to make Rustris play for score:

| Feature | What it measures | Trained weight |
| --- | --- | --- |
| `landing_height` | how high the piece came to rest | -8.37 |
| `eroded_cells` | lines cleared x cells of the piece that were cleared | -3.24 |
| `row_transitions` | filled/empty switches along each row (walls count as filled) | +0.26 |
| `col_transitions` | filled/empty switches down each column (the floor counts as filled) | -29.73 |
| `holes` | empty cells with something above them | -25.92 |
| `wells` | depth of one-wide wells, counted as 1 + 2 + ... + depth | -0.17 |
| `rows_with_holes` | rows containing at least one hole | -11.75 |
| `bumpiness` | sum of height differences between neighbouring columns | -2.74 |
| `tetrises` | 1 if this move cleared four lines | +4.39 |
| `small_clears` | lines cleared by a single, double or triple | -24.06 |
| `danger` | (tallest column - 10)², once the stack is past half way | -7.95 |

The score of a board is the sum of each feature times its weight; higher is
better. A negative weight means "avoid this", so the AI hates holes and
column transitions (which are mostly holes and overhangs), avoids building
high, and keeps the surface flat.

The learned weights tell the AI's strategy. `small_clears` is strongly
negative and `tetrises` positive, so it refuses to waste lines on singles and
doubles. Instead it stacks cleanly and keeps one column open for an I piece.
`wells` is almost zero, so that open column is not punished. `danger` kicks
in when the stack passes half height, which is when it will accept smaller
clears to stay alive.

### 4. Search: look-ahead and hold

With **look-ahead** on, each placement of the current piece is judged by the
best placement of the *next* piece on the board it leaves behind. A move
that sets up a good follow-up beats one that only looks good on its own. To
avoid counting the same board twice, the features are split in two:

- **move features** (`landing_height`, `eroded_cells`, `tetrises`,
  `small_clears`) describe each move, and are added up for both moves;
- **board features** (the other seven) describe a board, and are only
  measured on the final one.

If the next piece has nowhere to go, the move scores -1,000,000,000, so the
AI never walks into a position where it dies on the next piece.

With **hold** on, the AI also scores swapping: playing the held piece (or the
next piece, if hold is empty) instead of the current one, looking ahead to
whatever would come after. It picks whichever option scores best.

Look-ahead multiplies the work by about 30, but on bitboards a full move with
look-ahead and hold still takes about **0.16 ms**. Without look-ahead it
takes about 0.01 ms.

### 5. Learning the weights: the cross-entropy method

The weights in `assets/ai_weights.txt` were not tuned by hand. They were
learned with the **noisy cross-entropy method** (Szita & Lőrincz, 2006), the
same method Pytris used. It is a simple evolutionary search:

1. Start with a Gaussian over the 11 weights: mean 0, standard deviation 10
   for each.
2. **Sample** 60 weight vectors from it.
3. **Play**: each candidate plays the same 5 games of 500 pieces (same
   seeds, so the comparison is fair), with look-ahead and hold, exactly as
   it would in the game.
4. **Select** the best 20% by average score: the "elite".
5. **Refit** the Gaussian to the elite: the new mean is their average, the
   new spread their spread, plus a little extra noise that shrinks every
   generation (4.0 down to 0.1) so the search keeps exploring early on and
   settles later.
6. Repeat for 25 generations, writing the mean to `assets/ai_weights.txt`
   after each one.

The difference from Pytris is the goal: Pytris rewarded **lines cleared**,
Rustris rewards **score**. Because a Tetris is worth 800 points per level
and four singles only 400, optimising score is what taught the AI to build
for Tetrises. Lines-optimised weights clear about as many lines but almost
never score a Tetris.

Training runs on all CPU cores and took about 4 minutes on two cores. The
average score of the elite went from about 137,000 in generation 1 to about
396,000 per 500-piece game.

### 6. Results

Measured with `cargo run --release --example bench` on 20 unseen games of 500
pieces:

| Weights | Score | Lines | Lines from Tetrises | Deaths |
| --- | --- | --- | --- | --- |
| Rustris (trained for score) | 392,000 | 194 | 89% | 0 of 20 |
| Pytris (trained for lines) | 242,000 | 198 | 0% | 0 of 20 |
| Rustris without look-ahead | 365,000 | 189 | 84% | 2 of 20 |

In three 10,000-piece games it never died, clearing about 4,000 lines each
with 88% of them from Tetrises.

### 7. Playing in real time: the pilot

The search picks a target, but in the game the AI has to get there the same
way you do. The **pilot** (`Pilot` in `src/ai.rs`) turns a target into
actions (hold, rotate, shift left or right, hard drop) and presses them one
at a time through the same `Player` a human uses, so gravity, lock delay and
the rest apply to it too. That is also what makes a fair versus mode
possible.

- **Pacing:** `step` sets the seconds between actions and `think` the delay
  before acting on a new piece.
- **Fast gravity:** from around level 15 a new piece hits the stack almost as
  soon as it appears. When gravity is faster than the pilot's pace, it
  steers the piece the moment it spawns and only paces the drops, like a
  human at high levels would.
- **Blocked moves:** if a rotate or shift does not move the piece, or it has
  taken more than 16 actions, it hard-drops where it is instead of getting
  stuck.

### 8. Difficulty levels

The versus levels use the same weights. They differ in how fast the pilot
acts, how far the AI looks, and how often it deliberately picks a worse
move: a **blunder** is one of the next five best moves instead of the best.

| Level | Action interval | Reaction time | Look-ahead | Hold | Blunders | Measured pace |
| --- | --- | --- | --- | --- | --- | --- |
| Easy | 0.16 s | 0.75 s | no | no | 15% | 0.7 pieces/s |
| Normal | 0.10 s | 0.38 s | no | yes | 6% | 1.05 pieces/s |
| Hard | 0.065 s | 0.16 s | yes | yes | 2% | 2.0 pieces/s |
| Pain | 0.045 s | 0.06 s | yes | yes | 0% | 3.5 pieces/s |

For comparison, a casual player places about 1 piece a second. In
`cargo run --release --example dual` (6 matches per pairing) each level
beats the one below it: Normal beat Easy 6/6, Hard beat Normal 6/6 and Pain
beat Hard 5/6. Pain knocks out Easy in about 15 seconds; evenly matched
games last 40 to 65 seconds. Two Pain AIs always draw, because with the same
pieces and no mistakes they play identically and their attacks cancel out.

### Training and measuring it yourself

```sh
# train from scratch (rewrites assets/ai_weights.txt; rebuild the game after)
cargo run --release --example train -- --gens 25 --pop 60 --games 5 --pieces 500

# score, lines, Tetrises, survival and speed over seeded games
cargo run --release --example bench -- 20 500
cargo run --release --example bench -- 20 500 --no-lookahead
cargo run --release --example bench -- 20 500 --weights some_other_weights.txt

# every difficulty level against every other
cargo run --release --example dual -- 10
```

`train` also takes `--seed N`, `--fast` (no look-ahead, much faster) and
`--resume` (start from the current weights instead of from scratch).

### Limitations and ideas

- **No tucks or spins:** moves are straight drops, so the AI never slides a
  piece under an overhang and cannot do T-spins (and versus mode does not
  reward them).
- **No garbage awareness:** in versus mode the AI ignores incoming garbage
  when choosing moves. Adding the pending garbage as a feature, or training
  directly on versus win rate, would make it stronger.
- **One piece of look-ahead:** the game shows five upcoming pieces; searching
  deeper (with pruning) would help survival in very hard positions.