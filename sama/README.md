# sama

sama is a scriptable, extendible emulator, and the reference software implementation of the lawa isa

## installation

the easiest way to install sama is using cargo, the package manager for the rust programming language. if cargo is not installed on your computer, consult [the installation instructions](https://www.rust-lang.org/tools/install). once cargo is installed, the newest version of sama may be installed by simply running

``` bash
cargo install --git https://codeberg.org/mra/lawa-binutils --bin sama
```

## usage

to start sama, simply run

```bash
sama
```

and the tui will appear. sama may be interacted with using the built-in lua repl, and the state of the emulator may be examined and modified through the `emulator` global. the widgets which appear in the tui to display the state of the emulator may be configured through the `widgets` global, by setting the fields shown in [the default init.lua](src/init.lua). when it is started, sama attempts to load and execute the contents of the user's configuration file, which is located at `sama/init.lua` within the user's configuration directory (on linux, either `$XDG_CONFIG_DIR`, or `$HOME/.config` if the former is not set). the built-in lua repl also provides a `reload_configuration` function which, as the name suggests, attempts to load and execute the contents of the user's configuration file

## license

this is free and unencumbered software released into the public domain. see the [UNLICENSE](../UNLICENSE) file or [unlicense.org](https://unlicense.org/) for details
