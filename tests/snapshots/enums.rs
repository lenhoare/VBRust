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
impl std::fmt::Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn color(s: &Suit) -> Result<String, String> {
    match s {
        Suit :: Hearts => {
            return Ok("red".to_string());
        }
        Suit :: Diamonds => {
            return Ok("red".to_string());
        }
        Suit :: Clubs => {
            return Ok("black".to_string());
        }
        Suit :: Spades => {
            return Ok("black".to_string());
        }
    }
}

fn vbr_main() -> Result<(), String> {
    let s: Suit = Suit::Spades;
    println!("Spades are {}", color(&s)?);
    println!("Hearts are {}", color(&Suit::Hearts)?);
    if s == Suit::Spades {
        println!("yes, spades");
    }
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
