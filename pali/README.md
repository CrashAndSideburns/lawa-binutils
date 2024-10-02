# pali

pali is an assembler for the ilo isa, which produces assembled code in the poki relocatable binary format

## installation

the easiest way to install pali is using cargo, the package manager for the rust programming language. if cargo is not installed on your computer, consult [the installation instructions](https://www.rust-lang.org/tools/install). once cargo is installed, the newest version of pali may be installed by simply running

``` bash
cargo install --git https://codeberg.org/mra/ilo-binutils --bin pali
```

## usage

although it is fairly simple, complete documentation of the pali assembly language does not yet exist. for lack of documentation, we present here a simple example. the following is a simple program which computes the nth fibonacci number, where n is the number contained in the r1 register. the routine leaves the output in the r2 register

```scheme
(export fibonacci)

(segment rx
    (block fibonacci
        (addi r2 r0 0)
        (addi r3 r0 1)
        (block loop
            (beq r1 r0 fibonacci.return)
            (addi r4 r3 0)
            (add r3 r2)
            (addi r2 r4 0)
            ; pali does not yet support negative numeric literals, but all numbers are 16-bit, so this works!
            ; oh, and ';' marks the rest of the line as a comment :)
            (addi r1 r1 0xFFFF)
            (jsh fibonacci.loop))
    (block return)))
```

because the fibonacci label is exported, other programs which refer to this label but do not define it can be linked against this program, once both are assembled, to resolve the reference. supposing that the contents of the above example are saved to fibonacci.pali, it may be assembled by running

```bash
pali fibonacci.pali
```

which, by default, will write the assembled poki file to fibonacci.poki. if this behaviour is undesired, the output path may be specified by a second optional argument passed to the pali program

## license

this is free and unencumbered software released into the public domain. see the [UNLICENSE](../UNLICENSE) file or [unlicense.org](https://unlicense.org/) for details
