use clap::{Parser, Subcommand};
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use flume::{Receiver, Sender};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_stream::StreamExt;

#[derive(Debug, Parser)]
pub struct MtgCli {
    #[clap(subcommand)]
    cmd: MtgCmd,
}

impl MtgCli {
    pub async fn execute(self) -> Result<()> {
        self.cmd.execute().await?;
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
pub enum MtgCmd {
    /// Load AllCards.json
    Load {
        #[clap(long, default_value = "./scryfall-oracle-cards.json")]
        path: PathBuf,
    },
}

impl MtgCmd {
    pub async fn execute(self) -> Result<()> {
        match self {
            Self::Load { path } => {
                tracing::info!("Loading AllCards.json");
                let cards = get_cards(&path)?;
                println!("{cards:#?}");

                download_images(cards).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    id: String,
    name: String,
    image_uris: Option<CardImages>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CardImages {
    large: String,
}

pub fn get_cards(json_path: &Path) -> Result<Vec<Card>> {
    let file = std::fs::File::open(json_path)?;
    let file_buf = std::io::BufReader::new(file);
    let cards: Vec<Card> = serde_json::from_reader(file_buf)?;
    Ok(cards)
}

#[derive(Debug)]
pub enum DownloadProgress {
    NewDownload(String, u64), // Name, Total
    Bytes(u64, u64),          // Done, Total
}

async fn download_images(cards: Vec<Card>) -> Result<()> {
    let client = Arc::new(Client::new());

    let multibar = Arc::new(MultiProgress::new());
    let mb2 = Arc::clone(&multibar);
    let style = ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("=>-");
    std::thread::spawn(move || mb2.join().unwrap());

    let (tx, rx) = flume::unbounded();
    let mut handles = vec![];
    for i in 0..128 {
        let (progress_tx, progress_rx) = flume::unbounded();
        let bar = multibar.add(ProgressBar::new(1));
        bar.set_style(style.clone());

        tokio::spawn(download_images_task(
            client.clone(),
            rx.clone(),
            progress_tx,
        ));

        let handle = tokio::spawn(async move {
            tracing::info!("Started progress task {i}");
            while let Ok(progress) = progress_rx.recv_async().await {
                tracing::debug!("Received progress for {i}");
                match progress {
                    DownloadProgress::NewDownload(name, total) => {
                        bar.set_message(format!("Downloading {name}"));
                        bar.set_length(total);
                        bar.set_position(0);
                    }
                    DownloadProgress::Bytes(done, total) => {
                        bar.set_length(total);
                        bar.set_position(done);
                    }
                }
            }
        });

        handles.push(handle);
    }

    for card in cards {
        tx.send_async(card).await.expect("send card");
    }

    let _results = futures::future::join_all(handles).await;

    Ok(())
}

pub async fn download_images_task(
    client: Arc<Client>,
    cards: Receiver<Card>,
    progress: Sender<DownloadProgress>,
) {
    while let Ok(card) = cards.recv_async().await {
        tracing::debug!("Task received card: {}", card.name);
        download_image_task(client.clone(), card, progress.clone()).await;
    }
}

// pub fn download_image(client: Arc<Client>, url: String) -> Result<Receiver<DownloadProgress>> {
//     let (tx, rx) = flume::unbounded();
//     tokio::spawn(download_image_task(client, url, tx));
//     Ok(rx)
// }

pub async fn download_image_task(client: Arc<Client>, card: Card, tx: Sender<DownloadProgress>) {
    let name = card.name;
    let url = match card.image_uris {
        Some(images) => images.large,
        None => return,
    };

    let res = client.get(&url).send().await;
    let res = match res {
        Ok(res) => res,
        Err(e) => {
            tracing::error!("Failed to GET: {:?}", e);
            return;
        }
    };
    let total_size = res.content_length().expect("Content size");
    tx.send_async(DownloadProgress::NewDownload(name.clone(), total_size))
        .await
        .expect("send content size");

    let file =
        std::fs::File::create(format!("./images/{}.png", &card.id)).expect("should create file");
    let mut file_buf = std::io::BufWriter::new(file);
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.try_next().await.expect("Download") {
        file_buf.write_all(&item).expect("should write");
        let new = min(downloaded + (item.len() as u64), total_size);
        downloaded = new;
        tx.send_async(DownloadProgress::Bytes(downloaded, total_size))
            .await
            .expect("Send");
    }
}
