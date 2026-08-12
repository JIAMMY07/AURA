use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri::async_runtime::block_on(async {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client.get("https://example.com/").send().await?;
        if !response.status().is_success() {
            return Err(format!("HTTPS probe returned {}", response.status()).into());
        }
        println!("HTTPS rustls probe: {}", response.status());
        Ok(())
    })
}
