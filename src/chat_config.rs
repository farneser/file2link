use std::collections::HashMap;
use std::error::Error;

use log::{debug, error};
use serde::{Deserialize, Serialize};
use tokio::fs;

const CONFIG_PATH: &str = "permissions.json";

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum UsersConfig {
    AllUsers(String),
    SpecificUsers { users: Vec<String> },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PermissionsConfig {
    allow_all: UsersConfig,
    chats: HashMap<String, UsersConfig>,
}

impl PermissionsConfig {
    fn init_allow_all() -> Self {
        PermissionsConfig {
            allow_all: UsersConfig::AllUsers("*".to_string()),
            chats: HashMap::new(),
        }
    }

    pub fn user_has_access(&self, chat_id: String, user_id: &String) -> bool {
        fn process_users_config(cfg: &UsersConfig, user_id: &String) -> bool {
            match cfg {
                UsersConfig::AllUsers(users) if users == "*" || users.to_owned() == user_id.to_owned() => {
                    debug!("User '{}' has access due to allow_all rule", user_id);

                    return true;
                }
                UsersConfig::SpecificUsers { users } if users.contains(&user_id) => {
                    debug!("User '{}' has access due to allow_all specific users rule", user_id);

                    return true;
                }
                _ => { false }
            }
        }

        debug!("Checking access for user '{}' in chat '{}'", user_id, chat_id);

        if process_users_config(&self.allow_all, user_id) {
            return true;
        }

        if let Some(chat) = self.chats.get(&chat_id) {
            return process_users_config(chat, user_id);
        }

        false
    }
}

async fn create_initial_config() -> Result<(), Box<dyn Error>> {
    debug!("Creating initial configuration");

    let initial_config = PermissionsConfig::init_allow_all();

    save_config(&initial_config).await
}

pub async fn load_config() -> Result<PermissionsConfig, Box<dyn Error>> {
    let mut attempts = 0;

    let data = loop {
        match fs::read_to_string(CONFIG_PATH).await {
            Ok(data) => break data,
            Err(_) => {
                if attempts >= 2 {
                    error!("Failed to read config after 3 attempts");

                    return Err("Failed to read config after 3 attempts".into());
                }

                debug!("Attempt {} to read config failed, creating initial config", attempts + 1);

                create_initial_config().await.expect("Failed to create initial config");
                attempts += 1;
            }
        }
    };

    let config: PermissionsConfig = match serde_json::from_str(&data) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to parse configuration: {}", e);

            return Err("Failed to parse configuration".into());
        }
    };

    debug!("Successfully loaded configuration");

    Ok(config)
}

pub async fn save_config(config: &PermissionsConfig) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_string_pretty(config)
        .expect("Failed to serialize config");
    fs::write(CONFIG_PATH, data).await?;

    debug!("Configuration saved to '{}'", CONFIG_PATH);

    Ok(())
}
