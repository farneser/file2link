use std::collections::HashMap;
use std::error::Error;

use log::{debug, error};
use serde::{Deserialize, Serialize};
use tokio::fs;

const CONFIG_PATH: &str = "config/permissions.json";

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
#[derive(PartialEq)]
enum UsersArrayConfig {
    StringUser(String),
    IntegerUser(i64),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
#[derive(PartialEq)]
enum UsersConfig {
    SingleUser(i64),
    StringUsers(String),
    ArrayUsers(Vec<UsersArrayConfig>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PermissionsConfig {
    allow_all: UsersConfig,
    chats: HashMap<String, UsersConfig>,
}

impl PermissionsConfig {
    fn init_empty() -> Self {
        PermissionsConfig {
            allow_all: UsersConfig::StringUsers("".to_string()),
            chats: HashMap::new(),
        }
    }

    fn init_allow_all() -> Self {
        PermissionsConfig {
            allow_all: UsersConfig::StringUsers("*".to_string()),
            chats: HashMap::new(),
        }
    }

    pub fn user_has_access(&self, chat_id: String, user_id: &String) -> bool {
        fn process_users_config(cfg: &UsersConfig, user_id: &String) -> bool {
            match cfg {
                UsersConfig::SingleUser(user) if user.to_string() == user_id.to_string() => {
                    debug!("User '{}' has access due to allow_all rule", user_id);

                    return true;
                }
                UsersConfig::StringUsers(users) => {
                    if users == "*" || users.to_owned() == user_id.to_owned() {
                        debug!("User '{}' has access due to allow_all rule", user_id);

                        return true;
                    }

                    if users.contains(',') {
                        let ids: Vec<&str> = users.split(',').map(|u| u.trim()).collect();

                        if ids.contains(&user_id.as_str()) {
                            debug!("User '{}' has access due to allow_all rule", user_id);

                            return true;
                        }
                    }

                    debug!("User '{}' does not have access due to allow_all rule", user_id);

                    return false;
                }
                UsersConfig::ArrayUsers(users) => {
                    let ids: Vec<String> = users.iter().map(|user| match user {
                        UsersArrayConfig::StringUser(id) => id.trim().to_owned(),
                        UsersArrayConfig::IntegerUser(id) => id.to_string().trim().to_owned(),
                    }).collect();

                    if ids.contains(&user_id) {
                        debug!("User '{}' has access due to allow_all specific users rule", user_id);

                        return true;
                    }

                    debug!("User '{}' does not have access due to allow_all specific users rule", user_id);

                    return false;
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
    if let Some(path) = CONFIG_PATH.rsplit_once('/') {
        let dir_path = path.0;
        if !dir_path.is_empty() {
            fs::create_dir_all(dir_path).await?;

            debug!("Created directory structure '{}'", dir_path);
        }
    }

    let data = serde_json::to_string_pretty(config)
        .expect("Failed to serialize config");
    fs::write(CONFIG_PATH, data).await?;

    debug!("Configuration saved to '{}'", CONFIG_PATH);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_allow_all() {
        let config = PermissionsConfig::init_allow_all();

        assert_eq!(config.allow_all, UsersConfig::StringUsers("*".to_string()));
        assert!(config.chats.is_empty());
    }

    #[tokio::test]
    async fn test_init_empty() {
        let config = PermissionsConfig::init_empty();

        assert_eq!(config.allow_all, UsersConfig::StringUsers("".to_string()));
        assert!(config.chats.is_empty());
    }

    #[tokio::test]
    async fn test_user_has_access_allow_all() {
        let config = PermissionsConfig::init_allow_all();

        assert!(config.user_has_access("any_chat".to_string(), &"any_user".to_string()));
    }

    #[tokio::test]
    async fn test_user_has_access_specific_user() {
        let mut config = PermissionsConfig::init_empty();

        config.allow_all = UsersConfig::SingleUser(123);

        assert!(config.user_has_access("any_chat".to_string(), &"123".to_string()));
        assert!(!config.user_has_access("any_chat".to_string(), &"456".to_string()));
    }

    #[tokio::test]
    async fn test_user_has_access_string_users() {
        let mut config = PermissionsConfig::init_empty();

        config.allow_all = UsersConfig::StringUsers("user1, user2".to_string());

        assert!(config.user_has_access("any_chat".to_string(), &"user1".to_string()));
        assert!(config.user_has_access("any_chat".to_string(), &"user2".to_string()));
        assert!(!config.user_has_access("any_chat".to_string(), &"user3".to_string()));
    }

    #[tokio::test]
    async fn test_user_has_access_array_users() {
        let mut config = PermissionsConfig::init_empty();

        config.allow_all = UsersConfig::ArrayUsers(vec![
            UsersArrayConfig::StringUser("user1".to_string()),
            UsersArrayConfig::IntegerUser(123),
        ]);

        assert!(config.user_has_access("any_chat".to_string(), &"user1".to_string()));
        assert!(config.user_has_access("any_chat".to_string(), &"123".to_string()));
        assert!(!config.user_has_access("any_chat".to_string(), &"user2".to_string()));
    }

    #[tokio::test]
    async fn test_user_has_access_to_chat() {
        let mut config = PermissionsConfig::init_empty();

        config.chats.insert("chat1".to_string(), UsersConfig::SingleUser(123));
        config.chats.insert("chat2".to_string(), UsersConfig::StringUsers("user1, user2".to_string()));
        config.chats.insert("chat3".to_string(), UsersConfig::ArrayUsers(vec![
            UsersArrayConfig::StringUser("user1".to_string()),
            UsersArrayConfig::IntegerUser(123),
        ]));

        assert!(config.user_has_access("chat1".to_string(), &"123".to_string()));
        assert!(config.user_has_access("chat2".to_string(), &"user1".to_string()));
        assert!(config.user_has_access("chat2".to_string(), &"user2".to_string()));
        assert!(!config.user_has_access("chat2".to_string(), &"user3".to_string()));
        assert!(config.user_has_access("chat3".to_string(), &"user1".to_string()));
        assert!(config.user_has_access("chat3".to_string(), &"123".to_string()));
        assert!(!config.user_has_access("chat3".to_string(), &"user2".to_string()));
    }
}
