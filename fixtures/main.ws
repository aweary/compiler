const MIN_LENGTH = 10

fn math(shouldAdd: bool, first: number, second: number) {
  if shouldAdd {
    return first + second
  } else {
    return first - second
  }
}

fn seven() {
  let three = math(true, 1, 2)
  let four = math(false, 5, 1)
  let seven = three + four
  return seven
}

const a = seven()