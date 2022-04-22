use clap::{Parser, Subcommand};
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod nonblocking;
pub mod blocking;

#[derive(Debug, Parser)]
pub struct MtgCli {
    #[clap(subcommand)]
    cmd: MtgCmd,
}

impl MtgCli {
    pub async fn execute_async(self) -> Result<()> {
        self.cmd.execute_async().await?;
        Ok(())
    }

    pub fn execute(self) -> Result<()> {
        self.cmd.execute()?;
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
    pub async fn execute_async(self) -> Result<()> {
        match self {
            Self::Load { path } => {
                tracing::info!("Loading AllCards.json");
                let cards = get_cards(&path)?;
                println!("{cards:#?}");

                nonblocking::download_images(cards).await?;
            }
        }

        Ok(())
    }

    pub fn execute(self) -> Result<()> {
        match self {
            Self::Load { path } => {
                tracing::info!("Loading AllCards.json");
                let cards = get_cards(&path)?;
                println!("{cards:#?}");

                blocking::download_images(cards)?;
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
