//! The game tables in Varrow Centre: chess, checkers (English draughts) and Connect 4, with
//! their full rules and a bot for each at three strengths. Everything here is plain data and
//! rules (no drawing): the hub seats players at a table, the host applies their moves, and the
//! tables' states go out to everyone (see crate::hub).
//!
//! Chess: castling (not out of, through or into check), en passant, promotion to any piece,
//! check, checkmate and stalemate, and the draws: the fifty-move rule, threefold repetition and
//! dead positions (king against king, or with one bishop or knight, or bishops all on one colour).
//! Checkers: men move diagonally forward, kings both ways; taking is forced, a jump carries on
//! while it can, a man reaching the far row is crowned and its move ends there; a side with no
//! move loses; drawn after 40 moves each without a capture or a man moving, or a third repetition.
//! Connect 4: seven columns, six rows, four in a row in any direction; a full board is a draw.

/// How strong a table's bot plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Easy = 0,
    Medium = 1,
    Hard = 2,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Easy, Level::Medium, Level::Hard];
    pub fn from_u8(v: u8) -> Level {
        match v {
            0 => Level::Easy,
            1 => Level::Medium,
            _ => Level::Hard,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "Easy",
            Level::Medium => "Medium",
            Level::Hard => "Hard",
        }
    }
}

/// How a game stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    /// Won by side 0 (white; black in checkers; red in Connect 4) or side 1.
    Win(u8),
    Draw(DrawWhy),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawWhy {
    Stalemate,
    FiftyMoves,
    Repetition,
    DeadPosition,
    FullBoard,
    NoProgress,
    /// A player left (or both agreed).
    Agreed,
}

impl Outcome {
    pub fn text(self, names: [&str; 2]) -> String {
        match self {
            Outcome::Playing => "playing".into(),
            Outcome::Win(s) => format!("{} wins", names[(s as usize).min(1)]),
            Outcome::Draw(w) => format!(
                "draw ({})",
                match w {
                    DrawWhy::Stalemate => "stalemate",
                    DrawWhy::FiftyMoves => "fifty moves without a capture or a pawn move",
                    DrawWhy::Repetition => "the same position three times",
                    DrawWhy::DeadPosition => "neither side can mate",
                    DrawWhy::FullBoard => "the board is full",
                    DrawWhy::NoProgress => "forty moves each without a capture or a man moving",
                    DrawWhy::Agreed => "agreed",
                }
            ),
        }
    }
    pub fn to_u8(self) -> u8 {
        match self {
            Outcome::Playing => 0,
            Outcome::Win(s) => 1 + s.min(1),
            Outcome::Draw(w) => 3 + w as u8,
        }
    }
    pub fn from_u8(v: u8) -> Outcome {
        match v {
            1 => Outcome::Win(0),
            2 => Outcome::Win(1),
            3 => Outcome::Draw(DrawWhy::Stalemate),
            4 => Outcome::Draw(DrawWhy::FiftyMoves),
            5 => Outcome::Draw(DrawWhy::Repetition),
            6 => Outcome::Draw(DrawWhy::DeadPosition),
            7 => Outcome::Draw(DrawWhy::FullBoard),
            8 => Outcome::Draw(DrawWhy::NoProgress),
            9 => Outcome::Draw(DrawWhy::Agreed),
            _ => Outcome::Playing,
        }
    }
}

/// A tiny random generator (the bots' tie-breaks and their easy-level slips).
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        if self.0 == 0 {
            self.0 = 0x9e37_79b9_7f4a_7c15;
        }
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
    pub fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

fn hash_of<T: std::hash::Hash>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

// ===================================================================================== chess

pub const PAWN: i8 = 1;
pub const KNIGHT: i8 = 2;
pub const BISHOP: i8 = 3;
pub const ROOK: i8 = 4;
pub const QUEEN: i8 = 5;
pub const KING: i8 = 6;

/// A chess move: from and to squares (a1 = 0, b1 = 1, ... h8 = 63) and the piece a pawn
/// becomes (0 when it doesn't promote).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CMove {
    pub from: u8,
    pub to: u8,
    pub promo: i8,
}

/// A chess position and its history (for repetitions).
#[derive(Clone, Debug, PartialEq)]
pub struct Chess {
    /// Pieces: positive white, negative black (PAWN..KING), 0 empty; a1 = 0.
    pub sq: [i8; 64],
    /// 0 white to move, 1 black.
    pub turn: u8,
    /// Castling still allowed: white king side, white queen side, black king side, black queen side.
    pub castle: [bool; 4],
    /// The square a pawn just skipped over (it can be taken en passant there).
    pub ep: Option<u8>,
    /// Half-moves since the last capture or pawn move.
    pub halfmove: u16,
    pub fullmove: u16,
    /// Each position so far (after every move), for threefold repetition.
    pub seen: Vec<u64>,
    /// The moves played, in order.
    pub moves: Vec<CMove>,
}

const KNIGHT_D: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING_D: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK_D: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_D: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

fn at(f: i8, r: i8) -> Option<u8> {
    if (0..8).contains(&f) && (0..8).contains(&r) { Some((r * 8 + f) as u8) } else { None }
}

/// A square's name ("e4").
pub fn sq_name(s: u8) -> String {
    format!("{}{}", (b'a' + s % 8) as char, s / 8 + 1)
}

impl Default for Chess {
    fn default() -> Self {
        Chess::new()
    }
}

impl Chess {
    pub fn new() -> Chess {
        Chess::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").expect("start position")
    }

