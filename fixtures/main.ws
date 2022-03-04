import core.{Text, Stack, View}
import core.dom.{getElementById}

component HelloWorld {
  Stack {
   Text("Hello, world")
     .size(42)
   Text("This is great")
     .size(24)
   Text("cool cool cool")
     .color("red")
  }
  .spacing(4)
}

fn main<T>() {
  let target  = "Hello"
  let view = HelloWorld()
}