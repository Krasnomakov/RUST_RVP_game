fn main() {
    println!("Hello, Rust!");

    // Variables are immutable by default. Add `mut` when a value should change.
    let language = "Rust";
    let mut count = 1;
    count += 1;
    println!("Learning {language}: example {count}");

    // Functions declare parameter and return types.
    let total = add(7, 5);
    println!("7 + 5 = {total}");

    // `if` is an expression, so each branch can produce a value.
    let label = if total > 10 {
        "more than ten"
    } else {
        "ten or less"
    };
    println!("The total is {label}.");

    // A range and iterator make a simple loop.
    for n in 1..=3 {
        println!("counting: {n}");
    }
}

fn add(left: i32, right: i32) -> i32 {
    left + right // no semicolon: this expression is returned
}
