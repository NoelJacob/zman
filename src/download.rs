// https://github.com/DemwE/rgetd/tree/main/src/download.rs

use reqwest::Url;
use std::fs::File;
use std::io::{Write, BufWriter};
use std::path::PathBuf;
use indicatif::{ProgressBar, ProgressStyle};
use eyre::{Result, bail};
use reqwest::Client;

pub async fn download_file(client: &Client, url: &str, save_path: &PathBuf) -> Result<()> {
    // Parse URL
    let url = Url::parse(url)?;

    // Check if file already exists
    let mut start = 0;
    if save_path.exists() {
        start = save_path.metadata()?.len();
    }

    // Make GET request
    let mut response = client.get(url.clone())
        .header("Range", format!("bytes={}-", start))
        .send()
        .await?;

    if response.status().is_success() {
        // Get total file size from response headers
        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} | {binary_bytes_per_sec} | eta {eta}")
                .unwrap()
                .progress_chars("#>-"), // apply parsed config
        );

        // Open file for writing
        let file = File::create(save_path)?;
        let mut buffered_file = BufWriter::new(file);

        // Read response in chunks and write to file with progress update
        let mut downloaded = 0;
        while let Some(chunk) = response.chunk().await? {
            buffered_file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        buffered_file.flush()?; // Flush the buffer to ensure all data is written to disk

        Ok(())
    } else {
        bail!("Server status: {}", response.status());
    }
}