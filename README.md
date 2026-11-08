# Implode — Rust + WebAssembly

A browser port of [Sugar Labs Implode](https://github.com/sugarlabs/implode-activity), the falling-block logic game.

Remove connected groups of three or more blocks of the same color. Blocks fall after every move and empty columns slide left. The Rust engine generates solvable puzzles and implements grouping, removal, undo/redo, replay, difficulty, and win/stuck detection.

## Run

Build `game.wasm` with `./build.sh`, then serve the generated files over HTTP:

```sh
python3 -m http.server 8000
```

Then open <http://localhost:8000>.

## Build

Install the Rust WebAssembly target, then run:

```sh
./build.sh
```

`dist/` is the complete deployment package and contains only:

- `index.html`
- `style.css`
- `app.js`
- `game.wasm`

No framework, package manager, server runtime, or wasm-bindgen glue is required.

## Controls

- Mouse/touch: point at a block and select it
- Keyboard: arrow keys, then Enter or Space
- Undo/redo: toolbar or Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z

## License

GPL-2.0-or-later, consistent with the original activity. See `COPYING`.
