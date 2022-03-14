
fn dead_code_test(a: number, b: number) {
  if true {
    return 1
  } else {
    return 1
  }
  # dead code here
  let a = 1
  let b = 1
  let c = 1
}