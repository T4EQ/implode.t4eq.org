//! Implode game engine compiled to dependency-free WebAssembly.
//! The browser UI talks to this module through a tiny C-style ABI.

use std::sync::Mutex;

const EMPTY: i8 = -1;
const NEW: i8 = -2;
const COLORS: i8 = 5;

type Coord = (usize, usize);

#[derive(Clone, Default)]
struct Board {
    // Columns are stored bottom-to-top, matching the original activity.
    cols: Vec<Vec<i8>>,
}

impl Board {
    fn width(&self) -> usize {
        self.cols.len()
    }
    fn height(&self) -> usize {
        self.cols.iter().map(Vec::len).max().unwrap_or(0)
    }
    fn get(&self, x: isize, y: isize) -> i8 {
        if x < 0 || y < 0 {
            return EMPTY;
        }
        self.cols
            .get(x as usize)
            .and_then(|c| c.get(y as usize))
            .copied()
            .unwrap_or(EMPTY)
    }
    fn set(&mut self, x: usize, y: usize, value: i8) {
        while self.cols.len() <= x {
            self.cols.push(Vec::new());
        }
        let col = &mut self.cols[x];
        while col.len() <= y {
            col.push(EMPTY);
        }
        col[y] = value;
        while col.last() == Some(&EMPTY) {
            col.pop();
        }
    }
    fn group(&self, x: usize, y: usize) -> Vec<Coord> {
        let value = self.get(x as isize, y as isize);
        if value < 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut stack = vec![(x, y)];
        let mut seen = vec![false; self.width().saturating_mul(self.height().max(1))];
        while let Some((cx, cy)) = stack.pop() {
            if cx >= self.width() || cy >= self.height() {
                continue;
            }
            let idx = cy * self.width() + cx;
            if seen[idx] {
                continue;
            }
            seen[idx] = true;
            if self.get(cx as isize, cy as isize) != value {
                continue;
            }
            result.push((cx, cy));
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            stack.push((cx + 1, cy));
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            stack.push((cx, cy + 1));
        }
        result
    }
    fn has_moves(&self) -> bool {
        for x in 0..self.width() {
            for y in 0..self.cols[x].len() {
                if self.group(x, y).len() >= 3 {
                    return true;
                }
            }
        }
        false
    }
    fn remove_group(&mut self, group: &[Coord]) {
        for &(x, y) in group {
            self.cols[x][y] = EMPTY;
        }
        for col in &mut self.cols {
            col.retain(|v| *v != EMPTY);
        }
        self.cols.retain(|col| !col.is_empty());
    }
    fn new_coords(&self) -> Vec<Coord> {
        let mut out = Vec::new();
        for (x, col) in self.cols.iter().enumerate() {
            for (y, value) in col.iter().enumerate() {
                if *value == NEW {
                    out.push((x, y));
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy)]
enum Change {
    Column(usize, usize),
    Cell(usize, usize),
}

struct Rng(u64);
impl Rng {
    fn new(seed: u32) -> Self {
        Self((seed as u64).wrapping_add(0x9E37_79B9_7F4A_7C15))
    }
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
    fn range(&mut self, end: usize) -> usize {
        if end == 0 {
            0
        } else {
            self.next() as usize % end
        }
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / u32::MAX as f64
    }
}

fn apply_change(board: &mut Board, change: Change) {
    match change {
        Change::Column(index, height) => {
            board.cols.insert(index, vec![NEW; height]);
        }
        Change::Cell(x, y) => {
            let old = board.cols[x].clone();
            let mut data = Vec::with_capacity(old.len() + 1);
            let mut new_indexes = Vec::new();
            for (i, value) in old.into_iter().enumerate() {
                if i == y {
                    data.push(NEW);
                }
                if value == NEW {
                    new_indexes.push(i);
                } else {
                    data.push(value);
                }
            }
            if y == board.cols[x].len() {
                data.push(NEW);
            }
            for index in new_indexes {
                data.insert(index, NEW);
            }
            board.cols[x] = data;
        }
    }
}

fn possible_colors(board: &Board) -> Vec<i8> {
    // The original palette reserves index zero and generates colors 1..=5.
    let mut allowed = [true; COLORS as usize + 1];
    allowed[0] = false;
    for (x, y) in board.new_coords() {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let value = board.get(x as isize + dx, y as isize + dy);
            if value > 0 {
                allowed[value as usize] = false;
            }
        }
    }
    allowed
        .iter()
        .enumerate()
        .filter_map(|(i, ok)| ok.then_some(i as i8))
        .collect()
}

fn colorable(board: &Board, change: Change) -> bool {
    let mut test = board.clone();
    apply_change(&mut test, change);
    !possible_colors(&test).is_empty()
}

fn starting_changes(board: &Board, max_w: usize, max_h: usize) -> Vec<Change> {
    let mut changes = Vec::new();
    if board.width() < max_w {
        for x in 0..=board.width() {
            changes.push(Change::Column(x, 1));
        }
    }
    for x in 0..board.width() {
        if board.cols[x].len() < max_h {
            for y in 0..=board.cols[x].len() {
                changes.push(Change::Cell(x, y));
            }
        }
    }
    changes
}

fn growth_changes(
    board: &Board,
    max_w: usize,
    max_h: usize,
) -> (Vec<Change>, Vec<Change>, Vec<Change>) {
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    for x in 0..board.width() {
        let h = board.cols[x].len();
        if h >= max_h {
            continue;
        }
        for y in 0..=h {
            if board.get(x as isize, y as isize) == NEW {
                continue;
            }
            if board.get(x as isize - 1, y as isize) == NEW
                || board.get(x as isize + 1, y as isize) == NEW
            {
                horizontal.push(Change::Cell(x, y));
            } else if board.get(x as isize, y as isize - 1) == NEW
                || board.get(x as isize, y as isize + 1) == NEW
            {
                vertical.push(Change::Cell(x, y));
            }
        }
    }
    let mut columns = Vec::new();
    if board.width() < max_w {
        let mut tops = Vec::with_capacity(board.width());
        for col in &board.cols {
            let top = col
                .iter()
                .rposition(|v| *v == NEW)
                .map(|i| i + 1)
                .unwrap_or(0);
            tops.push(top);
        }
        for x in 0..=board.width() {
            let left = if x > 0 { tops[x - 1] } else { 0 };
            let right = if x < tops.len() { tops[x] } else { 0 };
            let h = left.max(right);
            if h > 0 {
                columns.push(Change::Column(x, h));
            }
        }
    }
    (horizontal, vertical, columns)
}

fn take_weighted(
    rng: &mut Rng,
    lists: &mut (Vec<Change>, Vec<Change>, Vec<Change>),
) -> Option<Change> {
    let weights = (lists.0.len() * 10, lists.1.len() * 5, lists.2.len());
    let total = weights.0 + weights.1 + weights.2;
    if total == 0 {
        return None;
    }
    let pick = rng.range(total);
    if pick < weights.0 {
        let i = rng.range(lists.0.len());
        Some(lists.0.swap_remove(i))
    } else if pick < weights.0 + weights.1 {
        let i = rng.range(lists.1.len());
        Some(lists.1.swap_remove(i))
    } else {
        let i = rng.range(lists.2.len());
        Some(lists.2.swap_remove(i))
    }
}

fn add_piece(
    board: &Board,
    rng: &mut Rng,
    wanted: usize,
    max_w: usize,
    max_h: usize,
) -> Option<Board> {
    let mut next = board.clone();
    let mut starts = starting_changes(&next, max_w, max_h);
    let first = loop {
        if starts.is_empty() {
            return None;
        }
        let i = rng.range(starts.len());
        let change = starts.swap_remove(i);
        if colorable(&next, change) {
            break change;
        }
    };
    apply_change(&mut next, first);
    let mut added = 1;
    while added < wanted {
        let mut lists = growth_changes(&next, max_w, max_h);
        let mut accepted = None;
        while let Some(change) = take_weighted(rng, &mut lists) {
            if colorable(&next, change) {
                accepted = Some(change);
                break;
            }
        }
        match accepted {
            Some(change) => {
                added += match change {
                    Change::Cell(_, _) => 1,
                    Change::Column(_, h) => h,
                };
                apply_change(&mut next, change);
            }
            None => break,
        }
    }
    if added < 3 {
        return None;
    }
    let colors = possible_colors(&next);
    let color = colors[rng.range(colors.len())];
    for (x, y) in next.new_coords() {
        next.set(x, y, color);
    }
    Some(next)
}

fn generate(seed: u32, difficulty: u8) -> Board {
    let (max_w, max_h, fragmentation) = match difficulty {
        0 => (8, 6, 0),
        1 => (12, 10, 0),
        _ => (20, 15, 2),
    };
    let mut rng = Rng::new(seed);
    let max_area = (max_w * max_h) as f64 * 0.5;
    let upper = max_area.sqrt().ceil() as usize;
    let mut area = 0usize;
    let mut sizes = Vec::new();
    while (area as f64) < max_area {
        let raw = if fragmentation == 0 {
            upper
        } else {
            (rng.unit().powi(fragmentation) * upper as f64) as usize
        };
        let size = raw.max(3);
        area += size;
        sizes.push(size);
    }
    let mut board = Board::default();
    for size in sizes {
        if let Some(next) = add_piece(&board, &mut rng, size, max_w, max_h) {
            board = next;
        }
    }
    board
}

struct Game {
    board: Board,
    undo: Vec<Board>,
    redo: Vec<Board>,
    seed: u32,
    difficulty: u8,
    moves: u32,
}

impl Game {
    fn new(seed: u32, difficulty: u8) -> Self {
        Self {
            board: generate(seed, difficulty),
            undo: Vec::new(),
            redo: Vec::new(),
            seed,
            difficulty,
            moves: 0,
        }
    }
    fn status(&self) -> i32 {
        if self.board.width() == 0 {
            1
        } else if !self.board.has_moves() {
            2
        } else {
            0
        }
    }
}

static GAME: Mutex<Option<Game>> = Mutex::new(None);

fn with_game<T>(default: T, f: impl FnOnce(&Game) -> T) -> T {
    let game = GAME.lock().unwrap();
    game.as_ref().map(f).unwrap_or(default)
}

#[no_mangle]
pub extern "C" fn game_init(seed: u32, difficulty: u32) {
    *GAME.lock().unwrap() = Some(Game::new(seed, difficulty.min(2) as u8));
}
#[no_mangle]
pub extern "C" fn game_width() -> i32 {
    with_game(0, |g| g.board.width() as i32)
}
#[no_mangle]
pub extern "C" fn game_height() -> i32 {
    with_game(0, |g| g.board.height() as i32)
}
#[no_mangle]
pub extern "C" fn game_cell(x: i32, y: i32) -> i32 {
    with_game(-1, |g| g.board.get(x as isize, y as isize) as i32)
}
#[no_mangle]
pub extern "C" fn game_group_size(x: i32, y: i32) -> i32 {
    if x < 0 || y < 0 {
        return 0;
    }
    with_game(0, |g| g.board.group(x as usize, y as usize).len() as i32)
}
#[no_mangle]
pub extern "C" fn game_same_group(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    if ax < 0 || ay < 0 || bx < 0 || by < 0 {
        return 0;
    }
    with_game(0, |g| {
        g.board
            .group(ax as usize, ay as usize)
            .contains(&(bx as usize, by as usize)) as i32
    })
}
#[no_mangle]
pub extern "C" fn game_remove(x: i32, y: i32) -> i32 {
    if x < 0 || y < 0 {
        return 0;
    }
    let mut lock = GAME.lock().unwrap();
    let Some(game) = lock.as_mut() else {
        return 0;
    };
    let group = game.board.group(x as usize, y as usize);
    if group.len() < 3 {
        return 0;
    }
    game.undo.push(game.board.clone());
    game.redo.clear();
    game.board.remove_group(&group);
    game.moves += 1;
    1
}
#[no_mangle]
pub extern "C" fn game_undo() -> i32 {
    let mut lock = GAME.lock().unwrap();
    let Some(game) = lock.as_mut() else {
        return 0;
    };
    let Some(previous) = game.undo.pop() else {
        return 0;
    };
    let current = std::mem::replace(&mut game.board, previous);
    game.redo.push(current);
    game.moves = game.moves.saturating_sub(1);
    1
}
#[no_mangle]
pub extern "C" fn game_redo() -> i32 {
    let mut lock = GAME.lock().unwrap();
    let Some(game) = lock.as_mut() else {
        return 0;
    };
    let Some(next) = game.redo.pop() else {
        return 0;
    };
    let current = std::mem::replace(&mut game.board, next);
    game.undo.push(current);
    game.moves += 1;
    1
}
#[no_mangle]
pub extern "C" fn game_replay() {
    let mut lock = GAME.lock().unwrap();
    if let Some(game) = lock.as_mut() {
        *game = Game::new(game.seed, game.difficulty);
    }
}
#[no_mangle]
pub extern "C" fn game_status() -> i32 {
    with_game(0, Game::status)
}
#[no_mangle]
pub extern "C" fn game_moves() -> i32 {
    with_game(0, |g| g.moves as i32)
}
#[no_mangle]
pub extern "C" fn game_seed() -> u32 {
    with_game(0, |g| g.seed)
}
#[no_mangle]
pub extern "C" fn game_difficulty() -> i32 {
    with_game(0, |g| g.difficulty as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_boards_start_playable() {
        for difficulty in 0..3 {
            for seed in 0..25 {
                let board = generate(seed, difficulty);
                assert!(board.width() > 0);
                assert!(board.has_moves());
            }
        }
    }
    #[test]
    fn removing_compacts_board() {
        let mut b = Board {
            cols: vec![vec![1, 1, 1], vec![2], vec![1]],
        };
        let group = b.group(0, 0);
        b.remove_group(&group);
        assert_eq!(b.cols, vec![vec![2], vec![1]]);
    }
}
