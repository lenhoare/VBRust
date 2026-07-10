// The entry of a multi-file project: collections, strings, and constants all
// cross the module boundary. Pass 1 of the project compile harvested life.vbr's
// interface, so every qualified call below is argument-checked and borrowed
// exactly like a local call — `Life.SetCell(grid, …)` becomes
// `crate::life::setcell(&mut grid, …)`.

mod life;

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
}
