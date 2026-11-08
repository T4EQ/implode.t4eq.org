const canvas = document.querySelector('#game');
const ctx = canvas.getContext('2d');
const wrap = document.querySelector('#game-wrap');
const loading = document.querySelector('#loading');
const stuck = document.querySelector('#stuck');
const win = document.querySelector('#win');
const moveCount = document.querySelector('#move-count');
const colors = ['#7f00bf', '#e6000a', '#00ea11', '#fffa00', '#5e008c', '#ff8f00'];

let wasm;
let level = Number(localStorage.getItem('implode-level') || 0);
let selected = null;
let transform = null;
let removing = null;
let animationFrame = 0;

async function loadWasm() {
  const response = await fetch('game.wasm');
  let result;
  if (WebAssembly.instantiateStreaming) {
    try { result = await WebAssembly.instantiateStreaming(response.clone(), {}); }
    catch { result = await WebAssembly.instantiate(await response.arrayBuffer(), {}); }
  } else {
    result = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  }
  wasm = result.instance.exports;
  newGame(randomSeed(), level);
  loading.hidden = true;
}

function randomSeed() {
  const values = new Uint32Array(1);
  crypto.getRandomValues(values);
  return values[0];
}

function newGame(seed, difficulty = level) {
  level = difficulty;
  localStorage.setItem('implode-level', String(level));
  wasm.game_init(seed, level);
  selected = null;
  removing = null;
  updateLevelButtons();
  updateState();
  canvas.focus();
}

function updateLevelButtons() {
  document.querySelectorAll('[data-level]').forEach(button => {
    button.setAttribute('aria-checked', String(Number(button.dataset.level) === level));
  });
}

function updateState() {
  const moves = wasm.game_moves();
  moveCount.textContent = `${moves} move${moves === 1 ? '' : 's'}`;
  const status = wasm.game_status();
  stuck.hidden = status !== 2;
  win.hidden = status !== 1;
  if (status === 1) document.querySelector('#win-moves').textContent = `Solved in ${moves} move${moves === 1 ? '' : 's'}.`;
  resize();
}

function resize() {
  const rect = wrap.getBoundingClientRect();
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const pixelWidth = Math.max(1, Math.round(rect.width * dpr));
  const pixelHeight = Math.max(1, Math.round(rect.height * dpr));
  if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
  }
  canvas.style.width = `${rect.width}px`;
  canvas.style.height = `${rect.height}px`;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  calculateTransform(rect.width, rect.height);
  draw();
}

function calculateTransform(width, height) {
  if (!wasm) return;
  const cols = Math.max(1, wasm.game_width());
  const rows = Math.max(1, wasm.game_height());
  const border = Math.min(width, height) * .05;
  const cell = Math.max(1, Math.min((width - border * 2) / cols, (height - border * 2) / rows));
  const boardWidth = cell * cols;
  const boardHeight = cell * rows;
  transform = { cols, rows, cell, x: (width - boardWidth) / 2, y: (height - boardHeight) / 2 };
}

function cellRect(x, y, scale = 1) {
  const gap = transform.cell * .1;
  const size = (transform.cell - gap) * scale;
  const cx = transform.x + (x + .5) * transform.cell;
  const cy = transform.y + (transform.rows - y - .5) * transform.cell;
  return { x: cx - size / 2, y: cy - size / 2, size, cx, cy };
}

function isSelectedGroup(x, y) {
  return selected && wasm.game_same_group(selected.x, selected.y, x, y) === 1 && wasm.game_group_size(selected.x, selected.y) >= 3;
}

function draw(now = performance.now()) {
  if (!wasm || !transform) return;
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = '#5959ae';
  ctx.fillRect(0, 0, width, height);

  if (selected && wasm.game_group_size(selected.x, selected.y) >= 3) {
    ctx.fillStyle = '#fff';
    for (let x = 0; x < transform.cols; x++) for (let y = 0; y < transform.rows; y++) {
      if (!isSelectedGroup(x, y)) continue;
      const r = cellRect(x, y, 1.12);
      ctx.fillRect(r.x, r.y, r.size, r.size);
    }
  }

  let shrink = 1;
  if (removing) shrink = Math.max(0, 1 - (now - removing.started) / 160);
  for (let x = 0; x < transform.cols; x++) for (let y = 0; y < transform.rows; y++) {
    const value = wasm.game_cell(x, y);
    if (value < 0) continue;
    const partOfRemoval = removing && wasm.game_same_group(removing.x, removing.y, x, y) === 1;
    const r = cellRect(x, y, partOfRemoval ? shrink : 1);
    ctx.fillStyle = colors[value % colors.length];
    ctx.fillRect(r.x, r.y, r.size, r.size);
  }

  if (selected && wasm.game_group_size(selected.x, selected.y) < 3 && wasm.game_cell(selected.x, selected.y) >= 0) {
    const r = cellRect(selected.x, selected.y);
    ctx.beginPath();
    ctx.arc(r.cx, r.cy, Math.max(4, transform.cell * .09), 0, Math.PI * 2);
    ctx.fillStyle = '#fff';
    ctx.fill();
  }

  if (removing) {
    if (shrink > 0) animationFrame = requestAnimationFrame(draw);
    else {
      const { x, y } = removing;
      removing = null;
      wasm.game_remove(x, y);
      selected = null;
      updateState();
    }
  }
}

