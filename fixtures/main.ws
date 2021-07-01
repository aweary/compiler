enum Result<T, E> {
	Ok(T)
	Err(E)
}

fn main {
   let ok = Result.Ok("Hello").Bar()
}