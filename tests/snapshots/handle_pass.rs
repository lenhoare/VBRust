// A Handle is a live Rust object Vinyl can pass, return, and store.
// Open it only inside a Rust block. ByRef lends it; ByVal would move it.

struct __VbrHandle(Box<dyn std::any::Any + Send>);
impl __VbrHandle {
    fn new<T: std::any::Any + Send>(value: T) -> Self {
        Self(Box::new(value))
    }
    fn with_mut<T: std::any::Any + Send, R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let inner = self.0.downcast_mut::<T>().unwrap_or_else(|| {
            panic!("this Handle holds a different Rust type than this `Rust` block uses")
        });
        f(inner)
    }
}

fn makewords() -> Result<__VbrHandle, String> {
    Ok(__VbrHandle::new("the quick brown fox".split_whitespace()))
}

fn nextword(words: &mut __VbrHandle) -> Result<String, String> {
    Ok(words.with_mut(|words| {
    let words: &mut std::str::SplitWhitespace = words;
    words.next().unwrap().to_string()
}))
}

fn vbr_main() -> Result<(), String> {
    #[allow(unused_mut)]
    let mut words: __VbrHandle = makewords()?;
    let first: String = nextword(&mut words)?;
    let second: String = nextword(&mut words)?;
    println!("first:  {}", first);
    println!("second: {}", second);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
