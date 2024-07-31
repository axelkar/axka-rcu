# axka-rcu 

[![Crates.io](https://img.shields.io/crates/v/axka-rcu)](https://lib.rs/crates/axka-rcu)
[![Documentation](https://docs.rs/axka-rcu/badge.svg)](https://docs.rs/axka-rcu)

A reference-counted read-copy-update (RCU) primitive useful for protecting shared data

## Example

```rs
use std::{thread::sleep, time::Duration, sync::Arc};
use axka_rcu::Rcu;

#[derive(Clone, Debug, PartialEq)]
struct Player {
    name: &'static str,
    points: usize
}

let players = Arc::new(Rcu::new(Arc::new(vec![
    Player { name: "foo", points: 100 }
])));
let players2 = players.clone();

// Lock-free writing
std::thread::spawn(move || players2.update(|players| {
    sleep(Duration::from_millis(50));
    players.push(Player {
        name: "bar",
        points: players[0].points + 50
    })
}));

// Lock-free reading
assert_eq!(*players.read(), [
    Player { name: "foo", points: 100 }
]);

sleep(Duration::from_millis(60));
assert_eq!(*players.read(), [
    Player { name: "foo", points: 100 },
    Player { name: "bar", points: 150 }
]);
```

Check out the [documentation](https://docs.rs/axka-rcu) for more details.

## Contributing patches

Please first make sure that you have not introduced any regressions and format the code by running the following commands at the repository root.
```sh
cargo fmt
cargo clippy
cargo test
```

You can either make a GitHub [pull request](https://github.com/axelkar/axka-rcu/pulls) or email me directly:

0. Setup `git send-email`:

   <https://git-send-email.io/>

1. Commit your changes, this will open up a text editor

   `git commit`

2. Send your patches to me. The command sends the last commit

   `git send-email --to="axel@axka.fi" HEAD^`

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.


Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
