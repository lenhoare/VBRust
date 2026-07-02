// Struct fields, collection elements, and Me carry their declared types:
// mixed-width arithmetic through them gets the same automatic widening casts
// as plain variables, and a method that mutates Me only through a mutating
// method call (Push) still takes &mut self.

#[derive(Debug, Clone)]
struct Basket {
    label: String,
    rate: f64,
    qty: i32,
    weights: Vec<i64>,
}

impl Basket {
    fn add_weight(&mut self, w: i64) {
        self.weights.push(w);
    }

    fn total_weight(&self) -> i64 {
        let mut sum: i64 = 0;
        for w in &self.weights {
            sum += *w;
        }
        sum
    }
}

fn main() {
    let start: Vec<i64> = Vec::new();
    let mut b: Basket = Basket { label: "box".to_string(), rate: 2.5, qty: 3, weights: start };
    b.add_weight(10);
    b.add_weight(32);
    // A Double field times an Integer field — widened automatically.
    let cost: f64 = b.rate * (b.qty as f64);
    // An Integer field meets a Long variable the same way.
    let n: i64 = 100;
    let scaled: i64 = (b.qty as i64) * n;
    println!("{} cost {}, scaled {}, weight {}", b.label, cost, scaled, b.total_weight());
}