    /// A position from Forsyth-Edwards notation.
    pub fn from_fen(fen: &str) -> Option<Chess> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        let mut sq = [0i8; 64];
        let (mut r, mut f) = (7i8, 0i8);
        for c in parts.first()?.chars() {
            match c {
                '/' => {
                    r -= 1;
                    f = 0;
                }
                '1'..='8' => f += c as i8 - '0' as i8,
                _ => {
                    let p = match c.to_ascii_lowercase() {
                        'p' => PAWN,
                        'n' => KNIGHT,
                        'b' => BISHOP,
                        'r' => ROOK,
                        'q' => QUEEN,
                        'k' => KING,
                        _ => return None,
                    };
                    let s = at(f, r)?;
                    sq[s as usize] = if c.is_ascii_uppercase() { p } else { -p };
                    f += 1;
                }
            }
        }
        let turn = if parts.get(1).copied().unwrap_or("w") == "b" { 1 } else { 0 };
        let cs = parts.get(2).copied().unwrap_or("-");
        let castle = [cs.contains('K'), cs.contains('Q'), cs.contains('k'), cs.contains('q')];
        let ep = parts.get(3).and_then(|e| {
            let b = e.as_bytes();
            if b.len() == 2 { at(b[0] as i8 - b'a' as i8, b[1] as i8 - b'1' as i8) } else { None }
        });
        let halfmove = parts.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
        let fullmove = parts.get(5).and_then(|v| v.parse().ok()).unwrap_or(1);
        let mut c = Chess { sq, turn, castle, ep, halfmove, fullmove, seen: Vec::new(), moves: Vec::new() };
        c.seen.push(c.key());
        Some(c)
    }

    fn me(&self) -> i8 {
        if self.turn == 0 { 1 } else { -1 }
    }

    pub fn king(&self, side: i8) -> Option<u8> {
        (0..64u8).find(|&s| self.sq[s as usize] == KING * side)
    }

    /// Is square `s` attacked by `by` (1 white, -1 black)?
    pub fn attacked(&self, s: u8, by: i8) -> bool {
        let (f, r) = ((s % 8) as i8, (s / 8) as i8);
        // pawns: a white pawn attacks up the board, so look one rank down for it
        for df in [-1, 1] {
            if let Some(p) = at(f + df, r - by) {
                if self.sq[p as usize] == PAWN * by {
                    return true;
                }
            }
        }
        for (df, dr) in KNIGHT_D {
            if let Some(p) = at(f + df, r + dr) {
                if self.sq[p as usize] == KNIGHT * by {
                    return true;
                }
            }
        }
        for (df, dr) in KING_D {
            if let Some(p) = at(f + df, r + dr) {
                if self.sq[p as usize] == KING * by {
                    return true;
                }
            }
        }
        for (dirs, a, b) in [(ROOK_D, ROOK, QUEEN), (BISHOP_D, BISHOP, QUEEN)] {
            for (df, dr) in dirs {
                let (mut x, mut y) = (f + df, r + dr);
                while let Some(p) = at(x, y) {
                    let v = self.sq[p as usize];
                    if v != 0 {
                        if v == a * by || v == b * by {
                            return true;
                        }
                        break;
                    }
                    x += df;
                    y += dr;
                }
            }
        }
        false
    }

    pub fn in_check(&self) -> bool {
        let me = self.me();
        self.king(me).is_some_and(|k| self.attacked(k, -me))
    }

    /// Moves that follow the pieces' moves (they may leave the king in check).
    fn pseudo(&self, out: &mut Vec<CMove>) {
        let me = self.me();
        for s in 0..64u8 {
            let v = self.sq[s as usize];
            if v * me <= 0 {
                continue;
            }
            let (f, r) = ((s % 8) as i8, (s / 8) as i8);
            let p = v.abs();
            let push = |to: u8, out: &mut Vec<CMove>| {
                let last = if me > 0 { 7 } else { 0 };
                if p == PAWN && (to / 8) as i8 == last {
                    for q in [QUEEN, ROOK, BISHOP, KNIGHT] {
                        out.push(CMove { from: s, to, promo: q });
                    }
                } else {
                    out.push(CMove { from: s, to, promo: 0 });
                }
            };
            match p {
                PAWN => {
                    let start = if me > 0 { 1 } else { 6 };
                    if let Some(t) = at(f, r + me) {
                        if self.sq[t as usize] == 0 {
                            push(t, out);
                            if r == start {
                                if let Some(t2) = at(f, r + 2 * me) {
                                    if self.sq[t2 as usize] == 0 {
                                        push(t2, out);
                                    }
                                }
                            }
                        }
                    }
                    for df in [-1, 1] {
                        if let Some(t) = at(f + df, r + me) {
                            if self.sq[t as usize] * me < 0 || self.ep == Some(t) {
                                push(t, out);
                            }
                        }
                    }
                }
                KNIGHT | KING => {
                    for (df, dr) in if p == KNIGHT { KNIGHT_D } else { KING_D } {
                        if let Some(t) = at(f + df, r + dr) {
                            if self.sq[t as usize] * me <= 0 {
                                push(t, out);
                            }
                        }
                    }
                    if p == KING {
                        self.castles(s, out);
                    }
                }
                _ => {
                    let dirs: &[(i8, i8)] = match p {
                        BISHOP => &BISHOP_D,
                        ROOK => &ROOK_D,
                        _ => &KING_D,
                    };
                    for &(df, dr) in dirs {
                        let (mut x, mut y) = (f + df, r + dr);
                        while let Some(t) = at(x, y) {
                            let tv = self.sq[t as usize];
                            if tv * me > 0 {
                                break;
                            }
                            push(t, out);
                            if tv != 0 {
                                break;
                            }
                            x += df;
                            y += dr;
                        }
                    }
                }
            }
        }
    }

    /// Castling: the king and rook unmoved, nothing between them, and the king not in check,
    /// not passing through or landing on an attacked square.
    fn castles(&self, k: u8, out: &mut Vec<CMove>) {
        let me = self.me();
        let (base, ks, qs) = if me > 0 { (0u8, 0, 1) } else { (56u8, 2, 3) };
        if k != base + 4 || self.attacked(k, -me) {
            return;
        }
        if self.castle[ks] && self.sq[(base + 7) as usize] == ROOK * me && self.sq[(base + 5) as usize] == 0 && self.sq[(base + 6) as usize] == 0 && !self.attacked(base + 5, -me) && !self.attacked(base + 6, -me) {
            out.push(CMove { from: k, to: base + 6, promo: 0 });
        }
        if self.castle[qs] && self.sq[base as usize] == ROOK * me && (1..4).all(|d| self.sq[(base + d) as usize] == 0) && !self.attacked(base + 3, -me) && !self.attacked(base + 2, -me) {
            out.push(CMove { from: k, to: base + 2, promo: 0 });
        }
    }

    /// Every legal move.
    pub fn legal(&self) -> Vec<CMove> {
        let mut ps = Vec::with_capacity(48);
        self.pseudo(&mut ps);
        let me = self.me();
        ps.retain(|&m| {
            let mut c = self.clone_light();
            c.play_raw(m);
            c.king(me).is_some_and(|k| !c.attacked(k, -me))
        });
        ps
    }

    /// A copy without the history (for searching).
    fn clone_light(&self) -> Chess {
        Chess { sq: self.sq, turn: self.turn, castle: self.castle, ep: self.ep, halfmove: self.halfmove, fullmove: self.fullmove, seen: Vec::new(), moves: Vec::new() }
    }

    /// Make a move without checking it or keeping history.
    fn play_raw(&mut self, m: CMove) {
        let me = self.me();
        let v = self.sq[m.from as usize];
        let p = v.abs();
        let captured = self.sq[m.to as usize];
        let mut capture = captured != 0;
        // en passant: the pawn taken is behind the square moved to
        if p == PAWN && Some(m.to) == self.ep && captured == 0 && m.from % 8 != m.to % 8 {
            let behind = (m.to as i8 - 8 * me) as usize;
            self.sq[behind] = 0;
            capture = true;
        }
        self.sq[m.to as usize] = if m.promo != 0 { m.promo * me } else { v };
        self.sq[m.from as usize] = 0;
        // castling moves the rook too
        if p == KING && (m.to as i8 - m.from as i8).abs() == 2 {
            let (rf, rt) = if m.to > m.from { (m.from + 3, m.from + 1) } else { (m.from - 4, m.from - 1) };
            self.sq[rt as usize] = self.sq[rf as usize];
            self.sq[rf as usize] = 0;
        }
        // rights lost: the king or a rook moved, or a rook taken in its corner
        for s in [m.from, m.to] {
            match s {
                4 => {
                    self.castle[0] = false;
                    self.castle[1] = false;
                }
                60 => {
                    self.castle[2] = false;
                    self.castle[3] = false;
                }
                7 => self.castle[0] = false,
                0 => self.castle[1] = false,
                63 => self.castle[2] = false,
                56 => self.castle[3] = false,
                _ => {}
            }
        }
        self.ep = if p == PAWN && (m.to as i8 - m.from as i8).abs() == 16 { Some(((m.from as i8 + m.to as i8) / 2) as u8) } else { None };
        self.halfmove = if p == PAWN || capture { 0 } else { self.halfmove + 1 };
        if self.turn == 1 {
            self.fullmove += 1;
        }
        self.turn ^= 1;
    }

    /// The position as it counts for repetitions: pieces, who moves, castling rights, and an en
    /// passant square only when a pawn could really take there.
    fn key(&self) -> u64 {
        let me = self.me();
        let ep = self.ep.filter(|&e| {
            let (f, r) = ((e % 8) as i8, (e / 8) as i8);
            [-1, 1].iter().any(|df| at(f + df, r - me).is_some_and(|p| self.sq[p as usize] == PAWN * me))
        });
        hash_of(&(self.sq, self.turn, self.castle, ep))
    }

    /// Play a legal move (false, and nothing changes, when it isn't one). A promotion given
    /// without a piece becomes a queen.
    pub fn play(&mut self, m: CMove) -> bool {
        if self.outcome() != Outcome::Playing {
            return false;
        }
        let legal = self.legal();
        let m = if m.promo == 0 && legal.iter().any(|l| l.from == m.from && l.to == m.to && l.promo == QUEEN) { CMove { promo: QUEEN, ..m } } else { m };
        if !legal.contains(&m) {
            return false;
        }
        self.play_raw(m);
        self.moves.push(m);
        self.seen.push(self.key());
        true
    }

    /// Dead position: no sequence of legal moves could mate (king against king, king and one
    /// minor piece against king, or kings and bishops all on squares of one colour).
    fn dead(&self) -> bool {
        let mut minors = Vec::new();
        for s in 0..64u8 {
            match self.sq[s as usize].abs() {
                0 | KING => {}
                KNIGHT | BISHOP => minors.push((self.sq[s as usize].abs(), (s % 8 + s / 8) % 2)),
                _ => return false,
            }
        }
        match minors.len() {
            0 | 1 => true,
            _ => minors.iter().all(|m| m.0 == BISHOP) && minors.iter().all(|m| m.1 == minors[0].1),
        }
    }

    pub fn outcome(&self) -> Outcome {
        if self.legal().is_empty() {
            return if self.in_check() { Outcome::Win(self.turn ^ 1) } else { Outcome::Draw(DrawWhy::Stalemate) };
        }
        if self.halfmove >= 100 {
            return Outcome::Draw(DrawWhy::FiftyMoves);
        }
        if let Some(&now) = self.seen.last() {
            if self.seen.iter().filter(|&&k| k == now).count() >= 3 {
                return Outcome::Draw(DrawWhy::Repetition);
            }
        }
        if self.dead() {
            return Outcome::Draw(DrawWhy::DeadPosition);
        }
        Outcome::Playing
    }

    /// Count the positions `depth` moves deep (checks the move rules against known numbers).
    pub fn perft(&self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = self.legal();
        if depth == 1 {
            return moves.len() as u64;
        }
        moves
            .iter()
            .map(|&m| {
                let mut c = self.clone_light();
                c.play_raw(m);
                c.perft(depth - 1)
            })
            .sum()
    }

    /// A move written out ("e2-e4", "Nb1xc3", "e7-e8=Q", "O-O").
    pub fn describe(&self, m: CMove) -> String {
        let p = self.sq[m.from as usize].abs();
        if p == KING && (m.to as i8 - m.from as i8).abs() == 2 {
            return if m.to > m.from { "O-O".into() } else { "O-O-O".into() };
        }
        let letter = ["", "", "N", "B", "R", "Q", "K"][p.clamp(0, 6) as usize];
        let take = self.sq[m.to as usize] != 0 || (p == PAWN && m.from % 8 != m.to % 8);
        let promo = if m.promo != 0 { format!("={}", ["", "", "N", "B", "R", "Q"][m.promo.clamp(0, 5) as usize]) } else { String::new() };
        format!("{letter}{}{}{}{promo}", sq_name(m.from), if take { "x" } else { "-" }, sq_name(m.to))
    }

    // ---- the bot

    fn material(&self) -> i32 {
        const VAL: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];
        self.sq.iter().map(|v| VAL[v.unsigned_abs().min(6) as usize]).sum()
    }

    fn eval(&self) -> i32 {
        // material plus a little for the centre, advanced pawns and developed pieces
        const VAL: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];
        let endgame = self.material() <= 2600;
        let mut s = 0;
        for q in 0..64u8 {
            let v = self.sq[q as usize];
            if v == 0 {
                continue;
            }
            let side = v.signum() as i32;
            let p = v.abs();
            let (f, r) = ((q % 8) as i32, (q / 8) as i32);
            let rel = if side > 0 { r } else { 7 - r };
            let centre = 6 - ((2 * f - 7).abs() + (2 * r - 7).abs()) / 2;
            let bonus = match p {
                PAWN => rel * 6 + if (2..=5).contains(&f) { rel * 2 } else { 0 },
                KNIGHT | BISHOP => centre * 4 - if rel == 0 { 10 } else { 0 },
                QUEEN => centre,
                KING => {
                    if endgame { centre * 4 } else { -centre * 3 + if rel == 0 && (f <= 2 || f >= 6) { 20 } else { 0 } }
                }
                _ => 0,
            };
            s += side * (VAL[p as usize] + bonus);
        }
        if self.turn == 0 { s } else { -s }
    }

    fn search(&self, depth: u32, mut alpha: i32, beta: i32, nodes: &mut u64, captures_only: bool) -> i32 {
        *nodes += 1;
        if captures_only {
            let stand = self.eval();
            if stand >= beta || *nodes > 400_000 {
                return stand.min(beta);
            }
            alpha = alpha.max(stand);
        }
        let mut moves = self.legal();
        if moves.is_empty() {
            if captures_only {
                return if self.in_check() { -100_000 } else { alpha };
            }
            return if self.in_check() { -100_000 - depth as i32 } else { 0 };
        }
        if !captures_only && depth == 0 {
            return self.search(0, alpha, beta, nodes, true);
        }
        if captures_only {
            moves.retain(|m| self.sq[m.to as usize] != 0 || m.promo != 0);
        }
        // captures of big pieces by small ones first
        moves.sort_by_key(|m| -(self.sq[m.to as usize].abs() as i32 * 10 - self.sq[m.from as usize].abs() as i32) - m.promo as i32 * 10);
        for m in moves {
            let mut c = self.clone_light();
            c.play_raw(m);
            let v = -c.search(depth.saturating_sub(1), -beta, -alpha, nodes, captures_only);
            if v >= beta {
                return beta;
            }
            alpha = alpha.max(v);
        }
        alpha
    }

    /// The bot's move at this level (None when the game is over).
    pub fn bot_move(&self, level: Level, rng: &mut Rng) -> Option<CMove> {
        let moves = self.legal();
        if moves.is_empty() {
            return None;
        }
        let depth = match level {
            Level::Easy => 1,
            Level::Medium => 2,
            Level::Hard => 3,
        };
        let mut best: Vec<(i32, CMove)> = Vec::new();
        let mut nodes = 0u64;
        for &m in &moves {
            let mut c = self.clone_light();
            c.play_raw(m);
            let mut v = -c.search(depth - 1, -1_000_000, 1_000_000, &mut nodes, false);
            // when ahead, steer clear of repeating the position
            let k = c.key();
            if self.seen.iter().filter(|&&x| x == k).count() >= 2 && self.eval() > 50 {
                v -= 500;
            }
            best.push((v, m));
        }
        best.sort_by_key(|b| -b.0);
        // the easy bot slips now and then: one of its three best moves
        let pick = match level {
            Level::Easy if best[0].0 < 50_000 => rng.below(best.len().min(3)),
            Level::Medium if rng.unit() < 0.15 && best[0].0 < 50_000 => rng.below(best.len().min(2)),
            _ => {
                let top = best[0].0;
                rng.below(best.iter().take_while(|b| b.0 == top).count())
            }
        };
        Some(best[pick].1)
    }

    /// The position as bytes (for sending): 64 squares, then turn, castling bits, en passant
    /// (255 none), half-move and full-move counts.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = self.sq.iter().map(|&v| v as u8).collect();
        b.push(self.turn);
        b.push(self.castle.iter().enumerate().map(|(k, &c)| (c as u8) << k).sum());
        b.push(self.ep.unwrap_or(255));
        b.extend_from_slice(&self.halfmove.to_le_bytes());
        b.extend_from_slice(&self.fullmove.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Chess> {
        if b.len() < 71 {
            return None;
        }
        let mut sq = [0i8; 64];
        for k in 0..64 {
            let v = b[k] as i8;
            if !(-6..=6).contains(&v) {
                return None;
            }
            sq[k] = v;
        }
        let castle = [b[65] & 1 != 0, b[65] & 2 != 0, b[65] & 4 != 0, b[65] & 8 != 0];
        let ep = if b[66] < 64 { Some(b[66]) } else { None };
        Some(Chess { sq, turn: b[64] & 1, castle, ep, halfmove: u16::from_le_bytes([b[67], b[68]]), fullmove: u16::from_le_bytes([b[69], b[70]]), seen: Vec::new(), moves: Vec::new() })
    }
}

