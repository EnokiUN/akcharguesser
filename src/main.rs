mod fetch_char_data;
mod util;

use anyhow::{bail, Context, Result};
use fetch_char_data::{load_skin_data, SkinData};
use image::load_from_memory;
use rand::{seq::IndexedRandom, Rng};
use reqwest::{Client, Url};
use std::{
    cmp::min,
    env,
    io::{BufWriter, Cursor},
    sync::Arc,
    time::Duration,
};
use tokio::time::timeout;
use twilight_cache_inmemory::{DefaultInMemoryCache, ResourceType};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt as _};
use twilight_http::Client as HttpClient;
use twilight_model::{gateway::payload::incoming::MessageCreate, http::attachment::Attachment};
use twilight_standby::Standby;
use util::normalise_name;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    if let Some("populate") = env::args().nth(1).as_deref() {
        fetch_char_data::save_skin_data().await?;
        println!("Saved skin data to skins.json");
        return Ok(());
    }

    let token = env::var("TOKEN")?;

    let mut shard = Shard::new(
        ShardId::ONE,
        token.clone(),
        Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
    );

    let http = Arc::new(HttpClient::new(token));
    let http_client = Arc::new(Client::new());
    let standby = Arc::new(Standby::new());

    let user = http.current_user().await?.model().await?;
    log::info!("Logged in as {}#{}", user.name, user.discriminator());

    let skin_data = Arc::new(load_skin_data().await?);

    let cache = DefaultInMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        let Ok(event) = item else {
            tracing::warn!(source = ?item.unwrap_err(), "error receiving event");

            continue;
        };

        cache.update(&event);
        standby.process(&event);

        tokio::spawn(handle_event(
            event,
            Arc::clone(&http),
            Arc::clone(&http_client),
            Arc::clone(&standby),
            Arc::clone(&skin_data),
        ));
    }

    Ok(())
}

async fn handle_event(
    event: Event,
    http: Arc<HttpClient>,
    http_client: Arc<Client>,
    standby: Arc<Standby>,
    skin_data: Arc<Vec<SkinData>>,
) -> Result<()> {
    match event {
        Event::MessageCreate(msg) if msg.content == "e!guess" => {
            let reply = http
                .create_message(msg.channel_id)
                .reply(msg.id)
                .content("Loading...")
                .await?
                .model()
                .await?;
            let random_skin = skin_data.choose(&mut rand::rng());
            let skin = match random_skin {
                Some(skin) => skin,
                None => {
                    http.create_message(msg.channel_id)
                        .content("Encountered an error while choosing skin")
                        .await?;
                    bail!("Randmizer failed to choose skin");
                }
            };
            let mut url = Url::parse("https://raw.githubusercontent.com")?;
            url.set_path(&format!(
                "yuanyan3060/ArknightsGameResource/refs/heads/main/skin/{}b.png",
                skin.skin_id
            ));
            let image = http_client.get(url.clone()).send().await?.bytes().await?;
            let cropped = match tokio::task::spawn_blocking(move || {
                let mut image = load_from_memory(&image).context("Couldn't parse image")?;
                let mut rng = rand::rng();
                let width = min(image.width() / 5, 500);
                let height = min(image.height() / 5, 500);
                let x = rng.random_range((width / 2)..(image.width() - width - width / 2));
                let y = rng.random_range(0..(image.height() - height));
                let cropped = image.crop(x, y, x + width, y + width);
                let mut bytes: Cursor<Vec<u8>> = Cursor::new(vec![]);
                {
                    let mut writer = BufWriter::new(&mut bytes);
                    cropped
                        .write_to(&mut writer, image::ImageFormat::Png)
                        .context("Couldn't write image result to buffer")?;
                }
                anyhow::Ok(bytes.into_inner())
            })
            .await?
            {
                Ok(cropped) => Ok(cropped),
                Err(err) => {
                    log::warn!("Failed to crop {}, {}", skin.skin_id, err);
                    http.create_message(msg.channel_id)
                        .reply(reply.id)
                        .content("Error happened X(")
                        .await?;
                    Err(err)
                }
            }?;
            http.update_message(reply.channel_id, reply.id)
                .content(Some("Guess who this is!"))
                .attachments(&[Attachment::from_bytes(
                    "skin_snip.png".to_string(),
                    cropped,
                    1,
                )])
                .await?;
            let answer = skin.model_name.clone();
            let future = standby.wait_for_message(msg.channel_id, move |e: &MessageCreate| {
                normalise_name(&e.content) == answer
                    || (e.content == "e!skip" && msg.author.id == e.author.id)
            });
            match timeout(Duration::from_secs(30), future).await {
                Ok(Ok(response)) => {
                    if response.content == "e!skip" {
                        http.create_message(reply.channel_id)
                            .reply(reply.id)
                            .content(&format!(
                                "[Skipped] It's [{}]({})",
                                skin.model_name,
                                url.as_str()
                            ))
                            .await?
                    } else {
                        http.create_message(reply.channel_id)
                            .reply(response.id)
                            .content(":tada:")
                            .await?
                    }
                }
                Err(_) => {
                    http.create_message(reply.channel_id)
                        .reply(reply.id)
                        .content(&format!("It's [{}]({})", skin.model_name, url.as_str()))
                        .await?
                }
                _ => unreachable!(),
            };
        }
        _ => {}
    }

    Ok(())
}
