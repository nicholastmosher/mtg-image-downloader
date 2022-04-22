use std::cmp::min;
use std::io::Write;
use std::sync::Arc;
use flume::{Receiver, Sender};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio_stream::StreamExt;
use crate::{Card, DownloadProgress};
use color_eyre::Result;

pub async fn download_images(cards: Vec<Card>) -> Result<()> {
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
