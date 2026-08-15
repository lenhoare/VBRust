// The logic module of a miniature Game of Life project. Everything marked
// Public crosses the module boundary — including its *signature*: a caller in
// another file gets the same argument treatment as a local one (`&mut` for the
// ByRef grid, `&` for a ByVal Vec or String). A Public Type or Enum is
// project-global by its bare name, exactly as in VB6 — main.vbr writes `Rule`,
// not `Life.Rule`, and the generated Rust gets `use crate::life::Rule;`.
// `Hidden` and `SECRET` stay file-local; using them from main.vbr earns a
// teaching error.
// Crosses with its public methods — an `impl` block on the Rust side, `pub fn`
// for each Public Function. Fields main.vbr touches must be Public too.

pub const WIDTH: i64 = 5;
pub const HEIGHT: i64 = 3;
const SECRET: i64 = 99;

#[derive(Debug, Clone)]
pub struct Rule {
    pub birth: String,
    pub survive: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CellState {
    Dead,
    Alive,
}
impl std::fmt::Display for CellState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Rule {
    pub fn describe(&self) -> Result<String, String> {
        Ok(format!("B{}/S{}", self.birth, self.survive))
    }
}

pub fn classicrule() -> Result<Rule, String> {
    Ok(Rule { birth: "3".to_string(), survive: "23".to_string() })
}

pub fn stateof(v: i64) -> Result<CellState, String> {
    if v == 0 {
        return Ok(CellState::Dead);
    }
    Ok(CellState::Alive)
}

pub fn newgrid() -> Result<Vec<i64>, String> {
    let mut g: Vec<i64> = vec![];
    for _ in 1..=WIDTH * HEIGHT {
        g.push(0);
    }
    Ok(g)
}

pub fn setcell(grid: &mut Vec<i64>, x: i64, y: i64, v: i64) -> Result<(), String> {
    grid[(y * WIDTH + x) as usize] = v;
    Ok(())
}

pub fn countlive(grid: &Vec<i64>) -> Result<i64, String> {
    let mut total: i64 = 0;
    for cell in &*grid {
        total = total + *cell;
    }
    Ok(total)
}

pub fn formatrule(birth: &str, survive: &str) -> Result<String, String> {
    Ok(format!("B{}/S{}", birth, survive))
}

fn hidden() -> Result<i64, String> {
    Ok(SECRET)
}

pub fn checksum() -> Result<i64, String> {
    Ok(hidden()? + WIDTH)
}
