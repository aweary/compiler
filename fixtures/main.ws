
fn dead_code_test(a: number, b: number) {
  if true {
    let a = 1
  } else if true {
    let a = 1
    let b = 1
  } else {
    let a = 1
    let b = 1
    let c = 1
  }
}