fn test(x: number) {
  if x > 5 {
    if x == 6 {
      return 5
    }
    let y = x + 10
    if y == 17 {
      return 10000
    }
    return y - 1
  }
  let y = 2
  return x + y
}

# Potential byte-code format
# DEFINE_FUNCTION 'test'

fn main() {
  # This is statically evaluated!
  let answer = test(7)
}