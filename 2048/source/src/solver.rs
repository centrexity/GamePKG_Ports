// Adapted from the pinned 2048-rs solver/evaluators. See CREDITS.md and MIT notice.
use super::model;
use std::collections::BTreeMap;
struct Answer {
    valid: bool,
    direction: u32,
    value: f32,
}
struct Frame {
    board: [u8; 16],
    depth: u8,
    probability: f32,
    average: bool,
    initialized: bool,
    waiting: bool,
    next: u8,
    count: u8,
    value: f32,
    pair: f32,
    valid: bool,
    direction: u32,
}
impl Frame {
    fn new(board: [u8; 16], depth: u8, probability: f32, average: bool) -> Frame {
        Frame {
            board,
            depth,
            probability,
            average,
            initialized: false,
            waiting: false,
            next: 0,
            count: 0,
            value: 0.0,
            pair: 0.0,
            valid: false,
            direction: 0,
        }
    }
}
pub struct Search {
    pub board: [u8; 16],
    frames: Vec<Frame>,
    returned: Option<Answer>,
    cache: BTreeMap<[u8; 16], (f32, f32)>,
}
pub fn search_depth(board: &[u8; 16]) -> u8 {
    let mut seen = [false; 64];
    let mut maximum = 0;
    let mut distinct = 0u8;
    for value in board {
        if *value > 0 {
            if !seen[*value as usize] {
                seen[*value as usize] = true;
                distinct += 1;
            }
            maximum = maximum.max(*value);
        }
    }
    let adjustment = match maximum {
        11 => 4,
        12 | 13 => 2,
        14 => 1,
        15..=63 => 0,
        _ => 7,
    };
    3u8.max(distinct.saturating_sub(adjustment))
}
pub fn row_value(row: &[u8; 4]) -> f32 {
    let mut forward = 0u32;
    let mut backward = 0u32;
    let mut empty = 0;
    let mut aligned = 0;
    for index in 0..4 {
        if row[index] == 0 {
            empty += 1;
        }
        if index > 0 {
            let previous = u32::from(row[index - 1]).pow(4);
            let current = u32::from(row[index]).pow(4);
            if previous > current {
                forward += previous - current;
            } else {
                backward += current - previous;
            }
            if row[index] != 0 && row[index] == row[index - 1] {
                aligned += 1;
            }
        }
    }
    -(forward.min(backward) as f32) + empty as f32 * 200.0 + aligned as f32 * 500.0
}
pub fn evaluate(board: &[u8; 16]) -> f32 {
    let mut result = 0.0;
    // Match upstream row-then-column summation order for f32 decisions.
    for axis in 0..2 {
        for line in 0..4 {
            let mut row = [0u8; 4];
            for index in 0..4 {
                row[index] = board[if axis == 0 {
                    line * 4 + index
                } else {
                    index * 4 + line
                }];
            }
            result += row_value(&row);
        }
    }
    result
}
impl Search {
    pub fn new(board: [u8; 16]) -> Search {
        let frames = Vec::from([Frame::new(board, search_depth(&board), 1.0, false)]);
        Search {
            board,
            frames,
            returned: None,
            cache: BTreeMap::new(),
        }
    }
    pub fn done(&self) -> bool {
        self.frames.is_empty()
    }
    pub fn chosen_move(&self) -> Option<u32> {
        if !self.done() {
            return None;
        }
        match &self.returned {
            Some(answer) if answer.valid => Some(answer.direction),
            _ => None,
        }
    }
    pub fn evaluation_score(&self) -> Option<f32> {
        if !self.done() {
            return None;
        }
        match &self.returned {
            Some(answer) if answer.valid => Some(answer.value),
            _ => None,
        }
    }
    pub fn advance(&mut self, work: u32) {
        for _ in 0..work {
            let mut frame = match self.frames.pop() {
                Some(frame) => frame,
                None => return,
            };
            if frame.waiting {
                let answer = match self.returned.take() {
                    Some(answer) => answer,
                    None => return,
                };
                if frame.average {
                    let value = if answer.valid {
                        answer.value
                    } else {
                        -200000.0
                    };
                    if (frame.next - 1) % 2 == 0 {
                        frame.pair = value;
                    } else {
                        frame.value += frame.pair * 0.9 + value * 0.1;
                    }
                } else if !frame.valid || answer.value >= frame.value {
                    frame.value = answer.value;
                    frame.direction = u32::from(frame.next - 1);
                    frame.valid = true;
                }
                frame.waiting = false;
            }
            if frame.average {
                if !frame.initialized {
                    if frame.depth == 0 || frame.probability < 0.001 {
                        self.returned = Some(Answer {
                            valid: true,
                            direction: 0,
                            value: evaluate(&frame.board),
                        });
                        continue;
                    }
                    if let Some((value, probability)) = self.cache.get(&frame.board) {
                        if *probability >= frame.probability {
                            self.returned = Some(Answer {
                                valid: true,
                                direction: 0,
                                value: *value,
                            });
                            continue;
                        }
                    }
                    for value in &frame.board {
                        if *value == 0 {
                            frame.count += 1;
                        }
                    }
                    if frame.count == 0 {
                        self.returned = Some(Answer {
                            valid: true,
                            direction: 0,
                            value: evaluate(&frame.board),
                        });
                        continue;
                    }
                    frame.initialized = true;
                }
                while frame.next < 32 && frame.board[(frame.next / 2) as usize] != 0 {
                    frame.next += 2;
                }
                if frame.next == 32 {
                    let value = frame.value / f32::from(frame.count);
                    // Bound memoization only. Uncached branches still run the
                    // original search; no fallback depth or node cutoff exists.
                    if self.cache.len() < 32768 || self.cache.contains_key(&frame.board) {
                        self.cache.insert(frame.board, (value, frame.probability));
                    }
                    self.returned = Some(Answer {
                        valid: true,
                        direction: 0,
                        value,
                    });
                    continue;
                }
                let mut board = frame.board;
                board[(frame.next / 2) as usize] = if frame.next % 2 == 0 { 1 } else { 2 };
                let probability = frame.probability * if frame.next % 2 == 0 { 0.9 } else { 0.1 }
                    / f32::from(frame.count);
                let child = Frame::new(board, frame.depth - 1, probability, false);
                frame.next += 1;
                frame.waiting = true;
                self.frames.push(frame);
                self.frames.push(child);
            } else {
                let mut next = None;
                while frame.next < 4 {
                    let direction = u32::from(frame.next);
                    frame.next += 1;
                    let board = model::slide(&frame.board, direction).0;
                    if board != frame.board {
                        next = Some(Frame::new(board, frame.depth, frame.probability, true));
                        break;
                    }
                }
                if let Some(child) = next {
                    frame.waiting = true;
                    self.frames.push(frame);
                    self.frames.push(child);
                } else {
                    self.returned = Some(Answer {
                        valid: frame.valid,
                        direction: frame.direction,
                        value: frame.value,
                    });
                }
            }
        }
    }
}
