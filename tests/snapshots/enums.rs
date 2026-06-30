// Simple enums — a named set of variants. They're Copy, compare with `=`, and
// pair naturally with Match. Reference a variant as `Suit.Hearts` → `Suit::Hearts`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

fn color(s: &Suit) -> String {
    match s {
        Suit :: Hearts => {
            return "red".to_string();
        }
        Suit :: Diamonds => {
            return "red".to_string();
        }
        Suit :: Clubs => {
            return "black".to_string();
        }
        Suit :: Spades => {
            return "black".to_string();
        }
    }
}

fn main() {
    let s: Suit = Suit::Spades;
    println!("{}", format!("{}{}", "Spades are ", color(&s)));
    println!("{}", format!("{}{}", "Hearts are ", color(&Suit::Hearts)));
    if s == Suit::Spades {
        println!("{}", "yes, spades");
    }
}
