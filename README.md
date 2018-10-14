DeltaMap
========
A simple map viewer.

Building
--------
DeltaMap is written in Rust, so you will need to [install
Rust](https://www.rust-lang.org/install.html) to compile the program. The
minimum supported version of Rust for DeltaMap is **1.25**.

On Linux you will also need OpenSSL with headers.
(see <https://docs.rs/crate/openssl/0.10.12> for details)

```sh
# On Debian and Ubuntu
$ sudo apt-get install pkg-config libssl-dev
# On Arch Linux
$ sudo pacman -S openssl
# On Fedora
$ sudo dnf install openssl-devel
```

Build and install the latest release from crates.io:

```sh
$ cargo install deltamap
```

Build the latest development version:

```sh
$ git clone https://github.com/b-r-u/deltamap
$ cd deltamap
$ cargo build --release
$ ./target/release/deltamap
```
