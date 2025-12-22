# Rust 生命周期学习教程

本教程将全面介绍 Rust 中的生命周期概念，从基础到高级，帮助你深入理解这一重要特性。

## 目录

1. [什么是生命周期](#什么是生命周期)
2. [为什么需要生命周期](#为什么需要生命周期)
3. [生命周期语法](#生命周期语法)
4. [生命周期规则](#生命周期规则)
5. [实践示例](#实践示例)
6. [常见模式](#常见模式)
7. [高级用法](#高级用法)

## 什么是生命周期

生命周期(Lifetime)是 Rust 中用于确保引用有效性的机制。它描述了引用保持有效的作用域范围。

**关键点：**

- 生命周期是一种泛型参数
- 用于防止悬垂引用(dangling references)
- 确保借用检查器能够验证引用的安全性

## 为什么需要生命周期

```rust
// 这段代码会产生编译错误
fn main() {
    let r;                // ---------+-- 'a
                          //          |
    {                     //          |
        let x = 5;        // -+-- 'b  |
        r = &x;           //  |       |
    }                     // -+       |
                          //          |
    println!("r: {}", r); //          |
}                         // ---------+
```

**问题：** `r` 引用的 `x` 在其作用域结束后就被释放了，导致悬垂引用。

**生命周期的作用：** 编译器通过生命周期注解来检测并阻止这类错误。

## 生命周期语法

生命周期参数以单引号 `'` 开头，通常使用小写字母：

```rust
&i32        // 引用
&'a i32     // 带有显式生命周期的引用
&'a mut i32 // 带有显式生命周期的可变引用
```

### 函数签名中的生命周期

```rust
// 基本语法
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() {
        x
    } else {
        y
    }
}
```

**含义：** 返回的引用的生命周期与参数 `x` 和 `y` 中较短的那个相同。

## 生命周期规则

Rust 编译器使用三条生命周期省略规则来推断生命周期：

### 规则 1：每个引用参数都有自己的生命周期

```rust
fn foo<'a>(x: &'a i32) { }
fn bar<'a, 'b>(x: &'a i32, y: &'b i32) { }
```

### 规则 2：如果只有一个输入生命周期参数，该生命周期被赋给所有输出生命周期参数

```rust
fn foo(x: &i32) -> &i32 { x }
// 等价于
fn foo<'a>(x: &'a i32) -> &'a i32 { x }
```

### 规则 3：如果有多个输入生命周期参数，但其中一个是 `&self` 或 `&mut self`，那么 `self` 的生命周期被赋给所有输出生命周期参数

```rust
impl MyStruct {
    fn method(&self, other: &str) -> &str {
        // self 的生命周期自动应用到返回值
    }
}
```

## 实践示例

查看 `examples/` 目录中的实际代码示例：

- `01_basic_lifetime.rs` - 基础生命周期示例
- `02_struct_lifetime.rs` - 结构体中的生命周期
- `03_multiple_lifetimes.rs` - 多个生命周期参数
- `04_lifetime_bounds.rs` - 生命周期约束
- `05_static_lifetime.rs` - 'static 生命周期

## 常见模式

### 返回引用

```rust
// 正确：返回参数的引用
fn first_word<'a>(s: &'a str) -> &'a str {
    let bytes = s.as_bytes();
    for (i, &item) in bytes.iter().enumerate() {
        if item == b' ' {
            return &s[0..i];
        }
    }
    &s[..]
}

// 错误：尝试返回本地变量的引用
// fn invalid() -> &String {
//     let s = String::from("hello");
//     &s  // 错误！s 在函数结束时被销毁
// }
```

### 结构体中的引用

```rust
struct ImportantExcerpt<'a> {
    part: &'a str,
}

fn main() {
    let novel = String::from("Call me Ishmael. Some years ago...");
    let first_sentence = novel.split('.').next().expect("Could not find a '.'");
    let excerpt = ImportantExcerpt {
        part: first_sentence,
    };
}
```

## 高级用法

### 生命周期子类型(Lifetime Subtyping)

```rust
fn parse_context<'a>(context: &'a str) -> Result<&'a str, &'static str> {
    if context.is_empty() {
        Err("context is empty")
    } else {
        Ok(context)
    }
}
```

### 生命周期约束(Lifetime Bounds)

```rust
struct Ref<'a, T: 'a> {
    reference: &'a T,
}

// 泛型 T 必须比 'a 活得更久
```

### Higher-Rank Trait Bounds (HRTB)

```rust
fn call_with_ref<F>(f: F)
where
    F: for<'a> Fn(&'a str),
{
    let s = String::from("hello");
    f(&s);
}
```

## 练习建议

1. 运行 `examples/` 中的所有示例代码
2. 尝试修改代码，观察编译器错误
3. 完成 `exercises/` 中的练习题
4. 阅读 Rust 官方文档的生命周期章节

## 参考资源

- [The Rust Programming Language - Validating References with Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- [Rust By Example - Lifetimes](https://doc.rust-lang.org/rust-by-example/scope/lifetime.html)
- [Rustonomicon - Lifetimes](https://doc.rust-lang.org/nomicon/lifetimes.html)

## 下一步

掌握生命周期后，建议继续学习：

- 智能指针(Smart Pointers)
- 所有权(Ownership)和借用(Borrowing)的高级概念
- 并发编程中的生命周期应用
