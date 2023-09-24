use sqlx::MySqlPool;

pub struct DB {
    connection_url: String,
    pool: MySqlPool,
}

impl DB {
    pub async fn new(connection_url: &str) -> Result<DB, sqlx::Error> {
        let pool = MySqlPool::connect(connection_url).await?;

        Ok(DB {
            connection_url: connection_url.to_string(),
            pool,
        })
    }

    pub fn get_all(&self) -> Vec<String> {
        vec!["gay".to_string()]
    }

    pub fn add(&self, link: &str) {
        println!("added {}", link);
    }
}