function pointToCell(clientX, clientY) {
  if (!transform) return null;
  const rect = canvas.getBoundingClientRect();
  const px = clientX - rect.left;
  const py = clientY - rect.top;
  const x = Math.floor((px - transform.x) / transform.cell);
  const fromTop = Math.floor((py - transform.y) / transform.cell);
  const y = transform.rows - 1 - fromTop;
  if (x < 0 || y < 0 || x >= transform.cols || y >= transform.rows) return null;
  return { x, y };
}

function select(cell) {
  if (!cell || removing) return;
  selected = cell;
  draw();
}

function removeSelected() {
  if (!selected || removing || wasm.game_group_size(selected.x, selected.y) < 3) return;
  removing = { ...selected, started: performance.now() };
  cancelAnimationFrame(animationFrame);
  draw(removing.started);
}

canvas.addEventListener('pointermove', event => {
  if (event.pointerType === 'mouse') select(pointToCell(event.clientX, event.clientY));
});
canvas.addEventListener('pointerdown', event => {
  event.preventDefault();
  canvas.focus();
  select(pointToCell(event.clientX, event.clientY));
});
canvas.addEventListener('pointerup', event => {
  event.preventDefault();
  const cell = pointToCell(event.clientX, event.clientY);
  if (cell && selected && cell.x === selected.x && cell.y === selected.y) removeSelected();
});
canvas.addEventListener('keydown', event => {
  if (!wasm || removing) return;
  const keyMoves = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, 1], ArrowDown: [0, -1] };
  if (keyMoves[event.key]) {
    event.preventDefault();
    const [dx, dy] = keyMoves[event.key];
    if (!selected) selected = { x: Math.floor(wasm.game_width() / 2), y: Math.max(0, wasm.game_height() - 1) };
    else selected = {
      x: Math.max(0, Math.min(wasm.game_width() - 1, selected.x + dx)),
      y: Math.max(0, Math.min(wasm.game_height() - 1, selected.y + dy))
    };
    draw();
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault(); removeSelected();
  }
});

document.querySelector('#new').onclick = () => newGame(randomSeed());
document.querySelector('#replay').onclick = () => { wasm.game_replay(); selected = null; updateState(); };
document.querySelector('#undo').onclick = () => { if (wasm.game_undo()) { selected = null; updateState(); } };
document.querySelector('#redo').onclick = () => { if (wasm.game_redo()) { selected = null; updateState(); } };
document.querySelector('#stuck-undo').onclick = () => { if (wasm.game_undo()) { selected = null; updateState(); } };
document.querySelector('#win-new').onclick = () => newGame(randomSeed());
document.querySelectorAll('[data-level]').forEach(button => button.onclick = () => newGame(randomSeed(), Number(button.dataset.level)));

document.addEventListener('keydown', event => {
  if (!(event.ctrlKey || event.metaKey)) return;
  if (event.key.toLowerCase() === 'z') {
    event.preventDefault();
    const changed = event.shiftKey ? wasm.game_redo() : wasm.game_undo();
    if (changed) { selected = null; updateState(); }
  }
});

const help = document.querySelector('#help-dialog');
document.querySelector('#help').onclick = () => help.showModal();
document.querySelectorAll('.close, .close-help').forEach(button => button.onclick = () => help.close());
help.addEventListener('click', event => {
  const rect = help.getBoundingClientRect();
  if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) help.close();
});

new ResizeObserver(resize).observe(wrap);
loadWasm().catch(error => {
  console.error(error);
  loading.textContent = 'Could not load game.wasm. Serve this folder through a local web server.';
});
