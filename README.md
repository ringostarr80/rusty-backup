# rusty-backup

## Prerequisites (Ubuntu)

```bash
sudo apt install build-essential
```

install [Rust](https://www.rust-lang.org/)

## Installation (Ubuntu)

```bash
git clone git@github.com:ringostarr80/rusty-backup.git
cd rusty-backup
cargo build --release
sudo cp target/release/rusty-backup /usr/local/bin
```