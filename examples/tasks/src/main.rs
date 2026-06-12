//! Runs the Loami tasks example against the backend named by `LOAMI_URL` (default `mem://`).

use loami::{Result, MEM_URL};
use loami_example_tasks::{connect, run};

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::var("LOAMI_URL").unwrap_or_else(|_| MEM_URL.to_string());
    println!("Loami tasks example — backend: {url}");

    let db = connect(&url).await?;
    run(&db).await?;

    println!("done");
    Ok(())
}