// ================================================================================== checkers

/// A checkers move: the squares the piece stands on, in order (two for a step, more for a run
/// of jumps). Squares as in chess (a1 = 0), dark squares only.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DMove(pub Vec<u8>);

impl DMove {
    pub fn is_jump(&self) -> bool {
        self.0.windows(2).any(|w| (w[0] as i8 - w[1] as i8).abs() > 9)
    }
}

/// English draughts. Side 0 (black, moving first) starts on the far three rows and moves down
/// the board; side 1 (red) starts on the near three rows and moves up. Pieces: 1 man, 2 king,
/// black negative.
#[derive(Clone, Debug, PartialEq)]
pub struct Checkers {
    pub sq: [i8; 64],
    pub turn: u8,
    /// Plies since the last capture or man move.
    pub quiet: u16,
    pub seen: Vec<u64>,
    pub moves: Vec<DMove>,
}

impl Default for Checkers {
    fn default() -> Self {
        Checkers::new()
    }
}

/// The squares checkers is played on (a1 is one).
pub fn dark(s: u8) -> bool {
    (s % 8 + s / 8) % 2 == 0
}

impl Checkers {
    pub fn new() -> Checkers {
        let mut sq = [0i8; 64];
        for s in 0..64u8 {
            if dark(s) {
                if s / 8 <= 2 {
                    sq[s as usize] = 1;
                } else if s / 8 >= 5 {
                    sq[s as usize] = -1;
                }
            }
        }
        Checkers::with(sq, 0)
    }

