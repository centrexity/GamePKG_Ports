// Game rules and portable saves. Upstream MIT notice: assets/LICENSE-2048.txt.
use super::gamepkg;
pub struct Position {
    pub cells: [u8; 16],
    pub score: u64,
    pub continued: bool,
}
impl Position {
    pub fn copy(&self) -> Position {
        Position {
            cells: self.cells,
            score: self.score,
            continued: self.continued,
        }
    }
}
pub struct Game {
    pub position: Position,
    pub best: u64,
    pub history: Vec<Position>,
    pub revision: u32,
    pub dirty: bool,
}
fn below(bound: u64) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = gamepkg::random_u64();
        if value >= threshold {
            return value % bound;
        }
    }
}
pub fn spawn(cells: &mut [u8; 16]) {
    let mut empty = 0u64;
    for value in cells.iter() {
        if *value == 0 {
            empty += 1;
        }
    }
    if empty == 0 {
        return;
    }
    let exponent = if below(10) == 0 { 2 } else { 1 };
    let mut rank = below(empty);
    for value in cells.iter_mut() {
        if *value == 0 {
            if rank == 0 {
                *value = exponent;
                return;
            }
            rank -= 1;
        }
    }
}
pub fn slide(cells: &[u8; 16], direction: u32) -> ([u8; 16], u64) {
    let mut output = [0u8; 16];
    let mut score = 0u64;
    for line in 0..4 {
        let mut values = [0u8; 4];
        let mut count = 0;
        for offset in 0..4 {
            let position = if direction == 1 || direction == 3 {
                3 - offset
            } else {
                offset
            };
            let index = if direction < 2 {
                line * 4 + position
            } else {
                position * 4 + line
            };
            if cells[index] != 0 {
                values[count] = cells[index];
                count += 1;
            }
        }
        let mut read = 0;
        let mut write = 0;
        while read < count {
            let mut value = values[read];
            if read + 1 < count && values[read + 1] == value && value < 63 {
                value += 1;
                score = score.saturating_add(1u64 << value);
                read += 1;
            }
            let position = if direction == 1 || direction == 3 {
                3 - write
            } else {
                write
            };
            let index = if direction < 2 {
                line * 4 + position
            } else {
                position * 4 + line
            };
            output[index] = value;
            write += 1;
            read += 1;
        }
    }
    (output, score)
}
impl Game {
    pub fn new(best: u64) -> Game {
        let mut cells = [0u8; 16];
        cells[1] = 1;
        spawn(&mut cells);
        Game {
            position: Position {
                cells,
                score: 0,
                continued: false,
            },
            best,
            history: Vec::new(),
            revision: 1,
            dirty: true,
        }
    }
    pub fn won(&self) -> bool {
        reached(&self.position.cells)
    }
    pub fn over(&self) -> bool {
        for direction in 0..4 {
            if slide(&self.position.cells, direction).0 != self.position.cells {
                return false;
            }
        }
        true
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
        self.dirty = true;
    }
    pub fn update(&mut self, input: u32) {
        if input & 64 != 0 {
            *self = Game::new(self.best);
            return;
        }
        if input & 16 != 0 {
            if let Some(previous) = self.history.pop() {
                self.position = previous;
                self.changed();
            }
            return;
        }
        if self.won() && !self.position.continued {
            if input & 65536 != 0 {
                self.position.continued = true;
                self.changed();
            }
            return;
        }
        let direction = if input & 1 != 0 {
            0
        } else if input & 2 != 0 {
            1
        } else if input & 4 != 0 {
            2
        } else if input & 8 != 0 {
            3
        } else {
            return;
        };
        let (cells, gained) = slide(&self.position.cells, direction);
        if cells == self.position.cells {
            return;
        }
        if self.history.len() == 128 {
            self.history.remove(0);
        }
        self.history.push(self.position.copy());
        self.position.cells = cells;
        self.position.score = self.position.score.saturating_add(gained) & !3;
        self.best = self.best.max(self.position.score);
        spawn(&mut self.position.cells);
        self.changed();
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = b"GP48\x01\r\n\x1a".to_vec();
        bytes.extend(u64::to_le_bytes(self.best));
        bytes.extend(u32::to_le_bytes(self.history.len() as u32));
        bytes.extend(u32::to_le_bytes(self.revision));
        write_position(&self.position, &mut bytes);
        for position in &self.history {
            write_position(position, &mut bytes);
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Option<Game> {
        if bytes.len() < 52 || bytes[..8] != *b"GP48\x01\r\n\x1a" {
            return None;
        }
        let best = read64(bytes, 8);
        let count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        let revision = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if count > 128 || bytes.len() != 52 + count * 28 || revision == 0 {
            return None;
        }
        let position = read_position(bytes, 24, best)?;
        let mut history = Vec::new();
        for index in 0..count {
            history.push(read_position(bytes, 52 + index * 28, best)?);
        }
        Some(Game {
            position,
            best,
            history,
            revision,
            dirty: false,
        })
    }
}
fn write_position(position: &Position, bytes: &mut Vec<u8>) {
    bytes.extend(position.cells);
    bytes.extend(u64::to_le_bytes(position.score));
    bytes.extend(u32::to_le_bytes(u32::from(position.continued)));
}
fn read64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
fn read_position(bytes: &[u8], offset: usize, best: u64) -> Option<Position> {
    let mut cells = [0u8; 16];
    cells.copy_from_slice(&bytes[offset..offset + 16]);
    let score = read64(bytes, offset + 16);
    let flags = u32::from_le_bytes([
        bytes[offset + 24],
        bytes[offset + 25],
        bytes[offset + 26],
        bytes[offset + 27],
    ]);
    let mut occupied = false;
    for value in &cells {
        if *value > 63 {
            return None;
        }
        if *value != 0 {
            occupied = true;
        }
    }
    if !occupied || score > best || !score.is_multiple_of(4) || flags > 1 || (flags == 1 && !reached(&cells))
    {
        return None;
    }
    Some(Position {
        cells,
        score,
        continued: flags == 1,
    })
}

fn reached(cells: &[u8; 16]) -> bool {
    for value in cells {
        if *value >= 11 {
            return true;
        }
    }
    false
}
