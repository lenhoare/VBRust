// The logic module of a miniature Game of Life project. Everything marked
// Public crosses the module boundary — including its *signature*: a caller in
// another file gets the same argument treatment as a local one (`&mut` for the
// ByRef grid, `&` for a ByVal Vec or String). `Hidden` and `SECRET` stay
// file-local; calling them from main.vbr earns a teaching error.

pub const WIDTH: i64 = 5;
pub const HEIGHT: i64 = 3;
const SECRET: i64 = 99;

pub fn newgrid() -> Vec<i64> {
    let mut g: Vec<i64> = vec![];
    for _ in 1..=WIDTH * HEIGHT {
        g.push(0);
    }
    g
}

pub fn setcell(grid: &mut Vec<i64>, x: i64, y: i64, v: i64) {
    grid[(y * WIDTH + x) as usize] = v;
}

pub fn countlive(grid: &Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for cell in &*grid {
        total = total + *cell;
    }
    total
}

pub fn formatrule(birth: &str, survive: &str) -> String {
    format!("B{}/S{}", birth, survive)
}

fn hidden() -> i64 {
    SECRET
}

pub fn checksum() -> i64 {
    hidden() + WIDTH
}