    /// A position with these pieces and this side to move.
    pub fn with(sq: [i8; 64], turn: u8) -> Checkers {
        let mut c = Checkers { sq, turn, quiet: 0, seen: Vec::new(), moves: Vec::new() };
        c.seen.push(c.key());
        c
    }

    fn me(&self) -> i8 {
        if self.turn == 0 { -1 } else { 1 }
    }

    fn key(&self) -> u64 {
        hash_of(&(self.sq, self.turn))
    }

    /// Directions a piece moves in (black men go down the board, red men up, kings both ways).
    fn dirs(v: i8) -> &'static [(i8, i8)] {
        match v {
            1 => &[(1, 1), (-1, 1)],
            -1 => &[(1, -1), (-1, -1)],
            _ => &[(1, 1), (-1, 1), (1, -1), (-1, -1)],
        }
    }

    /// Every run of jumps from `s` (with `taken` already jumped), carried on as far as it goes.
    fn jumps(&self, s: u8, v: i8, path: &mut Vec<u8>, taken: &mut Vec<u8>, out: &mut Vec<DMove>) {
        let me = v.signum();
        let (f, r) = ((s % 8) as i8, (s / 8) as i8);
        let mut any = false;
        // (a man reaching the far row is crowned and stops there)
        let crowned_now = v.abs() == 1 && path.len() > 1 && ((me > 0 && r == 7) || (me < 0 && r == 0));
        if !crowned_now {
            for &(df, dr) in Self::dirs(v) {
                let (Some(mid), Some(to)) = (at(f + df, r + dr), at(f + 2 * df, r + 2 * dr)) else { continue };
                let mv = self.sq[mid as usize];
                if mv * me < 0 && !taken.contains(&mid) && (self.sq[to as usize] == 0 || to == path[0]) {
                    any = true;
                    path.push(to);
                    taken.push(mid);
                    self.jumps(to, v, path, taken, out);
                    path.pop();
                    taken.pop();
                }
            }
        }
        if !any && path.len() > 1 {
            out.push(DMove(path.clone()));
        }
    }

    /// Every legal move: the jumps if there are any (taking is forced), otherwise the steps.
    pub fn legal(&self) -> Vec<DMove> {
        let me = self.me();
        let mut jumps = Vec::new();
        for s in 0..64u8 {
            let v = self.sq[s as usize];
            if v * me > 0 {
                let mut path = vec![s];
                self.jumps(s, v, &mut path, &mut Vec::new(), &mut jumps);
            }
        }
        if !jumps.is_empty() {
            return jumps;
        }
        let mut steps = Vec::new();
        for s in 0..64u8 {
            let v = self.sq[s as usize];
            if v * me > 0 {
                let (f, r) = ((s % 8) as i8, (s / 8) as i8);
                for &(df, dr) in Self::dirs(v) {
                    if let Some(t) = at(f + df, r + dr) {
                        if self.sq[t as usize] == 0 {
                            steps.push(DMove(vec![s, t]));
                        }
                    }
                }
            }
        }
        steps
    }

    fn play_raw(&mut self, m: &DMove) {
        let from = m.0[0];
        let to = *m.0.last().unwrap_or(&from);
        let v = self.sq[from as usize];
        let mut capture = false;
        for w in m.0.windows(2) {
            if (w[0] as i8 - w[1] as i8).abs() > 9 {
                self.sq[((w[0] as usize) + (w[1] as usize)) / 2] = 0;
                capture = true;
            }
        }
        self.sq[from as usize] = 0;
        let crown = v.abs() == 1 && ((v > 0 && to / 8 == 7) || (v < 0 && to / 8 == 0));
        self.sq[to as usize] = if crown { 2 * v.signum() } else { v };
        self.quiet = if capture || v.abs() == 1 { 0 } else { self.quiet + 1 };
        self.turn ^= 1;
    }

    pub fn play(&mut self, m: &DMove) -> bool {
        if m.0.len() < 2 || self.outcome() != Outcome::Playing || !self.legal().contains(m) {
            return false;
        }
        self.play_raw(m);
        self.moves.push(m.clone());
        self.seen.push(self.key());
        true
    }

    pub fn outcome(&self) -> Outcome {
        if self.legal().is_empty() {
            return Outcome::Win(self.turn ^ 1);
        }
        if self.quiet >= 80 {
            return Outcome::Draw(DrawWhy::NoProgress);
        }
        if let Some(&now) = self.seen.last() {
            if self.seen.iter().filter(|&&k| k == now).count() >= 3 {
                return Outcome::Draw(DrawWhy::Repetition);
            }
        }
        Outcome::Playing
    }

    fn eval(&self) -> i32 {
        let mut s = 0;
        for q in 0..64u8 {
            let v = self.sq[q as usize];
            if v == 0 {
                continue;
            }
            let r = (q / 8) as i32;
            let adv = if v > 0 { r } else { 7 - r };
            let f = (q % 8) as i32;
            let edge = if f == 0 || f == 7 { 4 } else { 0 };
            let val = if v.abs() == 2 { 170 } else { 100 + adv * 6 } + edge;
            s += v.signum() as i32 * val;
        }
        // (black is side 0 and negative)
        if self.turn == 0 { -s } else { s }
    }

    fn search(&self, depth: u32, mut alpha: i32, beta: i32) -> i32 {
        let moves = self.legal();
        if moves.is_empty() {
            return -100_000 - depth as i32;
        }
        if depth == 0 {
            return self.eval();
        }
        for m in moves {
            let mut c = Checkers { sq: self.sq, turn: self.turn, quiet: self.quiet, seen: Vec::new(), moves: Vec::new() };
            c.play_raw(&m);
            let v = -c.search(depth - 1, -beta, -alpha);
            if v >= beta {
                return beta;
            }
            alpha = alpha.max(v);
        }
        alpha
    }

    pub fn bot_move(&self, level: Level, rng: &mut Rng) -> Option<DMove> {
        let moves = self.legal();
        if moves.is_empty() {
            return None;
        }
        let depth = match level {
            Level::Easy => 1,
            Level::Medium => 4,
            Level::Hard => 6,
        };
        let mut scored: Vec<(i32, DMove)> = moves
            .into_iter()
            .map(|m| {
                let mut c = Checkers { sq: self.sq, turn: self.turn, quiet: self.quiet, seen: Vec::new(), moves: Vec::new() };
                c.play_raw(&m);
                (-c.search(depth - 1, -1_000_000, 1_000_000), m)
            })
            .collect();
        scored.sort_by_key(|s| -s.0);
        let pick = match level {
            Level::Easy => rng.below(scored.len().min(3)),
            _ => {
                let top = scored[0].0;
                rng.below(scored.iter().take_while(|s| s.0 == top).count())
            }
        };
        Some(scored[pick].1.clone())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = self.sq.iter().map(|&v| v as u8).collect();
        b.push(self.turn);
        b.extend_from_slice(&self.quiet.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Checkers> {
        if b.len() < 67 {
            return None;
        }
        let mut sq = [0i8; 64];
        for k in 0..64 {
            let v = b[k] as i8;
            if !(-2..=2).contains(&v) || (v != 0 && !dark(k as u8)) {
                return None;
            }
            sq[k] = v;
        }
        Some(Checkers { sq, turn: b[64] & 1, quiet: u16::from_le_bytes([b[65], b[66]]), seen: Vec::new(), moves: Vec::new() })
    }
}

// ================================================================================= connect 4

pub const C4_COLS: usize = 7;
pub const C4_ROWS: usize = 6;

/// Connect 4: `cells[col][row]`, row 0 at the bottom; 1 for side 0 (red, first), 2 for side 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Connect4 {
    pub cells: [[u8; C4_ROWS]; C4_COLS],
    pub turn: u8,
    pub moves: Vec<u8>,
}

