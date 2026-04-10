fn f() -> Result<i32, String> {
    let r: Result<Result<i32, String>, String> = Ok(Ok(1));
    r?
}
fn main() {
    f().unwrap();
}
