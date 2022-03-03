fn View(align: string) {}
fn Stack() {}
fn Button(label: string) {}

const body = View(align: "top") {
  Stack {
    Button("Hello")
  }
}