impl Default for Connect4 {
    fn default() -> Self {
        Connect4::new()
    }
}

impl Connect4 {
    pub fn new() -> Connect4 {
        Connect4 { cells: [[0; C4_ROWS]; C4_COLS], turn: 0, moves: Vec::new() }
    }

    pub fn height(&self, c: usize) -> usize {
        self.cells[c].iter().take_while(|&&v| v != 0).count()
    }

    pub fn legal(&self) -> Vec<u8> {
        if self.outcome() != Outcome::Playing {
            return Vec::new();
        }
        (0..C4_COLS as u8).filter(|&c| self.height(c as usize) < C4_ROWS).collect()
    }

    fn drop_raw(&mut self, c: usize) {
        let h = self.height(c);
        self.cells[c][h] = self.turn + 1;
        self.turn ^= 1;
    }

    pub fn play(&mut self, c: u8) -> bool {
        if (c as usize) >= C4_COLS || !self.legal().contains(&c) {
            return false;
        }
        self.drop_raw(c as usize);
        self.moves.push(c);
        true
    }

    /// The four in a row, if someone has one.
    pub fn line(&self) -> Option<[(usize, usize); 4]> {
        for c in 0..C4_COLS as i32 {
            for r in 0..C4_ROWS as i32 {
                let v = self.cells[c as usize][r as usize];
                if v == 0 {
                    continue;
                }
                for (dc, dr) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                    let cells: Vec<(i32, i32)> = (0..4).map(|k| (c + dc * k, r + dr * k)).collect();
                    if cells.iter().all(|&(x, y)| (0..C4_COLS as i32).contains(&x) && (0..C4_ROWS as i32).contains(&y) && self.cells[x as usize][y as usize] == v) {
                        return Some([0, 1, 2, 3].map(|k| (cells[k].0 as usize, cells[k].1 as usize)));
                    }
                }
            }
        }
        None
    }

    pub fn outcome(&self) -> Outcome {
        if let Some(l) = self.line() {
            return Outcome::Win(self.cells[l[0].0][l[0].1] - 1);
        }
        if (0..C4_COLS).all(|c| self.height(c) == C4_ROWS) {
            return Outcome::Draw(DrawWhy::FullBoard);
        }
        Outcome::Playing
    }

    fn eval(&self) -> i32 {
        // every window of four: worth more the more of one side's discs it holds and none of
        // the other's; the middle column a little extra
        let me = self.turn + 1;
        let mut s = 0;
        for c in 0..C4_COLS as i32 {
            for r in 0..C4_ROWS as i32 {
                for (dc, dr) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                    let (ec, er) = (c + dc * 3, r + dr * 3);
                    if !(0..C4_COLS as i32).contains(&ec) || !(0..C4_ROWS as i32).contains(&er) {
                        continue;
                    }
                    let (mut mine, mut theirs) = (0, 0);
                    for k in 0..4 {
                        let v = self.cells[(c + dc * k) as usize][(r + dr * k) as usize];
                        if v == me {
                            mine += 1;
                        } else if v != 0 {
                            theirs += 1;
                        }
                    }
                    if theirs == 0 {
                        s += [0, 1, 6, 40, 0][mine];
                    } else if mine == 0 {
                        s -= [0, 1, 7, 50, 0][theirs];
                    }
                }
            }
        }
        s + self.cells[3].iter().filter(|&&v| v == me).count() as i32 * 3
    }

    fn search(&self, depth: u32, mut alpha: i32, beta: i32) -> i32 {
        if self.line().is_some() {
            // the side that just moved won
            return -100_000 - depth as i32;
        }
        let moves = self.legal();
        if moves.is_empty() {
            return 0;
        }
        if depth == 0 {
            return self.eval();
        }
        // middle columns first
        let mut order = moves;
        order.sort_by_key(|&c| (c as i32 - 3).abs());
        for c in order {
            let mut n = self.clone();
            n.drop_raw(c as usize);
            let v = -n.search(depth - 1, -beta, -alpha);
            if v >= beta {
                return beta;
            }
            alpha = alpha.max(v);
        }
        alpha
    }

    pub fn bot_move(&self, level: Level, rng: &mut Rng) -> Option<u8> {
        let moves = self.legal();
        if moves.is_empty() {
            return None;
        }
        let depth = match level {
            Level::Easy => 2,
            Level::Medium => 4,
            Level::Hard => 7,
        };
        let mut scored: Vec<(i32, u8)> = moves
            .iter()
            .map(|&c| {
                let mut n = self.clone();
                n.drop_raw(c as usize);
                (-n.search(depth - 1, -1_000_000, 1_000_000), c)
            })
            .collect();
        scored.sort_by_key(|s| (-s.0, (s.1 as i32 - 3).abs()));
        let pick = match level {
            Level::Easy if rng.unit() < 0.35 && scored[0].0 < 50_000 => rng.below(scored.len()),
            _ => {
                let top = scored[0].0;
                rng.below(scored.iter().take_while(|s| s.0 == top).count())
            }
        };
        Some(scored[pick].1)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = self.cells.iter().flat_map(|c| c.iter().copied()).collect();
        b.push(self.turn);
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Connect4> {
        if b.len() < C4_COLS * C4_ROWS + 1 {
            return None;
        }
        let mut cells = [[0u8; C4_ROWS]; C4_COLS];
        for c in 0..C4_COLS {
            for r in 0..C4_ROWS {
                let v = b[c * C4_ROWS + r];
                if v > 2 {
                    return None;
                }
                cells[c][r] = v;
            }
        }
        Some(Connect4 { cells, turn: b[C4_COLS * C4_ROWS] & 1, moves: Vec::new() })
    }
}

