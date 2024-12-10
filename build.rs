// https://stackoverflow.com/questions/30291757/attaching-an-icon-resource-to-a-rust-application

use std::io;

use winresource::WindowsResource;

fn main() -> io::Result<()> {
    if cfg!(all(target_os = "windows", not(debug_assertions))) {
        WindowsResource::new()
            .set_icon("icon/icon.ico")
            .compile()?;
    }
    Ok(())
}