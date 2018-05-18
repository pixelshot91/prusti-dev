#![feature(nll)]

extern crate prusti_contracts;

struct IntPair { a: i32, b: (i32, i32) }

enum IntPairOption<'a> { Some(&'a IntPair), None }

fn foo<'a>(arg: IntPairOption<'a>) -> i32 {
    let mut x = IntPair { a: 111, b: (222, 333) };
    x.b.0 = 444;
    let y = x;
    let z = IntPairOption::Some(&y);
    match arg {
        IntPairOption::Some(k) => k.a,
        IntPairOption::None => 555
    }
}

fn main() {

}
