use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ShuntConfig {
    /// Directory where shunted messages are stored
    pub storage_dir: PathBuf,
    /// Whether to automatically open the browser when a message is shunted
    pub open_browser: bool,
    /// Port for the web preview server
    pub web_port: u16,
    /// Host for the web preview server
    pub web_host: String,
}

impl Default for ShuntConfig {
    fn default() -> Self {
        Self {
            storage_dir: PathBuf::from("tmp/shunt"),
            open_browser: true,
            web_port: 9876,
            web_host: "127.0.0.1".to_string(),
        }
    }
}

impl ShuntConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn storage_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.storage_dir = dir.into();
        self
    }

    pub fn open_browser(mut self, open: bool) -> Self {
        self.open_browser = open;
        self
    }

    pub fn web_port(mut self, port: u16) -> Self {
        self.web_port = port;
        self
    }

    pub fn web_host(mut self, host: impl Into<String>) -> Self {
        self.web_host = host.into();
        self
    }

    /// Returns the full address for the web server
    pub fn web_addr(&self) -> String {
        format!("{}:{}", self.web_host, self.web_port)
    }

    /// Returns the URL for the web preview
    pub fn web_url(&self) -> String {
        format!("http://{}:{}", self.web_host, self.web_port)
    }
}