// ===================================================================================== a game

/// Which game a table holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameKind {
    Chess = 0,
    Checkers = 1,
    Connect4 = 2,
}

impl GameKind {
    pub const ALL: [GameKind; 3] = [GameKind::Chess, GameKind::Checkers, GameKind::Connect4];
    pub fn from_u8(v: u8) -> GameKind {
        match v {
            1 => GameKind::Checkers,
            2 => GameKind::Connect4,
            _ => GameKind::Chess,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            GameKind::Chess => "Chess",
            GameKind::Checkers => "Checkers",
            GameKind::Connect4 => "Connect 4",
        }
    }
    /// What each side is called.
    pub fn sides(self) -> [&'static str; 2] {
        match self {
            GameKind::Chess => ["White", "Black"],
            GameKind::Checkers => ["Black", "Red"],
            GameKind::Connect4 => ["Red", "Yellow"],
        }
    }
}

/// A move in any of the games (as it goes over the network).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AnyMove {
    Chess(CMove),
    Checkers(DMove),
    Drop(u8),
}

impl AnyMove {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            AnyMove::Chess(m) => vec![0, m.from, m.to, m.promo as u8],
            AnyMove::Checkers(m) => {
                let mut b = vec![1, m.0.len().min(16) as u8];
                b.extend(m.0.iter().take(16));
                b
            }
            AnyMove::Drop(c) => vec![2, *c],
        }
    }
    /// A move and how many bytes it took.
    pub fn from_bytes(b: &[u8]) -> Option<(AnyMove, usize)> {
        match *b.first()? {
            0 if b.len() >= 4 => Some((AnyMove::Chess(CMove { from: b[1].min(63), to: b[2].min(63), promo: (b[3] as i8).clamp(0, 5) }), 4)),
            1 if b.len() >= 2 => {
                let n = (b[1] as usize).min(16);
                let path: Vec<u8> = b.get(2..2 + n)?.iter().map(|&s| s.min(63)).collect();
                Some((AnyMove::Checkers(DMove(path)), 2 + n))
            }
            2 if b.len() >= 2 => Some((AnyMove::Drop(b[1].min(6)), 2)),
            _ => None,
        }
    }
}

/// One game on a table.
#[derive(Clone, Debug, PartialEq)]
pub enum Board {
    Chess(Chess),
    Checkers(Checkers),
    Connect4(Connect4),
}

