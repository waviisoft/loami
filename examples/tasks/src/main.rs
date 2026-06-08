//! Runs the Loami tasks example against the backend named by `LOAMI_URL` (default `mem://`).

use loami::Result;
use loami_example_tasks::{connect, run, DEFAULT_URL};

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::var("LOAMI_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    println!("Loami tasks example — backend: {url}");

    let db = connect(&url)?;
    run(&db).await?;

    println!("done");
    Ok(())
}
