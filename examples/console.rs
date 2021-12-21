use std::io::prelude::*;

fn main() {
    let console = conpty::console::Console::current().unwrap();

    assert_eq!(true, console.is_stdin_empty().unwrap());

    console.set_raw().unwrap();

    println!("Type `]` character to exit");

    let mut buf = [0; 1];
    loop {
        let n = std::io::stdin().read(&mut buf).unwrap();
        if n == 0 {
            break;
        }

        assert_eq!(false, console.is_stdin_empty().unwrap());

        let c: char = buf[0].into();
        println!("char={}", c);

        if c == ']' {
            break;
        }
    }

    console.reset().unwrap();
}
