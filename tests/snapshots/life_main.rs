// The entry of a multi-file project: collections, strings, and constants all
// cross the module boundary. Pass 1 of the project compile harvested life.vbr's
// interface, so every qualified call below is argument-checked and borrowed
// exactly like a local call — `Life.SetCell(grid, …)` becomes
// `crate::life::setcell(&mut grid, …)`.

mod life;

use crate::life::CellState;
use crate::life::Rule;

fn main() {
    let mut grid: Vec<i64> = crate::life::newgrid();
    crate::life::setcell(&mut grid, 1, 1, 1);
    crate::life::setcell(&mut grid, 2, 1, 1);
    crate::life::setcell(&mut grid, 3, 1, 1);
    println!("live cells: {}", crate::life::countlive(&grid));
    println!("grid: {} x {}", crate::life::WIDTH, crate::life::HEIGHT);
    let birth: String = "3".to_string();
    let survive: String = "23".to_string();
    println!("rule: {}", crate::life::formatrule(&birth, &survive));
    println!("checksum: {}", crate::life::checksum());
    // life.vbr's Public Type, by bare name — VB6 semantics, and also exactly
    // how Rust does it (a `use crate::life::Rule;` up top, then just `Rule`).
    let r: Rule = crate::life::classicrule();
    println!("classic: {}", r.describe());
    // Its Public Enum crosses the same way — variants, and `Match` patterns.
    match crate::life::stateof(grid[8]) {
        CellState :: Alive => {
            println!("centre cell is alive");
        }
        CellState :: Dead => {
            println!("centre cell is dead");
        }
    }
}
