
fn dead_code_test(a: number, b: number) {
  let c = a + b
  if true {
    if true {
      let y = 2
      let z = 3
      let x = 1
    } else {
      return 5
    }
  } else {
    return 6
  }
  let x = 1
}