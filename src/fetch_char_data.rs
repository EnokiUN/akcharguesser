use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::util::normalise_name;

const SKIN_DATA_URL: &str = "https://raw.githubusercontent.com/yuanyan3060/ArknightsGameResource/refs/heads/main/gamedata/excel/skin_table.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDataResponse {
    char_skins: HashMap<String, PartialSkinData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDisplay {
    model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialSkinData {
    portrait_id: Option<String>,
    display_skin: SkinDisplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinData {
    pub skin_id: String,
    pub model_name: String,
}

pub async fn save_skin_data() -> Result<Vec<SkinData>> {
    let client = Client::new();
    let data: SkinDataResponse = client.get(SKIN_DATA_URL).send().await?.json().await?;
    let skins = data
        .char_skins
        .into_values()
        .into_iter()
        .filter(|s| s.display_skin.model_name.is_some())
        .map(|s| SkinData {
            skin_id: s.portrait_id.unwrap(),
            model_name: normalise_name(&s.display_skin.model_name.unwrap()),
        })
        .collect();
    fs::write("skins.json", serde_json::to_string(&skins)?).await?;
    Ok(skins)
}

pub async fn load_skin_data() -> Result<Vec<SkinData>> {
    let data = fs::read_to_string("skins.json").await?;
    Ok(serde_json::from_str(&data)?)
}
