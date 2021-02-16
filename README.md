# Cargo Remote

***Use with caution, I didn't test this software well and it is a really hacky
(at least for now). If you want to test it please create a VM or at least a separate
user on your build host***

## Why I built it
One big annoyance when working on rust projects on my notebook are the compile
times. Since I'm using rust nightly for some of my projects I have to recompile
rather often. Currently there seem to be no good remote-build integrations for
rust, so I decided to build one my own.

## Planned capabilities
This first version is very simple (could have been a bash script), but I intend to
enhance it to a point where it detects compatibility between local and remote
versions, allows (nearly) all cargo commands and maybe even load distribution
over multiple machines.

## Usage
For now only `cargo remote [FLAGS] [OPTIONS] <command>` works: it copies the
current project to a temporary directory (`~/remote-builds/<project_name>`) on
the remote server, calls `cargo <command>` remotely and optionally (`-c`) copies
back the resulting target folder. This assumes that server and client are running
the same rust version and have the same processor architecture. On the client `ssh`
and `rsync` need to be installed.

If you want to pass remote flags you have to end the options/flags section using
`--`. E.g. to build in release mode and copy back the result use:
```bash
cargo remote \ 
    --base-path /data \
    -c target/release/yourbinname1 \
    -c target/release/yourbinname2 \
    -c target/release/yourdir/ \
    -b "RUST_BACKTRACE=1 WASM_BUILD_TYPE=release" \
    -e "/home/yourname/.profile" \
    -r yourname@1.1.1.1 \
    build -- \
    --release
```

### Flags and options
```
$ cargo remote -h
cargo-remote 0.2.0

USAGE:
    cargo remote [FLAGS] [OPTIONS] <command> --remote <build-server> [--] [remote options]...

FLAGS:
        --transfer-compress    Compress file data during the transfer
    -h, --help                 Prints help information
        --transfer-hidden      Transfer hidden files and directories to the build server
        --no-copy-lock         don't transfer the Cargo.lock file back to the local machine
    -V, --version              Prints version information

OPTIONS:
        --base-path <base-path>              the base dir of build path [default: ~]
    -b, --build-env <build-env>              Set remote environment variables. RUST_BACKTRACE, CC, LIB, etc.  [default:
                                             RUST_BACKTRACE=1]
    -r, --remote <build-server>              Remote ssh build server
    -c, --copy-back <copy-back>...           Transfer specific files or folders from that folder back to the local
                                             machine
    -e, --env <env>                          Environment profile. default_value = /etc/profile [default: /etc/profile]
        --manifest-path <manifest-path>      Path to the manifest to execute [default: Cargo.toml]
    -d, --rustup-default <rustup-default>    Rustup default (stable|beta|nightly) [default: stable]

ARGS:
    <command>              cargo command that will be executed remotely
    <remote options>...    cargo options and flags that will be applied remotely
```


## How to install
```bash
git clone https://github.com/sgeisler/cargo-remote
cargo install --path cargo-remote/
```

### MacOS Problems
It was reported that the `rsync` version shipped with MacOS doesn't support the progress flag and thus fails when
`cargo-remote` tries to use it. You can install a newer version by running
```bash
brew install rsync
```
See also [#10](https://github.com/sgeisler/cargo-remote/issues/10).
