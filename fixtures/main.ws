

pub component App {
  state count = 0
  fn handleClick {
    if count < 10 {
      count = count + 1
    }
  }
  return (
    <div>
      <span>{count}</span>
      <button onClick={handleClick}>Click me</button>
      <div>
        <h1>Hello, <span style="color: red">{count}</span></h1>
      </div>
    </div>
  )
}