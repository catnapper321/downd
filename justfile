default:
    just --list

install: build
    fd --maxdepth 1 -t f -t x . target/release | xargs install -m 700 -p -t $HOME/bin

build:
    cargo build --release

docs:
    rustup doc --std
    cargo doc --open

