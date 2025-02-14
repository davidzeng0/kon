## Async via Fibers

Statistics from AMD Zen 3 5800x CPU

### Motivation

Efficiency is a primary focus of all xx_* libraries. Using fibers addresses two major slowdowns of rust's default async implementation

### Polling based, rather than callback based
In order to resume a future N function calls deep, it has to be resumed from the root future. Each layer of function call introduces around 1-2ns of overhead in order to reach the future that can actually do work.

### Memory
Rust coroutines are stackless, and each new future needs memory in order to run. Async functions return sized futures, allowing the rust compiler to allocate all the memory it needs at the root future (effectively acting as a stackful coroutine), saving on allocation costs. However, recursive (and extern async functions, however a much rarer case) require a new memory allocation per level and don't receive pre-allocated benefits.

Due to [#99504](https://github.com/rust-lang/rust/issues/99504), memory is also copied for every nested future and incurs significant overhead.

### Advantages of fibers

Function calls (even async ones) do not incur any overhead over a normal function call in a synchronous context.

### Drawbacks of fibers

Fibers, like threads, can only do one thing at a time. Branching out via `select` or `join` requires spawning a new fiber for each future to be awaited.
The cost of spawning fibers can be alleviated with a pool.

~~Switching fibers incurs a cost of \~3ns (14ns on an Apple M1 with only 8 FP regs preserved, possibly more on other cpus).~~

Switching fibers has been optimized to only incur a cost of about ~1.0-1.5 non-inline function calls worth of of time, +/- cache miss. (Worst case on ARM64 is still 14ns, when a non-inlined async function call suspends)

## Example

```rust
#[asynchronous]
async fn async_add(a: i32, b: i32) -> i32 {
    a + b
}

#[asynchronous]
#[inline(never)]
async fn async_main() {
    let a = 2;
    let b = 3;

    let c = async_add(a, b).await;

    println!("{} + {} = {}", a, b, c);
}
```

Taking a look at the disassembly for `async_main`, the call to `async_add` is entirely inlined
```x86asm
                            ; function async_main
movl   $0x2, 0x8(%rsp)      ; a
movl   $0x3, 0xc(%rsp)      ; b
movl   $0x5, 0x4(%rsp)      ; c
leaq   0x8(%rsp), %rax
movq   %rax, 0x80(%rsp)
leaq   0x45147(%rip), %rax  ; core::fmt::num::imp::<impl core::fmt::Display for i32>::fmt at num.rs:283
```