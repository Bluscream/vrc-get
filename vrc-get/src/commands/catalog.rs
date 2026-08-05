use clap::Parser;

pub const VRC_GET_CATALOG_URL: &str =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";
pub const VPM_CATALOG_URL: &str =
	"https://raw.githubusercontent.com/kurotu/vpm-catalog/master/repositories.txt";

/// List remote VPM package catalog repositories from global catalog feeds
#[derive(Parser)]
pub struct Catalog {}

impl Catalog {
	pub async fn run(self) {
		println!("Fetching VPM catalog repositories...");
		let client = reqwest::Client::new();
		let mut repo_urls = Vec::new();

		if let Ok(res) = client.get(VPM_CATALOG_URL).send().await {
			if res.status().is_success() {
				if let Ok(text) = res.text().await {
					for line in text.lines() {
						let line = line.trim();
						if !line.is_empty() && !line.starts_with('#') {
							repo_urls.push(line.to_string());
						}
					}
				}
			}
		}

		if let Ok(res) = client.get(VRC_GET_CATALOG_URL).send().await {
			if res.status().is_success() {
				if let Ok(text) = res.text().await {
					for line in text.lines() {
						let line = line.trim();
						if !line.is_empty() && !line.starts_with('#') {
							if !repo_urls.iter().any(|u| u.eq_ignore_ascii_case(line)) {
								repo_urls.push(line.to_string());
							}
						}
					}
				}
			}
		}

		println!("Found {} catalog repositories:", repo_urls.len());
		for url in &repo_urls {
			println!("- {url}");
		}
	}
}
