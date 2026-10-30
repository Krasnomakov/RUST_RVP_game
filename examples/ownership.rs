fn main() {
    let first = String::from("hello");
    let second = first; // ownership moves; `first` can no longer be used
    println!("{second}, Rust!");

    let word = String::from("borrow");
    print_length(&word); // borrow a reference; word remains usable
    println!("Still available: {word}");
}

fn print_length(text: &String) {
    println!("{text} has {} bytes", text.len());
}
