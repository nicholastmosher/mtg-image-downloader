use std::sync::Arc;
use crate::{Card, DownloadProgress};
use color_eyre::Result;
use flume::{Receiver, Sender};
use reqwest::Client;

pub fn download_images(cards: Vec<Card>) -> Result<()> {
    todo!()
}

pub fn download_images_thread(client: Arc<Client>, cards: Receiver<Card>) {
    todo!()
}

pub fn download_image(client: Arc<Client>, card: Card) {
    todo!()
}
