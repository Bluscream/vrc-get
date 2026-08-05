use clap::{Parser, Subcommand};
use reqwest::Url;
use vrc_get_vpm::environment::{add_remote_repo, AddRepositoryErr, Settings};
use vrc_get_vpm::io::DefaultEnvironmentIo;

pub const VRC_GET_CATALOG_URL: &str =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";
pub const VPM_CATALOG_URL: &str =
	"https://raw.githubusercontent.com/kurotu/vpm-catalog/master/repositories.txt";

/// List or add remote VPM package catalog repositories from global catalog feeds
#[derive(Subcommand)]
pub enum Catalog {
	List(List),
	Add(Add),
	Install(Add),
}

impl Catalog {
	pub async fn run(self) {
		match self {
			Catalog::List(cmd) => cmd.run().await,
			Catalog::Add(cmd) => cmd.run().await,
			Catalog::Install(cmd) => cmd.run().await,
		}
	}
}

/// List all available catalog repositories
#[derive(Parser)]
pub struct List {}

impl List {
	pub async fn run(self) {
		let repo_urls = fetch_catalog_urls().await;
		println!("Found {} catalog repositories:", repo_urls.len());
		for url in &repo_urls {
			println!("- {url}");
		}
	}
}

/// Add / install a catalog repository into user environment by URL or search query
#[derive(Parser)]
pub struct Add {
	/// URL or name search query for the repository feed
	#[arg()]
	query: String,
}

impl Add {
	pub async fn run(self) {
		let client = reqwest::Client::new();
		let io = DefaultEnvironmentIo::new_default();
		let mut settings = match Settings::load(&io).await {
			Ok(s) => s,
			Err(err) => {
				eprintln!("Error loading settings: {err}");
				return;
			}
		};

		let target_url = if self.query.starts_with("http://") || self.query.starts_with("https://") {
			match Url::parse(&self.query) {
				Ok(u) => u,
				Err(err) => {
					eprintln!("Invalid URL: {err}");
					return;
				}
			}
		} else {
			let catalog_urls = fetch_catalog_urls().await;
			let q = self.query.to_lowercase();
			let matched = catalog_urls
				.into_iter()
				.find(|url| url.to_lowercase().contains(&q));

			match matched {
				Some(u) => match Url::parse(&u) {
					Ok(parsed) => parsed,
					Err(err) => {
						eprintln!("Invalid matched catalog URL ({u}): {err}");
						return;
					}
				},
				None => {
					eprintln!("No catalog repository matching '{}' found.", self.query);
					return;
				}
			}
		};

		println!("Adding repository feed from {}...", target_url);

		match add_remote_repo(&mut settings, target_url.clone(), None, indexmap::IndexMap::new(), &io, &client).await {
			Ok(_) => {
				if let Err(err) = settings.save(&io).await {
					eprintln!("Failed to save settings: {err}");
				} else {
					println!("Successfully added repository feed: {}", target_url);
				}
			}
			Err(AddRepositoryErr::AlreadyAdded) => {
				println!("Repository is already added: {}", target_url);
			}
			Err(err) => {
				eprintln!("Failed to add repository: {err}");
			}
		}
	}
}

async fn fetch_catalog_urls() -> Vec<String> {
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

	repo_urls
}