impl Board {
    pub fn new(k: GameKind) -> Board {
        match k {
            GameKind::Chess => Board::Chess(Chess::new()),
            GameKind::Checkers => Board::Checkers(Checkers::new()),
            GameKind::Connect4 => Board::Connect4(Connect4::new()),
        }
    }
    pub fn kind(&self) -> GameKind {
        match self {
            Board::Chess(_) => GameKind::Chess,
            Board::Checkers(_) => GameKind::Checkers,
            Board::Connect4(_) => GameKind::Connect4,
        }
    }
    /// Whose turn: side 0 or 1.
    pub fn turn(&self) -> u8 {
        match self {
            Board::Chess(c) => c.turn,
            Board::Checkers(c) => c.turn,
            Board::Connect4(c) => c.turn,
        }
    }
    pub fn outcome(&self) -> Outcome {
        match self {
            Board::Chess(c) => c.outcome(),
            Board::Checkers(c) => c.outcome(),
            Board::Connect4(c) => c.outcome(),
        }
    }
    pub fn play(&mut self, m: &AnyMove) -> bool {
        match (self, m) {
            (Board::Chess(c), AnyMove::Chess(m)) => c.play(*m),
            (Board::Checkers(c), AnyMove::Checkers(m)) => c.play(m),
            (Board::Connect4(c), AnyMove::Drop(col)) => c.play(*col),
            _ => false,
        }
    }
    pub fn bot_move(&self, level: Level, rng: &mut Rng) -> Option<AnyMove> {
        match self {
            Board::Chess(c) => c.bot_move(level, rng).map(AnyMove::Chess),
            Board::Checkers(c) => c.bot_move(level, rng).map(AnyMove::Checkers),
            Board::Connect4(c) => c.bot_move(level, rng).map(AnyMove::Drop),
        }
    }
    /// The last move written out.
    pub fn last_move_text(&self) -> Option<String> {
        match self {
            Board::Chess(c) => c.moves.last().map(|m| format!("{}-{}", sq_name(m.from), sq_name(m.to))),
            Board::Checkers(c) => c.moves.last().map(|m| m.0.iter().map(|&s| sq_name(s)).collect::<Vec<_>>().join(if m.is_jump() { "x" } else { "-" })),
            Board::Connect4(c) => c.moves.last().map(|m| format!("column {}", m + 1)),
        }
    }
    /// The position as bytes (the kind first). The history isn't sent: the host decides draws
    /// by repetition and sends the outcome with it.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![self.kind() as u8];
        b.extend(match self {
            Board::Chess(c) => c.to_bytes(),
            Board::Checkers(c) => c.to_bytes(),
            Board::Connect4(c) => c.to_bytes(),
        });
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Board> {
        let rest = b.get(1..)?;
        Some(match GameKind::from_u8(*b.first()?) {
            GameKind::Chess => Board::Chess(Chess::from_bytes(rest)?),
            GameKind::Checkers => Board::Checkers(Checkers::from_bytes(rest)?),
            GameKind::Connect4 => Board::Connect4(Connect4::from_bytes(rest)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(c: &Chess, s: &str) -> CMove {
        let b = s.as_bytes();
        let sq = |i: usize| (b[i + 1] - b'1') * 8 + (b[i] - b'a');
        let promo = match b.get(4) {
            Some(b'q') => QUEEN,
            Some(b'r') => ROOK,
            Some(b'b') => BISHOP,
            Some(b'n') => KNIGHT,
            _ => 0,
        };
        let m = CMove { from: sq(0), to: sq(2), promo };
        assert!(c.legal().contains(&m), "{s} should be legal");
        m
    }

    fn play(c: &mut Chess, line: &str) {
        for s in line.split_whitespace() {
            let m = mv(c, s);
            assert!(c.play(m), "{s}");
        }
    }

    #[test]
    fn chess_move_counts_match_the_known_numbers() {
    let _quiet = crate::heavy();
        // the standard "perft" counts: every rule (castling, en passant, promotion, pins,
        // checks) has to be right for these to come out exactly
        let start = Chess::new();
        assert_eq!(start.perft(1), 20);
        assert_eq!(start.perft(2), 400);
        assert_eq!(start.perft(3), 8_902);
        assert_eq!(start.perft(4), 197_281);
        let kiwipete = Chess::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(kiwipete.perft(1), 48);
        assert_eq!(kiwipete.perft(2), 2_039);
        let p3 = Chess::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(p3.perft(4), 43_238);
        let p4 = Chess::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap();
        assert_eq!(p4.perft(3), 9_467);
        let p5 = Chess::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();
        assert_eq!(p5.perft(2), 1_486);
    }

    #[test]
    fn castling_needs_a_clear_safe_path() {
        let c = Chess::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let ms = c.legal();
        assert!(ms.contains(&CMove { from: 4, to: 6, promo: 0 }), "king side");
        assert!(ms.contains(&CMove { from: 4, to: 2, promo: 0 }), "queen side");
        // a rook watching f1: no king-side castle (passing through check)
        let c = Chess::from_fen("r3k2r/8/8/8/8/8/5r2/R3K2R w KQkq - 0 1").unwrap();
        assert!(!c.legal().contains(&CMove { from: 4, to: 6, promo: 0 }));
        // in check: no castling at all
        let c = Chess::from_fen("r3k2r/8/8/8/8/8/4r3/R3K2R w KQkq - 0 1").unwrap();
        assert!(!c.legal().iter().any(|m| m.from == 4 && (m.to == 6 || m.to == 2)));
        // the king moved: rights gone
        let mut c = Chess::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        play(&mut c, "e1e2 e8e7 e2e1 e7e8");
        assert!(!c.legal().iter().any(|m| m.from == 4 && m.to == 6));
        // castling moves the rook
        let mut c = Chess::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        play(&mut c, "e1g1");
        assert_eq!(c.sq[5], ROOK);
        assert_eq!(c.sq[7], 0);
    }

    #[test]
    fn en_passant_only_straight_after_the_double_step() {
        let mut c = Chess::new();
        play(&mut c, "e2e4 a7a6 e4e5 d7d5");
        let ep = mv(&c, "e5d6");
        assert!(c.play(ep));
        assert_eq!(c.sq[35], 0, "the black pawn on d5 is gone");
        let mut c = Chess::new();
        play(&mut c, "e2e4 a7a6 e4e5 d7d5 a2a3 a6a5");
        assert!(!c.legal().contains(&CMove { from: 36, to: 43, promo: 0 }), "too late");
    }

    #[test]
    fn promotion_to_any_piece() {
        let c = Chess::from_fen("8/P6k/8/8/8/8/8/K7 w - - 0 1").unwrap();
        let ms: Vec<i8> = c.legal().iter().filter(|m| m.from == 48).map(|m| m.promo).collect();
        assert_eq!(ms.len(), 4);
        let mut c2 = c.clone();
        play(&mut c2, "a7a8n");
        assert_eq!(c2.sq[56], KNIGHT);
        // given without a piece, it queens
        let mut c3 = c.clone();
        assert!(c3.play(CMove { from: 48, to: 56, promo: 0 }));
        assert_eq!(c3.sq[56], QUEEN);
    }

    #[test]
    fn mate_stalemate_and_the_draws() {
        let mut c = Chess::new();
        play(&mut c, "f2f3 e7e5 g2g4 d8h4");
        assert_eq!(c.outcome(), Outcome::Win(1), "fool's mate");
        let s = Chess::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert_eq!(s.outcome(), Outcome::Draw(DrawWhy::Stalemate));
        let mut r = Chess::new();
        play(&mut r, "g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1 f6g8");
        assert_eq!(r.outcome(), Outcome::Draw(DrawWhy::Repetition));
        let mut f = Chess::from_fen("8/8/8/4k3/8/8/4K3/4R3 w - - 99 80").unwrap();
        play(&mut f, "e1d1");
        assert_eq!(f.outcome(), Outcome::Draw(DrawWhy::FiftyMoves));
        for dead in ["8/8/8/4k3/8/8/4K3/8 w - - 0 1", "8/8/8/4k3/8/8/4K3/5B2 w - - 0 1", "8/8/8/4k3/8/8/4K3/5N2 w - - 0 1", "8/8/2b5/4k3/8/8/4K3/5B2 w - - 0 1"] {
            assert_eq!(Chess::from_fen(dead).unwrap().outcome(), Outcome::Draw(DrawWhy::DeadPosition), "{dead}");
        }
        // bishops on squares of both colours can still mate
        assert_eq!(Chess::from_fen("8/8/3b4/4k3/8/8/4K3/5B2 w - - 0 1").unwrap().outcome(), Outcome::Playing);
        // nothing more is played once it's over
        assert!(!c.play(CMove { from: 12, to: 28, promo: 0 }));
    }

    #[test]
    fn illegal_moves_are_refused() {
        let mut c = Chess::new();
        assert!(!c.play(CMove { from: 12, to: 36, promo: 0 }), "a pawn can't step three");
        assert_eq!(c, Chess::new());
        // pinned: the knight on d2 can't leave the king's line
        let mut p = Chess::from_fen("4k3/8/8/8/1b6/8/3N4/4K3 w - - 0 1").unwrap();
        assert!(!p.play(CMove { from: 11, to: 26, promo: 0 }));
    }

    #[test]
    fn the_chess_bot_takes_a_free_queen_and_mates_in_one() {
        let _quiet = crate::heavy();
        let mut rng = Rng(7);
        let c = Chess::from_fen("4k3/8/8/3q4/8/8/8/3QK3 w - - 0 1").unwrap();
        for l in [Level::Medium, Level::Hard] {
            assert_eq!(c.bot_move(l, &mut rng).unwrap().to, 35, "{l:?} takes the queen");
        }
        let m = Chess::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1").unwrap();
        for l in Level::ALL {
            let b = m.bot_move(l, &mut rng).unwrap();
            let mut after = m.clone();
            after.play(b);
            assert_eq!(after.outcome(), Outcome::Win(0), "{l:?} finds the back-rank mate");
        }
    }

    #[test]
    fn checkers_taking_is_forced_and_jumps_carry_on() {
        let c = Checkers::new();
        assert_eq!(c.legal().len(), 7, "black's seven opening steps");
        let s = |f: u8, r: u8| r * 8 + f;
        // a black man that can take two red men in a row
        let mut sq = [0i8; 64];
        sq[s(4, 6) as usize] = -1;
        sq[s(3, 5) as usize] = 1;
        sq[s(1, 3) as usize] = 1;
        sq[s(7, 1) as usize] = 1;
        let mut j = Checkers::with(sq, 0);
        let ms = j.legal();
        assert_eq!(ms, vec![DMove(vec![s(4, 6), s(2, 4), s(0, 2)])], "the only move: both jumps");
        // stopping after the first jump, or stepping instead, is refused
        assert!(!j.play(&DMove(vec![s(4, 6), s(2, 4)])));
        assert!(!j.play(&DMove(vec![s(4, 6), s(5, 5)])));
        assert!(j.play(&ms[0]));
        assert_eq!(j.sq[s(3, 5) as usize], 0);
        assert_eq!(j.sq[s(1, 3) as usize], 0);
    }

    #[test]
    fn checkers_crowning_ends_the_move_and_kings_go_both_ways() {
        let s = |f: u8, r: u8| r * 8 + f;
        let mut sq = [0i8; 64];
        sq[s(2, 6) as usize] = 1;
        sq[s(7, 1) as usize] = -1;
        let mut c = Checkers::with(sq, 1);
        assert!(c.play(&DMove(vec![s(2, 6), s(1, 7)])));
        assert_eq!(c.sq[s(1, 7) as usize], 2, "crowned");
        // a red man jumping into the far row stops there even with another jump waiting
        let mut sq = [0i8; 64];
        sq[s(1, 5) as usize] = 1;
        sq[s(2, 6) as usize] = -1;
        sq[s(4, 6) as usize] = -1;
        sq[s(6, 0) as usize] = -1;
        let j = Checkers::with(sq, 1);
        assert_eq!(j.legal(), vec![DMove(vec![s(1, 5), s(3, 7)])]);
        // a king moves backwards too
        let mut sq = [0i8; 64];
        sq[s(3, 3) as usize] = 2;
        sq[s(7, 7) as usize] = -1;
        assert_eq!(Checkers::with(sq, 1).legal().len(), 4);
        // no pieces (or no moves) left: the other side wins
        let mut sq = [0i8; 64];
        sq[s(0, 0) as usize] = 1;
        assert_eq!(Checkers::with(sq, 0).outcome(), Outcome::Win(1));
    }

    #[test]
    fn checkers_draws_by_no_progress_and_repetition() {
        let s = |f: u8, r: u8| r * 8 + f;
        let mut sq = [0i8; 64];
        sq[s(0, 0) as usize] = 2;
        sq[s(7, 7) as usize] = -2;
        let mut c = Checkers::with(sq, 0);
        c.quiet = 79;
        let m = c.legal()[0].clone();
        assert!(c.play(&m));
        assert_eq!(c.outcome(), Outcome::Draw(DrawWhy::NoProgress));
        // two kings shuffling: the same position a third time
        let mut r = Checkers::with(sq, 0);
        for mv in [vec![s(7, 7), s(6, 6)], vec![s(0, 0), s(1, 1)], vec![s(6, 6), s(7, 7)], vec![s(1, 1), s(0, 0)], vec![s(7, 7), s(6, 6)], vec![s(0, 0), s(1, 1)], vec![s(6, 6), s(7, 7)], vec![s(1, 1), s(0, 0)]] {
            assert!(r.play(&DMove(mv)));
        }
        assert_eq!(r.outcome(), Outcome::Draw(DrawWhy::Repetition));
    }

    #[test]
    fn the_checkers_bot_plays_whole_games() {
    let _quiet = crate::heavy();
        let mut rng = Rng(3);
        let mut c = Checkers::new();
        let mut n = 0;
        while c.outcome() == Outcome::Playing && n < 400 {
            let lvl = if c.turn == 0 { Level::Medium } else { Level::Easy };
            let m = c.bot_move(lvl, &mut rng).unwrap();
            assert!(c.play(&m));
            n += 1;
        }
        assert_ne!(c.outcome(), Outcome::Playing, "it finishes");
        assert_ne!(c.outcome(), Outcome::Win(1), "medium doesn't lose to easy");
    }

    #[test]
    fn connect4_lines_in_every_direction() {
        let mut c = Connect4::new();
        for col in [0, 0, 1, 1, 2, 2, 3] {
            assert!(c.play(col));
        }
        assert_eq!(c.outcome(), Outcome::Win(0), "across");
        assert!(!c.play(5), "no move after a win");
        let mut v = Connect4::new();
        for col in [4, 5, 4, 5, 4, 5, 4] {
            v.play(col);
        }
        assert_eq!(v.outcome(), Outcome::Win(0), "up");
        let mut d = Connect4::new();
        for col in [0, 1, 1, 2, 2, 3, 2, 3, 3, 6, 3] {
            d.play(col);
        }
        assert_eq!(d.outcome(), Outcome::Win(0), "diagonal");
        let mut a = Connect4::new();
        for col in [6, 5, 5, 4, 4, 3, 4, 3, 3, 0, 3] {
            a.play(col);
        }
        assert_eq!(a.outcome(), Outcome::Win(0), "the other diagonal");
        let mut col = Connect4::new();
        for _ in 0..6 {
            col.play(0);
        }
        assert!(!col.play(0), "a full column");
    }

    #[test]
    fn connect4_full_board_is_a_draw() {
        let mut b = Connect4::new();
        b.cells = [[1, 1, 2, 1, 1, 2], [2, 2, 1, 2, 2, 1], [1, 1, 2, 1, 1, 2], [2, 2, 1, 2, 2, 1], [1, 1, 2, 1, 1, 2], [2, 2, 1, 2, 2, 1], [1, 1, 2, 1, 1, 2]];
        assert_eq!(b.line(), None);
        assert_eq!(b.outcome(), Outcome::Draw(DrawWhy::FullBoard));
        assert!(b.legal().is_empty());
    }

    #[test]
    fn the_connect4_bot_wins_and_blocks() {
    let _quiet = crate::heavy();
        let mut rng = Rng(11);
        let mut c = Connect4::new();
        for col in [0, 6, 1, 6, 2] {
            c.play(col);
        }
        for l in [Level::Medium, Level::Hard] {
            assert_eq!(c.bot_move(l, &mut rng), Some(3), "{l:?} blocks");
        }
        let mut w = Connect4::new();
        for col in [0, 6, 1, 6, 2, 5] {
            w.play(col);
        }
        assert_eq!(w.bot_move(Level::Medium, &mut rng), Some(3), "takes the win");
        // the hard bot beats the easy one
        let mut wins = 0;
        for seed in 0..4u64 {
            let mut rng = Rng(100 + seed);
            let mut g = Connect4::new();
            let hard = (seed % 2) as u8;
            while g.outcome() == Outcome::Playing {
                let l = if g.turn == hard { Level::Hard } else { Level::Easy };
                let m = g.bot_move(l, &mut rng).unwrap();
                g.play(m);
            }
            if g.outcome() == Outcome::Win(hard) {
                wins += 1;
            }
        }
        assert!(wins >= 3, "hard won {wins} of 4");
    }

    #[test]
    fn boards_and_moves_survive_the_wire() {
        let mut c = Chess::new();
        play(&mut c, "e2e4 d7d5");
        let b = Board::Chess(c.clone());
        match Board::from_bytes(&b.to_bytes()).unwrap() {
            Board::Chess(x) => {
                assert_eq!(x.sq, c.sq);
                assert_eq!(x.ep, c.ep);
                assert_eq!(x.legal(), c.legal());
            }
            _ => panic!("chess came back as something else"),
        }
        let k = Board::Checkers(Checkers::new());
        assert_eq!(Board::from_bytes(&k.to_bytes()).map(|b| b.kind()), Some(GameKind::Checkers));
        let f = Board::Connect4(Connect4::new());
        assert_eq!(Board::from_bytes(&f.to_bytes()).map(|b| b.kind()), Some(GameKind::Connect4));
        for m in [AnyMove::Chess(CMove { from: 12, to: 28, promo: 0 }), AnyMove::Checkers(DMove(vec![1, 10, 19])), AnyMove::Drop(3)] {
            let bytes = m.to_bytes();
            assert_eq!(AnyMove::from_bytes(&bytes), Some((m.clone(), bytes.len())));
        }
        for o in [Outcome::Playing, Outcome::Win(0), Outcome::Win(1), Outcome::Draw(DrawWhy::Repetition), Outcome::Draw(DrawWhy::Agreed)] {
            assert_eq!(Outcome::from_u8(o.to_u8()), o);
        }
        // junk is refused
        assert!(Board::from_bytes(&[0, 9, 9]).is_none());
        assert!(Chess::from_bytes(&[50; 71]).is_none());
    }
}
